//! 单机保留数量与资源生命周期。不包含同步墓碑或云端确认。
use std::sync::atomic::Ordering;
use clip_core::{domain::clip::ClipRecord,errors::AppResult};
use crate::LocalContext;

/// 按本地保留数量淘汰旧记录，并在删除提交后回收其缓存。
pub struct CleanupService<'a>{local:&'a LocalContext}
/// 清理标记的作用域守卫；正常结束或提前报错都会自动解除“正在清理”。
struct CleaningGuard<'a>(&'a std::sync::atomic::AtomicBool);
impl Drop for CleaningGuard<'_>{
    /// 释放原子标记，让后续事件可以再次触发清理。
    fn drop(&mut self){self.0.store(false,Ordering::Release);}
}
impl<'a> CleanupService<'a>{
    /// 借用单机上下文，不依赖云端状态或删除确认。
    pub fn new(local:&'a LocalContext)->Self{Self{local}}
    /// 同一时刻只运行一次；其他调用发现已有清理任务时直接返回。
    pub async fn try_clean(&self)->AppResult<()>{
        if self.local.cleaning.swap(true,Ordering::Acquire){return Ok(());}
        let _guard=CleaningGuard(&self.local.cleaning);
        // 与采集和主动删除互斥，避免刚保存的附件被交叉清理。
        let _records=self.local.capture_lock.lock().await;
        let max=self.local.with_settings(|settings|settings.max_records)?;
        // 仓储先在事务内删除并返回快照，磁盘清理不能发生在提交之前。
        let removed=self.local.repositories().clip_records().remove_overflow(max).await?;
        for record in removed{
            // 一条索引清理失败不影响当前及其余已删除记录的缓存回收。
            if let Err(error)=self.local.search_engine().remove(&[record.id.to_string()]).await {
                log::error!("移除超限记录的搜索索引失败，ID: {}，错误: {}",record.id,error);
            }
            delete_managed_resources(self.local,&record).await;
        }
        Ok(())
    }
}

/// 只有明确声明为 managed 且位于资源根内的普通文件可删除，外部原件和目录不删除。
pub async fn delete_managed_resources(local:&LocalContext,record:&ClipRecord){
    // reference 属于用户原件，无论路径看起来在哪里都不在此处删除。
    for resource in record.resources.iter().filter(|resource|resource.storage_kind=="managed"){
        // 复用受管资源的路径检查，支持用户改过备份目录的旧记录。
        let Ok(path)=crate::records::resolve_resource(local, resource)else{continue;};
        if path.is_file(){
            if let Err(error)=std::fs::remove_file(&path){log::error!("删除受管缓存失败: {}",error);}
        }
    }
}

/// 资源更新提交后，仅回收不再被当前记录使用的旧文件。
pub async fn delete_replaced_resources(local:&LocalContext,old:&ClipRecord,current:&ClipRecord){
    let mut obsolete=old.clone();
    // 比较所有权和保存路径；同内容重采集复用的文件必须保留。
    obsolete.resources.retain(|old|!current.resources.iter().any(|next|old.storage_kind==next.storage_kind && old.path==next.path));
    delete_managed_resources(local,&obsolete).await;
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use clip_core::{errors::AppError, search::SearchEngine};
    use crate::{capture::{CaptureInput, CaptureService}, records::{ClipRecordService, resolve_resource}, dto::QueryParam};
    use super::*;

    /// 故意模拟索引不可写，数据库删除后仍应清理全部受管缓存。
    struct FailingIndex;
    #[async_trait::async_trait]
    impl SearchEngine for FailingIndex {
        /// 返回故意保留的失效 ID，验证数据库回查能过滤已删除记录。
        async fn search(&self, _: &str) -> Vec<String> { vec!["1".into()] }
        /// 测试替身接受索引写入，只让删除索引路径故意失败。
        async fn add(&self, _: &str, _: &str) -> AppResult<()> { Ok(()) }
        /// 故意模拟索引删除失败，检查后续缓存回收是否仍会执行。
        async fn remove(&self, _: &[String]) -> AppResult<()> { Err(AppError::System("test index failure".into())) }
        /// 测试替身不建真实索引，只验证删除故障后的业务收尾。
        async fn initialize(&self, _: Vec<ClipRecord>) -> AppResult<()> { Ok(()) }
    }

    /// 验证索引故障不阻断已提交删除、幂等重删和超限缓存回收。
    #[tokio::test]
    async fn committed_deletes_and_retention_finish_cache_cleanup_even_when_index_fails() {
        let dir = tempfile::tempdir().unwrap();
        let resources = dir.path().join("resources");
        std::fs::create_dir(&resources).unwrap();
        let mut local = LocalContext::open_with_resources(
            &dir.path().join("test.db"), None, Arc::new(|value: &str| Ok(value.to_owned())), Some(resources),
        ).await.unwrap();
        let mut saved = Vec::new();
        for byte in 1..=4 {
            saved.push(CaptureService::new(&local, &|value: &str| Ok(value.to_owned()))
                .capture(CaptureInput::Image(Some(&vec![byte]))).await.unwrap().unwrap());
        }
        let paths: Vec<_> = saved.iter().map(|row| resolve_resource(&local, &row.resources[0]).unwrap()).collect();
        local.search_engine = Arc::new(FailingIndex);
        let service = ClipRecordService::new(&local);
        service.delete_record(saved[0].id.to_string()).await.unwrap();
        service.delete_record(saved[0].id.to_string()).await.unwrap(); // 重复删除幂等。
        assert!(!paths[0].exists());
        // 即使索引仍返回已删除 ID，页面查询也必须回查数据库过滤。
        assert!(service.query_clip_records(QueryParam { page: 1, size: 10, search: Some("stale".into()) }).await.unwrap().is_empty());
        local.update_settings(|settings| settings.max_records = 1).unwrap();
        CleanupService::new(&local).try_clean().await.unwrap();
        assert!(!paths[1].exists() && !paths[2].exists() && paths[3].exists());
        let remaining = local.repositories().clip_records().list_all().await.unwrap();
        assert_eq!(remaining.len(), 1);
        assert_eq!(remaining[0].id, saved[3].id);
        let rows: Vec<clip_core::domain::clip::ClipRecordResource> = local.db().query_decode(
            "SELECT * FROM clip_record_resource", vec![],
        ).await.unwrap();
        assert_eq!(rows, remaining[0].resources);
    }
}

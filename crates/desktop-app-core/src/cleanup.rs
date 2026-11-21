//! 本地保留条数及资源生命周期管理。单机无需等待云端确认删除。
use std::{path::{Path, PathBuf}, sync::atomic::Ordering};
use clip_core::{domain::clip::{ClipRecord, IMAGE_TYPE, FILE_TYPE}, errors::AppResult};
use crate::LocalContext;

pub struct CleanupService<'a> { local: &'a LocalContext, retain_tombstones: bool }
struct CleaningGuard<'a>(&'a std::sync::atomic::AtomicBool);
impl Drop for CleaningGuard<'_> { fn drop(&mut self) { self.0.store(false, Ordering::Release); } }
impl<'a> CleanupService<'a> {
    pub fn new(local: &'a LocalContext, retain_tombstones: bool) -> Self { Self { local, retain_tombstones } }
    pub async fn try_clean(&self) -> AppResult<()> {
        // AtomicBool状态保护器：只对当前应用串行清理，不影响其他实例或测试。
        if self.local.cleaning.swap(true, Ordering::Acquire) { return Ok(()); }
        let _guard = CleaningGuard(&self.local.cleaning);
        let _records = self.local.capture_lock.lock().await;
        let repository = self.local.repositories().clip_records();
        let max_records = self.local.with_settings(|settings| settings.max_records)?;
        let overflow = repository.list(-1, max_records.min(i32::MAX as u32) as i32).await?;
        for record in overflow.into_iter().filter(|record| !record.is_pinned()) {
            let ids = vec![record.id.clone()];
            if self.retain_tombstones { repository.mark_deleted(&ids).await?; }
            else { repository.delete_permanently(&ids).await?; }
            self.local.search_engine().remove(&ids).await?;
            delete_managed_resources(self.local, &record).await;
        }
        // 数据清理分为超限有效记录与墓碑记录；同步版仅清理已确认的墓碑。
        let deleted = if self.retain_tombstones { repository.list_synced_tombstones().await? }
            else { repository.list_tombstones().await? };
        for record in deleted {
            let ids = vec![record.id.clone()];
            repository.delete_permanently(&ids).await?;
            self.local.search_engine().remove(&ids).await?;
            delete_managed_resources(self.local, &record).await;
        }
        Ok(())
    }
}

/// 删除resources目录下的文件；单文件原始路径与多文件原件绝不删除。
/// 新文件记录显示名称和缓存路径分开保存，因此优先检查 local_file_path。
pub async fn delete_managed_resources(local: &LocalContext, record: &ClipRecord) {
    let Some(base) = local.resources_dir().and_then(|path| path.canonicalize().ok()) else { return; };
    let mut candidates = Vec::new();
    match record.r#type.as_str() {
        IMAGE_TYPE => {
            if let Some(content) = record.content.as_str() { candidates.push(base.join(content)); }
            if let Some(path) = &record.local_file_path { candidates.push(PathBuf::from(path)); }
        }
        FILE_TYPE => {
            if let Some(path) = &record.local_file_path {
                if !path.contains(":::") { candidates.push(PathBuf::from(path)); }
            }
            if let Some(content) = record.content.as_str().filter(|value| value.starts_with("files/")) {
                candidates.push(base.join(content));
            }
        }
        _ => return,
    }
    for path in candidates {
        // 规范化后检查边界，同时防止 .. 和符号链接越出资源目录。
        let Ok(owned) = path.canonicalize() else { continue; };
        if owned != base && owned.starts_with(&base) && Path::new(&owned).is_file() {
            if let Err(error) = std::fs::remove_file(&owned) {
                log::error!("删除缓存资源失败: {:?}: {}", owned, error);
            }
        }
    }
}

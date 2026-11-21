//! 两表共用的 SQLite 仓储：主记录和附件作为一个整体读取、写入和删除。
use async_trait::async_trait;
use std::collections::HashMap;
use rbatis::{RBatis, executor::Executor};
use rbs::to_value;
use crate::{domain::clip::{ClipRecord, ClipRecordResource}, errors::{AppError, AppResult}, repositories::ClipRecordRepository};

/// 本地记录仓储。只负责数据库，不操作搜索索引、缓存文件或系统剪贴板。
pub struct SqliteClipRecordRepository { db: RBatis }
impl SqliteClipRecordRepository {
    /// 共享已有连接池；调用方负责先完成表结构初始化。
    pub fn new(db: RBatis) -> Self { Self { db } }

    /// 主表与资源表必须来自同一个读快照，避免并发替换资源时返回不匹配的数据。
    async fn read(&self, sql: &str, params: Vec<rbs::Value>) -> AppResult<Vec<ClipRecord>> {
        let tx = self.db.acquire_begin().await?;
        // 提前返回或读取报错时回滚，避免把未结束的事务留在连接池里。
        let tx = tx.defer_async(|tx| async move {
            if !tx.done() { let _ = tx.rollback().await; }
        });
        let records = load(&tx, sql, params).await?;
        tx.commit().await?;
        Ok(records)
    }
}

/// 执行主表查询，再按主键补齐有序资源；既能用于读事务，也能用于写事务。
async fn load(executor: &dyn Executor, sql: &str, params: Vec<rbs::Value>) -> AppResult<Vec<ClipRecord>> {
    let mut records: Vec<ClipRecord> = rbatis::decode(executor.query(sql, params).await?)?;
    if records.is_empty() { return Ok(records); }
    // 建立“记录 ID → 数组位置”的映射，分配每个附件时不用遍历整个记录列表。
    let positions: HashMap<i64, usize> = records.iter().enumerate().map(|(index, record)| (record.id, index)).collect();
    let ids: Vec<_> = records.iter().map(|record| record.id).collect();
    // 启动索引重建会读取全部记录，资源查询也分批，不能只对搜索查询分批。
    for chunk in ids.chunks(400) {
        let placeholders = chunk.iter().map(|_| "?").collect::<Vec<_>>().join(",");
        let resources: Vec<ClipRecordResource> = rbatis::decode(executor.query(&format!("SELECT * FROM clip_record_resource WHERE record_id IN ({placeholders}) ORDER BY record_id, position"), chunk.iter().map(|id| to_value!(id)).collect()).await?)?;
        for resource in resources {
            if let Some(&index) = positions.get(&resource.record_id) { records[index].resources.push(resource); }
        }
    }
    Ok(records)
}

/// 在调用方事务中持久化记录与资源，用于采集及一次性旧数据迁移。
/// 新记录的 id 由数据库生成；重复内容保留首次时间和置顶状态。
pub async fn persist_record(executor: &dyn Executor, record: &ClipRecord) -> AppResult<i64> {
    // 同内容更新最近复制时间和排序；故意不更新首次时间、主键和置顶状态。
    executor.exec("INSERT INTO clip_record (content_key,type,content,created_at,last_copied_at,sort_order,pinned) VALUES (?,?,?,?,?,?,?) ON CONFLICT(content_key) DO UPDATE SET type=excluded.type, content=excluded.content, last_copied_at=excluded.last_copied_at, sort_order=excluded.sort_order", vec![
        to_value!(&record.content_key), to_value!(&record.r#type), to_value!(&record.content), to_value!(record.created_at), to_value!(record.last_copied_at), to_value!(record.sort_order), to_value!(record.pinned),
    ]).await?;
    // 冲突更新时不能依赖 last_insert_rowid，按唯一内容键读回实际主键。
    #[derive(serde::Deserialize)] struct Id { id: i64 }
    let rows: Vec<Id> = rbatis::decode(executor.query("SELECT id FROM clip_record WHERE content_key=?", vec![to_value!(&record.content_key)]).await?)?;
    let id = rows[0].id;
    // 资源清单整组替换；任一插入失败，由外层事务恢复原清单。
    executor.exec("DELETE FROM clip_record_resource WHERE record_id=?", vec![to_value!(id)]).await?;
    for resource in &record.resources {
        executor.exec("INSERT INTO clip_record_resource (record_id,position,display_name,resource_kind,storage_kind,path,byte_size,content_hash) VALUES (?,?,?,?,?,?,?,?)", vec![to_value!(id), to_value!(resource.position), to_value!(&resource.display_name), to_value!(&resource.resource_kind), to_value!(&resource.storage_kind), to_value!(&resource.path), to_value!(resource.byte_size), to_value!(&resource.content_hash)]).await?;
    }
    Ok(id)
}

#[async_trait]
impl ClipRecordRepository for SqliteClipRecordRepository {
    /// 读取全部记录及资源，主要供启动重建索引使用；-1 表示不限制条数。
    async fn list_all(&self) -> AppResult<Vec<ClipRecord>> { self.list(-1, 0).await }
    /// 根据本地主键查找，记录不存在时返回 None。
    async fn find_by_id(&self, id: i64) -> AppResult<Option<ClipRecord>> {
        Ok(self.read("SELECT * FROM clip_record WHERE id=?", vec![to_value!(id)]).await?.into_iter().next())
    }
    /// 根据内容身份查重，不依赖设备 ID 或记录 ID。
    async fn find_by_content_key(&self, key: &str) -> AppResult<Option<ClipRecord>> {
        Ok(self.read("SELECT * FROM clip_record WHERE content_key=?", vec![to_value!(key)]).await?.into_iter().next())
    }
    /// 先置顶、再按最近采集顺序分页；ID 用于顺序相同时稳定排序。
    async fn list(&self, limit: i32, offset: i64) -> AppResult<Vec<ClipRecord>> {
        self.read("SELECT * FROM clip_record ORDER BY pinned DESC,sort_order DESC,id DESC LIMIT ? OFFSET ?", vec![to_value!(limit),to_value!(offset)]).await
    }
    /// 搜索索引只提供候选 ID；回查数据库后统一排序分页，过滤失效结果。
    async fn list_by_ids(&self, ids: &[String], limit: i32, offset: i64) -> AppResult<Vec<ClipRecord>> {
        if ids.is_empty() { return Ok(vec![]); }
        let tx = self.db.acquire_begin().await?;
        let tx = tx.defer_async(|tx| async move {
            if !tx.done() { let _ = tx.rollback().await; }
        });
        // 分批查询，搜索命中数不受 SQLite 占位符上限限制。
        let mut found = Vec::new();
        for chunk in ids.chunks(400) {
            let sql = format!("SELECT * FROM clip_record WHERE id IN ({})", chunk.iter().map(|_| "?").collect::<Vec<_>>().join(","));
            found.extend(load(&tx, &sql, chunk.iter().map(|id| to_value!(id)).collect()).await?);
        }
        tx.commit().await?;
        // 必须合并所有分批结果后排序，不能把每批当成独立页面。
        found.sort_by_key(|row| std::cmp::Reverse((row.pinned, row.sort_order, row.id)));
        Ok(found.into_iter().skip(offset as usize).take(limit as usize).collect())
    }
    /// 保存一次采集；写入顺序、内容去重及资源替换在同一事务完成。
    async fn save_capture(&self, record: &ClipRecord) -> AppResult<ClipRecord> {
        let tx = self.db.acquire_begin().await?;
        let tx=tx.defer_async(|tx|async move{if !tx.done(){if let Err(error)=tx.rollback().await{log::error!("回滚本地事务失败: {}",error);}}});
        // 写入顺序在事务内分配；首先写入避免读事务升级时的竞争。
        let mut next = record.clone();
        tx.exec("UPDATE clip_record SET sort_order=sort_order WHERE id=-1", vec![]).await?;
        #[derive(serde::Deserialize)] struct Sort { next: i64 }
        let rows: Vec<Sort> = tx.query_decode("SELECT COALESCE(MAX(sort_order),0)+1 AS next FROM clip_record", vec![]).await?;
        next.sort_order = rows[0].next;
        let id = persist_record(&tx, &next).await?;
        // 返回数据库实际分配的记录和资源 ID，不能返回尚未入库的输入对象。
        let result = load(&tx, "SELECT * FROM clip_record WHERE id=?", vec![to_value!(id)]).await?.remove(0);
        tx.commit().await?;
        Ok(result)
    }
    /// 切换唯一置顶；目标不存在或更新失败时，原置顶记录保持不变。
    async fn set_pinned(&self, id: i64, pinned: i32) -> AppResult<()> {
        if !matches!(pinned,0|1) { return Err(AppError::Config("置顶状态无效".into())); }
        let tx = self.db.acquire_begin().await?;
        let tx=tx.defer_async(|tx|async move{if !tx.done(){if let Err(error)=tx.rollback().await{log::error!("回滚本地事务失败: {}",error);}}});
        tx.exec("UPDATE clip_record SET sort_order=sort_order WHERE id=-1", vec![]).await?;
        if load(&tx,"SELECT * FROM clip_record WHERE id=?",vec![to_value!(id)]).await?.is_empty() { return Err(AppError::Config("记录不存在".into())); }
        // 先确认目标存在，再释放旧置顶，整个过程仍在同一事务内。
        if pinned == 1 { tx.exec("UPDATE clip_record SET pinned=0 WHERE pinned=1",vec![]).await?; }
        tx.exec("UPDATE clip_record SET pinned=? WHERE id=?",vec![to_value!(pinned),to_value!(id)]).await?;
        tx.commit().await?; Ok(())
    }
    /// 物理删除并返回删除前快照，供提交后回收缓存；重复删除返回 None。
    async fn delete(&self, id: i64) -> AppResult<Option<ClipRecord>> {
        let tx = self.db.acquire_begin().await?;
        let tx=tx.defer_async(|tx|async move{if !tx.done(){if let Err(error)=tx.rollback().await{log::error!("回滚本地事务失败: {}",error);}}});
        tx.exec("UPDATE clip_record SET sort_order=sort_order WHERE id=-1",vec![]).await?;
        let record = load(&tx,"SELECT * FROM clip_record WHERE id=?",vec![to_value!(id)]).await?.into_iter().next();
        // 资源行由 ON DELETE CASCADE 删除，磁盘文件交给应用层处理。
        tx.exec("DELETE FROM clip_record WHERE id=?",vec![to_value!(id)]).await?;
        tx.commit().await?; Ok(record)
    }
    /// 统计有效本地记录；本表没有逻辑删除状态。
    async fn count(&self) -> AppResult<i64> {
        #[derive(serde::Deserialize)] struct Count { count: i64 }
        let rows: Vec<Count> = self.db.query_decode("SELECT COUNT(*) AS count FROM clip_record",vec![]).await?;
        Ok(rows[0].count)
    }
    /// 保留最新的指定条数并保护置顶记录，返回被淘汰记录的资源快照。
    async fn remove_overflow(&self, max_records: u32) -> AppResult<Vec<ClipRecord>> {
        let tx = self.db.acquire_begin().await?;
        let tx=tx.defer_async(|tx|async move{if !tx.done(){if let Err(error)=tx.rollback().await{log::error!("回滚本地事务失败: {}",error);}}});
        tx.exec("UPDATE clip_record SET sort_order=sort_order WHERE id=-1",vec![]).await?;
        let rows = load(&tx,"SELECT * FROM clip_record ORDER BY pinned DESC,sort_order DESC,id DESC LIMIT -1 OFFSET ?",vec![to_value!(max_records)]).await?;
        let rows: Vec<_> = rows.into_iter().filter(|row| !row.is_pinned()).collect();
        for row in &rows { tx.exec("DELETE FROM clip_record WHERE id=?",vec![to_value!(row.id)]).await?; }
        tx.commit().await?; Ok(rows)
    }
}

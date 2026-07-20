use async_trait::async_trait;

use crate::{domain::clip::ClipRecord, errors::AppResult};

#[async_trait]
/// 剪贴记录仓储接口，隔离 service 与具体数据库、SQL 和事务实现。
pub trait ClipRecordRepository: Send + Sync {
    /// 按记录 ID 查询单条记录；不存在时返回 `None`。
    async fn find_by_id(&self, id: &str) -> AppResult<Option<ClipRecord>>;

    /// 按置顶、排序值和创建时间分页查询有效记录。
    async fn list(&self, limit: i32, offset: i32) -> AppResult<Vec<ClipRecord>>;

    /// 在搜索引擎给出的 ID 范围内分页查询有效记录。
    async fn list_by_ids(
        &self,
        ids: &[String],
        limit: i32,
        offset: i32,
    ) -> AppResult<Vec<ClipRecord>>;

    /// 更新记录置顶状态，并保证同一时间只有一条记录置顶。
    async fn set_pinned(&self, id: &str, pinned_flag: i32) -> AppResult<()>;

    /// 批量逻辑删除记录，并将同步状态重置为待同步。
    async fn mark_deleted(&self, ids: &[String]) -> AppResult<()>;
}

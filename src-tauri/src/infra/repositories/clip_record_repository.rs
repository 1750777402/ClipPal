use async_trait::async_trait;

use crate::{biz::clip_record::ClipRecord, errors::AppResult};

#[async_trait]
pub trait ClipRecordRepository: Send + Sync {
    async fn find_by_id(&self, id: &str) -> AppResult<Option<ClipRecord>>;

    async fn list(&self, limit: i32, offset: i32) -> AppResult<Vec<ClipRecord>>;

    async fn list_by_ids(
        &self,
        ids: &[String],
        limit: i32,
        offset: i32,
    ) -> AppResult<Vec<ClipRecord>>;

    async fn set_pinned(&self, id: &str, pinned_flag: i32) -> AppResult<()>;

    async fn mark_deleted(&self, ids: &[String]) -> AppResult<()>;
}

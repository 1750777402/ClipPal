use async_trait::async_trait;
use rbatis::RBatis;

use crate::{
    biz::clip_record::ClipRecord,
    errors::{AppError, AppResult},
    infra::repositories::ClipRecordRepository,
};

pub struct SqliteClipRecordRepository<'a> {
    db: &'a RBatis,
}

impl<'a> SqliteClipRecordRepository<'a> {
    pub fn new(db: &'a RBatis) -> Self {
        Self { db }
    }
}

#[async_trait]
impl ClipRecordRepository for SqliteClipRecordRepository<'_> {
    async fn find_by_id(&self, id: &str) -> AppResult<Option<ClipRecord>> {
        ClipRecord::select_by_id(self.db, id)
            .await
            .map(|records| records.into_iter().next())
            .map_err(AppError::Database)
    }

    async fn list(&self, limit: i32, offset: i32) -> AppResult<Vec<ClipRecord>> {
        ClipRecord::select_order_by_limit(self.db, limit, offset)
            .await
            .map_err(AppError::Database)
    }

    async fn list_by_ids(
        &self,
        ids: &[String],
        limit: i32,
        offset: i32,
    ) -> AppResult<Vec<ClipRecord>> {
        let ids = ids.to_vec();
        ClipRecord::select_by_ids(self.db, &ids, limit, offset)
            .await
            .map_err(AppError::Database)
    }

    async fn set_pinned(&self, id: &str, pinned_flag: i32) -> AppResult<()> {
        ClipRecord::update_pinned(self.db, id, pinned_flag).await
    }

    async fn mark_deleted(&self, ids: &[String]) -> AppResult<()> {
        ClipRecord::update_del_by_ids(self.db, &ids.to_vec()).await
    }
}

use async_trait::async_trait;
use rbatis::RBatis;
use rbs::to_value;

use crate::{
    domain::clip::ClipRecord,
    errors::{AppError, AppResult},
    infra::repositories::ClipRecordRepository,
};

/// 基于 RBatis/SQLite 的剪贴记录仓储实现。
pub struct SqliteClipRecordRepository {
    db: RBatis,
}

impl SqliteClipRecordRepository {
    /// 保存共享 RBatis 连接句柄；RBatis clone 复用同一连接池而不是新建数据库连接。
    pub fn new(db: RBatis) -> Self {
        Self { db }
    }
}

#[async_trait]
impl ClipRecordRepository for SqliteClipRecordRepository {
    /// 使用主键查询记录，并把空结果转换为 `None`。
    async fn find_by_id(&self, id: &str) -> AppResult<Option<ClipRecord>> {
        self.db
            .query_decode(
                "SELECT * FROM clip_record WHERE id = ?",
                vec![to_value!(id)],
            )
            .await
            .map(|records: Vec<ClipRecord>| records.into_iter().next())
            .map_err(AppError::Database)
    }

    /// 查询未删除记录，并按置顶、排序值、创建时间依次降序排列。
    async fn list(&self, limit: i32, offset: i32) -> AppResult<Vec<ClipRecord>> {
        self.db
            .query_decode(
                "SELECT * FROM clip_record WHERE del_flag = 0 ORDER BY pinned_flag DESC, sort DESC, created DESC LIMIT ? OFFSET ?",
                vec![to_value!(limit), to_value!(offset)],
            )
            .await
            .map_err(AppError::Database)
    }

    /// 在搜索结果 ID 集合中分页查询记录；空 ID 集合直接返回，避免生成无效 SQL。
    async fn list_by_ids(
        &self,
        ids: &[String],
        limit: i32,
        offset: i32,
    ) -> AppResult<Vec<ClipRecord>> {
        if ids.is_empty() {
            return Ok(Vec::new());
        }

        // 根据 ID 数量生成等量占位符，具体值仍通过参数绑定避免 SQL 注入。
        let sql = format!(
            "SELECT * FROM clip_record WHERE id IN ({}) AND del_flag = 0 ORDER BY pinned_flag DESC, sort DESC, created DESC LIMIT ? OFFSET ?",
            ids.iter().map(|_| "?").collect::<Vec<_>>().join(",")
        );
        let mut params = ids.iter().map(|id| to_value!(id)).collect::<Vec<_>>();
        params.push(to_value!(limit));
        params.push(to_value!(offset));

        self.db
            .query_decode(&sql, params)
            .await
            .map_err(AppError::Database)
    }

    /// 在事务中更新置顶状态；置顶新记录前先取消其他记录的置顶标记。
    async fn set_pinned(&self, id: &str, pinned_flag: i32) -> AppResult<()> {
        let tx = self.db.acquire_begin().await?;
        // 业务规则要求置顶互斥，只在设置为置顶时执行批量取消。
        if pinned_flag == 1 {
            let _ = tx
                .exec(
                    "UPDATE clip_record SET pinned_flag = 0 WHERE pinned_flag = 1",
                    vec![],
                )
                .await;
        }
        // 同步递增版本号，确保后续云同步能够识别本地状态变化。
        let _ = tx
            .exec(
                "UPDATE clip_record SET pinned_flag = ?, version = IFNULL(version, 0) + 1 WHERE id = ?",
                vec![to_value!(pinned_flag), to_value!(id)],
            )
            .await;
        tx.commit()
            .await
            .map_err(|error| AppError::Database(rbatis::Error::from(error)))
    }

    /// 在事务中批量逻辑删除记录，并将删除动作标记为待同步。
    async fn mark_deleted(&self, ids: &[String]) -> AppResult<()> {
        // ID 只用于生成占位符数量，所有值通过绑定参数传入。
        let sql = format!(
            "UPDATE clip_record SET del_flag = 1, sync_flag = 0 WHERE id IN ({})",
            ids.iter().map(|_| "?").collect::<Vec<_>>().join(",")
        );
        let params = ids.iter().map(|id| to_value!(id)).collect::<Vec<_>>();
        let tx = self.db.acquire_begin().await?;
        tx.exec(&sql, params).await?;
        tx.commit()
            .await
            .map_err(|error| AppError::Database(rbatis::Error::from(error)))
    }

    async fn list_skipped(&self, sync_flag: i32, skip_type: i32) -> AppResult<Vec<ClipRecord>> {
        self.db
            .query_decode(
                "SELECT * FROM clip_record WHERE sync_flag = ? AND skip_type = ? AND del_flag = 0",
                vec![to_value!(sync_flag), to_value!(skip_type)],
            )
            .await
            .map_err(AppError::Database)
    }

    async fn update_sync_state(
        &self,
        id: &str,
        sync_flag: i32,
        skip_type: Option<i32>,
    ) -> AppResult<()> {
        let tx = self.db.acquire_begin().await?;
        match skip_type {
            Some(skip_type) => {
                tx.exec(
                    "UPDATE clip_record SET sync_flag = ?, skip_type = ?, version = IFNULL(version, 0) + 1 WHERE id = ?",
                    vec![to_value!(sync_flag), to_value!(skip_type), to_value!(id)],
                )
                .await?;
            }
            None => {
                tx.exec(
                    "UPDATE clip_record SET sync_flag = ?, skip_type = NULL, version = IFNULL(version, 0) + 1 WHERE id = ?",
                    vec![to_value!(sync_flag), to_value!(id)],
                )
                .await?;
            }
        }
        tx.commit()
            .await
            .map_err(|error| AppError::Database(rbatis::Error::from(error)))
    }

    async fn count_active(&self) -> AppResult<i64> {
        #[derive(serde::Deserialize)]
        struct CountResult {
            count: i64,
        }

        self.db
            .query_decode(
                "SELECT COUNT(*) AS count FROM clip_record WHERE del_flag = 0",
                vec![],
            )
            .await
            .map(|rows: Vec<CountResult>| rows.first().map(|row| row.count).unwrap_or(0))
            .map_err(AppError::Database)
    }

    async fn delete_oldest_unpinned(&self, count: i32) -> AppResult<()> {
        self.db
            .exec(
                "DELETE FROM clip_record WHERE id IN (
                    SELECT id FROM clip_record
                    WHERE del_flag = 0 AND pinned_flag = 0
                    ORDER BY sort ASC, created ASC
                    LIMIT ?
                )",
                vec![to_value!(count)],
            )
            .await
            .map(|_| ())
            .map_err(AppError::Database)
    }
}

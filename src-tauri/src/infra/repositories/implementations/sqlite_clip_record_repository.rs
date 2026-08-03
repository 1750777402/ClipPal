use async_trait::async_trait;
use rbatis::{crud, RBatis};
use rbs::to_value;

use crate::{
    domain::clip::ClipRecord,
    errors::{AppError, AppResult},
    infra::repositories::ClipRecordRepository,
};

// RBatis 生成的基础插入能力只在 SQLite 仓储实现和暂留的云端合并事务中使用。
crud!(ClipRecord {}, "clip_record");

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
    /// 加载数据库中的全部记录，供启动阶段重建搜索索引。
    async fn list_all(&self) -> AppResult<Vec<ClipRecord>> {
        self.db
            .query_decode(
                "SELECT * FROM clip_record ORDER BY sort DESC, created DESC",
                vec![],
            )
            .await
            .map_err(AppError::Database)
    }

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

    /// 使用类型和 MD5 查询任意状态的一条记录，保持现有去重行为。
    async fn find_by_type_and_md5(
        &self,
        record_type: &str,
        md5: &str,
    ) -> AppResult<Option<ClipRecord>> {
        self.db
            .query_decode(
                "SELECT * FROM clip_record WHERE type = ? AND md5_str = ? LIMIT 1",
                vec![to_value!(record_type), to_value!(md5)],
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

    /// 查询指定同步状态且内容有效的记录。
    async fn list_by_sync_status(&self, sync_flag: i32) -> AppResult<Vec<ClipRecord>> {
        self.db
            .query_decode(
                "SELECT * FROM clip_record WHERE sync_flag = ? AND content IS NOT NULL ORDER BY created DESC",
                vec![to_value!(sync_flag)],
            )
            .await
            .map_err(AppError::Database)
    }

    /// 查询指定同步状态和来源的有限数量记录。
    async fn list_by_sync_status_and_source(
        &self,
        sync_flag: i32,
        cloud_source: i32,
        limit: i32,
    ) -> AppResult<Vec<ClipRecord>> {
        self.db
            .query_decode(
                "SELECT * FROM clip_record WHERE sync_flag = ? AND cloud_source = ? ORDER BY created DESC LIMIT ?",
                vec![
                    to_value!(sync_flag),
                    to_value!(cloud_source),
                    to_value!(limit),
                ],
            )
            .await
            .map_err(AppError::Database)
    }

    /// 查询可以物理清理的已同步墓碑记录。
    async fn list_synced_tombstones(&self) -> AppResult<Vec<ClipRecord>> {
        self.db
            .query_decode(
                "SELECT * FROM clip_record WHERE sync_flag = 2 AND del_flag = 1",
                vec![],
            )
            .await
            .map_err(AppError::Database)
    }

    /// 通过 RBatis 生成的参数化插入语句保存完整记录。
    async fn insert(&self, record: &ClipRecord) -> AppResult<()> {
        ClipRecord::insert(&self.db, record)
            .await
            .map(|_| ())
            .map_err(AppError::Database)
    }

    /// 在事务内根据创建时间腾出排序位置并插入记录。
    async fn insert_by_created_sort(&self, mut record: ClipRecord) -> AppResult<()> {
        let tx = self.db.acquire_begin().await?;
        let next_records: Vec<ClipRecord> = self
            .db
            .query_decode(
                "SELECT * FROM clip_record WHERE created >= ? ORDER BY created DESC LIMIT 1",
                vec![to_value!(record.created)],
            )
            .await?;

        if let Some(next_record) = next_records.first() {
            tx.exec(
                "UPDATE clip_record SET sort = IFNULL(sort, 0) + 1 WHERE created >= ?",
                vec![to_value!(next_record.created)],
            )
            .await?;
            record.sort = next_record.sort;
        } else {
            record.sort = self.next_sort().await?;
        }

        ClipRecord::insert(&tx, &record).await?;
        tx.commit()
            .await
            .map_err(|error| AppError::Database(rbatis::Error::from(error)))
    }

    /// 读取当前最大排序值并返回下一可用值。
    async fn next_sort(&self) -> AppResult<i32> {
        #[derive(serde::Deserialize)]
        struct SortResult {
            sort: i32,
        }

        self.db
            .query_decode(
                "SELECT sort FROM clip_record ORDER BY sort DESC, created DESC LIMIT 1",
                vec![],
            )
            .await
            .map(|records: Vec<SortResult>| {
                records.first().map(|record| record.sort + 1).unwrap_or(0)
            })
            .map_err(AppError::Database)
    }

    /// 在事务中更新记录内容。
    async fn update_content(&self, id: &str, content: &str) -> AppResult<()> {
        let tx = self.db.acquire_begin().await?;
        tx.exec(
            "UPDATE clip_record SET content = ? WHERE id = ?",
            vec![to_value!(content), to_value!(id)],
        )
        .await?;
        tx.commit()
            .await
            .map_err(|error| AppError::Database(rbatis::Error::from(error)))
    }

    /// 更新排序值并递增记录版本。
    async fn update_sort(&self, id: &str, sort: i32) -> AppResult<()> {
        let tx = self.db.acquire_begin().await?;
        tx.exec(
            "UPDATE clip_record SET sort = ?, version = IFNULL(version, 0) + 1 WHERE id = ?",
            vec![to_value!(sort), to_value!(id)],
        )
        .await?;
        tx.commit()
            .await
            .map_err(|error| AppError::Database(rbatis::Error::from(error)))
    }

    /// 更新记录关联的本地文件路径。
    async fn update_local_file_path(&self, id: &str, local_path: &str) -> AppResult<()> {
        let tx = self.db.acquire_begin().await?;
        tx.exec(
            "UPDATE clip_record SET local_file_path = ? WHERE id = ?",
            vec![to_value!(local_path), to_value!(id)],
        )
        .await?;
        tx.commit()
            .await
            .map_err(|error| AppError::Database(rbatis::Error::from(error)))
    }

    /// 更新云端文件下载完成后的本地资源信息和同步状态。
    async fn update_after_cloud_download(
        &self,
        id: &str,
        filename: &str,
        absolute_path: &str,
    ) -> AppResult<()> {
        let tx = self.db.acquire_begin().await?;
        tx.exec(
            "UPDATE clip_record SET content = ?, local_file_path = ?, sync_flag = 2 WHERE id = ?",
            vec![to_value!(filename), to_value!(absolute_path), to_value!(id)],
        )
        .await?;
        tx.commit()
            .await
            .map_err(|error| AppError::Database(rbatis::Error::from(error)))
    }

    /// 用新内容覆盖已删除记录，同时保留原记录 ID。
    async fn restore_deleted(&self, id: &str, record: &ClipRecord) -> AppResult<()> {
        let tx = self.db.acquire_begin().await?;
        tx.exec(
            "UPDATE clip_record SET type = ?, content = ?, md5_str = ?, local_file_path = ?, created = ?, os_type = ?, sort = ?, pinned_flag = ?, sync_flag = ?, sync_time = ?, device_id = ?, version = ?, del_flag = ?, cloud_source = ? WHERE id = ?",
            vec![
                to_value!(&record.r#type),
                to_value!(&record.content),
                to_value!(&record.md5_str),
                to_value!(&record.local_file_path),
                to_value!(record.created),
                to_value!(&record.os_type),
                to_value!(record.sort),
                to_value!(record.pinned_flag),
                to_value!(&record.sync_flag),
                to_value!(&record.sync_time),
                to_value!(&record.device_id),
                to_value!(&record.version),
                to_value!(&record.del_flag),
                to_value!(&record.cloud_source),
                to_value!(id),
            ],
        )
        .await?;
        tx.commit()
            .await
            .map_err(|error| AppError::Database(rbatis::Error::from(error)))
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

    /// 将云端墓碑批量应用到本地记录。
    async fn apply_remote_deletions(&self, ids: &[String], sync_time: u64) -> AppResult<()> {
        if ids.is_empty() {
            return Ok(());
        }
        let sql = format!(
            "UPDATE clip_record SET del_flag = 1, sync_flag = 2, sync_time = ? WHERE id IN ({})",
            ids.iter().map(|_| "?").collect::<Vec<_>>().join(",")
        );
        let mut params = vec![to_value!(sync_time)];
        params.extend(ids.iter().map(|id| to_value!(id)));
        let tx = self.db.acquire_begin().await?;
        tx.exec(&sql, params).await?;
        tx.commit()
            .await
            .map_err(|error| AppError::Database(rbatis::Error::from(error)))
    }

    /// 在事务中物理删除指定记录。
    async fn delete_permanently(&self, ids: &[String]) -> AppResult<()> {
        if ids.is_empty() {
            return Ok(());
        }
        let sql = format!(
            "DELETE FROM clip_record WHERE id IN ({})",
            ids.iter().map(|_| "?").collect::<Vec<_>>().join(",")
        );
        let params = ids.iter().map(|id| to_value!(id)).collect::<Vec<_>>();
        let tx = self.db.acquire_begin().await?;
        tx.exec(&sql, params).await?;
        tx.commit()
            .await
            .map_err(|error| AppError::Database(rbatis::Error::from(error)))
    }

    /// 批量写入同步状态和服务端同步时间。
    async fn update_sync_status(
        &self,
        ids: &[String],
        sync_flag: i32,
        sync_time: u64,
    ) -> AppResult<()> {
        if ids.is_empty() {
            return Ok(());
        }
        let sql = format!(
            "UPDATE clip_record SET sync_flag = ?, sync_time = ? WHERE id IN ({})",
            ids.iter().map(|_| "?").collect::<Vec<_>>().join(",")
        );
        let mut params = vec![to_value!(sync_flag), to_value!(sync_time)];
        params.extend(ids.iter().map(|id| to_value!(id)));
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

    /// 统计可以物理清理的已同步数量。
    async fn count_synced_tombstones(&self) -> AppResult<i64> {
        #[derive(serde::Deserialize)]
        struct CountResult {
            count: i64,
        }

        self.db
            .query_decode(
                "SELECT COUNT(*) AS count FROM clip_record WHERE del_flag = 1 AND sync_flag = 2",
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

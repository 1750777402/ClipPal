use async_trait::async_trait;

use crate::{domain::clip::ClipRecord, errors::AppResult};

#[async_trait]
/// 剪贴记录仓储接口，隔离 service 与具体数据库、SQL 和事务实现。
pub trait ClipRecordRepository: Send + Sync {
    /// 查询全部记录并按排序值和创建时间倒序返回，用于启动时重建搜索索引。
    async fn list_all(&self) -> AppResult<Vec<ClipRecord>>;

    /// 按记录 ID 查询单条记录；不存在时返回 `None`。
    async fn find_by_id(&self, id: &str) -> AppResult<Option<ClipRecord>>;

    /// 按记录类型和内容摘要查询一条记录，用于剪贴板内容去重。
    async fn find_by_type_and_md5(
        &self,
        record_type: &str,
        md5: &str,
    ) -> AppResult<Option<ClipRecord>>;

    /// 按置顶、排序值和创建时间分页查询有效记录。
    async fn list(&self, limit: i32, offset: i32) -> AppResult<Vec<ClipRecord>>;

    /// 在搜索引擎给出的 ID 范围内分页查询有效记录。
    async fn list_by_ids(
        &self,
        ids: &[String],
        limit: i32,
        offset: i32,
    ) -> AppResult<Vec<ClipRecord>>;

    /// 查询指定同步状态的全部记录，并按创建时间倒序返回。
    async fn list_by_sync_status(&self, sync_flag: i32) -> AppResult<Vec<ClipRecord>>;

    /// 按同步状态和记录来源查询有限数量的记录。
    async fn list_by_sync_status_and_source(
        &self,
        sync_flag: i32,
        cloud_source: i32,
        limit: i32,
    ) -> AppResult<Vec<ClipRecord>>;

    /// 查询已经同步完成且被逻辑删除、可以物理清理的记录。
    async fn list_synced_tombstones(&self) -> AppResult<Vec<ClipRecord>>;

    /// 插入一条完整剪贴记录。
    async fn insert(&self, record: &ClipRecord) -> AppResult<()>;

    /// 按记录产生时间计算排序位置并插入记录。
    #[allow(dead_code)]
    async fn insert_by_created_sort(&self, record: ClipRecord) -> AppResult<()>;

    /// 获取下一条新记录使用的排序值。
    async fn next_sort(&self) -> AppResult<i32>;

    /// 更新记录内容。
    #[allow(dead_code)]
    async fn update_content(&self, id: &str, content: &str) -> AppResult<()>;

    /// 更新记录排序并递增版本号。
    async fn update_sort(&self, id: &str, sort: i32) -> AppResult<()>;

    /// 更新记录对应的本地文件路径。
    #[allow(dead_code)]
    async fn update_local_file_path(&self, id: &str, local_path: &str) -> AppResult<()>;

    /// 写入云端资源下载后的文件名、本地路径和同步状态。
    async fn update_after_cloud_download(
        &self,
        id: &str,
        filename: &str,
        absolute_path: &str,
    ) -> AppResult<()>;

    /// 使用新记录数据恢复一条已经逻辑删除的同内容记录。
    async fn restore_deleted(&self, id: &str, record: &ClipRecord) -> AppResult<()>;

    /// 更新记录置顶状态，并保证同一时间只有一条记录置顶。
    async fn set_pinned(&self, id: &str, pinned_flag: i32) -> AppResult<()>;

    /// 批量逻辑删除记录，并将同步状态重置为待同步。
    async fn mark_deleted(&self, ids: &[String]) -> AppResult<()>;

    /// 将云端删除操作应用到本地，并标记为已经同步。
    async fn apply_remote_deletions(&self, ids: &[String], sync_time: u64) -> AppResult<()>;

    /// 从数据库物理删除指定记录。
    async fn delete_permanently(&self, ids: &[String]) -> AppResult<()>;

    /// 批量更新记录同步状态和同步时间。
    async fn update_sync_status(
        &self,
        ids: &[String],
        sync_flag: i32,
        sync_time: u64,
    ) -> AppResult<()>;

    /// 查询因指定原因跳过同步的有效记录。
    async fn list_skipped(&self, sync_flag: i32, skip_type: i32) -> AppResult<Vec<ClipRecord>>;

    /// 更新单条记录的同步状态和跳过原因。
    async fn update_sync_state(
        &self,
        id: &str,
        sync_flag: i32,
        skip_type: Option<i32>,
    ) -> AppResult<()>;

    /// 统计未逻辑删除的记录数。
    async fn count_active(&self) -> AppResult<i64>;

    /// 统计已经同步完成且被逻辑删除的记录数。
    async fn count_synced_tombstones(&self) -> AppResult<i64>;

    /// 删除最旧的非置顶有效记录。
    async fn delete_oldest_unpinned(&self, count: i32) -> AppResult<()>;
}

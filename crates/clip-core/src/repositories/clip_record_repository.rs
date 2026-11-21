use async_trait::async_trait;
use crate::{domain::clip::ClipRecord, errors::AppResult};

/// 单机仓储只暴露本地记录及资源操作，不包含远程同步状态。
#[async_trait]
pub trait ClipRecordRepository: Send + Sync {
    /// 读取全部记录及其资源，供启动重建索引等后台操作使用。
    async fn list_all(&self) -> AppResult<Vec<ClipRecord>>;
    /// 按本地主键定位；记录已删除时返回 None。
    async fn find_by_id(&self, id: i64) -> AppResult<Option<ClipRecord>>;
    /// 按内容身份查重，不要求调用方知道已有记录的主键。
    async fn find_by_content_key(&self, key: &str) -> AppResult<Option<ClipRecord>>;
    /// 按置顶和最近采集顺序分页，offset 是要跳过的记录条数。
    async fn list(&self, limit: i32, offset: i64) -> AppResult<Vec<ClipRecord>>;
    /// 回查搜索候选 ID，再按正常列表顺序分页，过滤已删除记录。
    async fn list_by_ids(&self, ids: &[String], limit: i32, offset: i64) -> AppResult<Vec<ClipRecord>>;
    /// 事务内分配排序、按内容去重并替换资源。重复复制保留 ID、首次时间和置顶。
    async fn save_capture(&self, record: &ClipRecord) -> AppResult<ClipRecord>;
    /// 设置唯一置顶；只接受 0/1，更新失败必须恢复原置顶状态。
    async fn set_pinned(&self, id: i64, pinned: i32) -> AppResult<()>;
    /// 返回已删除的完整快照，提交后由资源管理器回收受管缓存。
    async fn delete(&self, id: i64) -> AppResult<Option<ClipRecord>>;
    /// 统计本机当前实际保留的记录数。
    async fn count(&self) -> AppResult<i64>;
    /// 淘汰超过保留数量的旧记录并返回快照，保护置顶记录。
    async fn remove_overflow(&self, max_records: u32) -> AppResult<Vec<ClipRecord>>;
}

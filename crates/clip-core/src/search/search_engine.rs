use async_trait::async_trait;

use crate::{domain::clip::ClipRecord, errors::AppResult};

#[async_trait]
/// 剪贴记录搜索引擎接口，隔离 service 与当前内存索引实现。
pub trait SearchEngine: Send + Sync {
    /// 根据查询文本返回候选记录 ID 列表。
    async fn search(&self, query: &str) -> Vec<String>;

    /// 添加或替换一条已完成内容转换的记录。
    async fn add(&self, id: &str, content: &str) -> AppResult<()>;

    /// 从索引中批量移除记录 ID，使索引状态与数据库保持一致。
    async fn remove(&self, ids: &[String]) -> AppResult<()>;

    /// 使用数据库中的完整记录快照重建索引。
    async fn initialize(&self, records: Vec<ClipRecord>) -> AppResult<()>;
}

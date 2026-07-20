use async_trait::async_trait;

use crate::errors::AppResult;

#[async_trait]
/// 剪贴记录搜索引擎接口，隔离 service 与当前内存索引实现。
pub trait SearchEngine: Send + Sync {
    /// 根据查询文本返回候选记录 ID 列表。
    async fn search(&self, query: &str) -> Vec<String>;

    /// 从索引中批量移除记录 ID，使索引状态与数据库保持一致。
    async fn remove(&self, ids: &[String]) -> AppResult<()>;
}

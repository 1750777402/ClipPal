use async_trait::async_trait;

use crate::{
    biz::content_search::{remove_ids_from_index, search_ids_by_content},
    errors::AppResult,
    infra::search::SearchEngine,
};

#[derive(Default)]
/// 基于当前进程内布隆过滤器索引的搜索引擎实现。
pub struct InMemorySearchEngine;

#[async_trait]
impl SearchEngine for InMemorySearchEngine {
    /// 委托内存索引执行内容搜索并返回命中的记录 ID。
    async fn search(&self, query: &str) -> Vec<String> {
        search_ids_by_content(query).await
    }

    /// 从内存索引移除已删除记录。
    async fn remove(&self, ids: &[String]) -> AppResult<()> {
        remove_ids_from_index(ids).await
    }
}

mod search_engine;
mod in_memory;
pub use search_engine::SearchEngine;
pub use in_memory::InMemorySearchEngine;

use crate::errors::AppResult;

/// 解密策略由应用注入；核心索引无需知道密钥来源或部署方式。
pub trait TextDecoder: Send + Sync {
    fn decode(&self, text: &str) -> AppResult<String>;
}
impl<F> TextDecoder for F
where F: Fn(&str) -> AppResult<String> + Send + Sync {
    fn decode(&self, text: &str) -> AppResult<String> { self(text) }
}

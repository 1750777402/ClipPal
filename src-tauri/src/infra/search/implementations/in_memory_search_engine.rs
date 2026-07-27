use crate::domain::clip::ClipRecord;
use crate::domain::settings::{
    Settings, DEFAULT_BLOOM_FILTER_TRUST_THRESHOLD, DEFAULT_DIRECT_CONTAINS_THRESHOLD,
};
use crate::errors::AppResult;
use crate::infra::search::SearchEngine;
use crate::utils::lock_utils::lock_utils::safe_read_lock;
use async_trait::async_trait;
use bloomfilter::Bloom;
use clipboard_listener::ClipType;
use dashmap::DashMap;
use once_cell::sync::Lazy;
use regex::Regex;
use std::collections::HashSet;
use std::sync::{Arc, RwLock};

// 静态编译的正则表达式
static WORD_REGEX: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"\b[a-z]{2,}\b|\b\d{2,}\b").expect("Valid word regex"));

static TAG_REGEX: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"</?([a-z][a-z0-9]*)\b").expect("Valid tag regex"));

static ATTR_REGEX: Lazy<Regex> =
    Lazy::new(|| Regex::new(r#"(\w+)=["']([^"']*)["']"#).expect("Valid attribute regex"));

/// 搜索索引配置
const BLOOM_FILTER_ITEMS: usize = 1000; // 每个记录预期的词汇数量
const BLOOM_FILTER_FP_RATE: f64 = 0.01; // 1%的误报率

/// 记录搜索结构 - 每条记录独立维护
#[derive(Debug)]
struct RecordSearchData {
    /// 记录的原始内容（解密后）
    content: String,
    /// 该记录的bloom filter
    bloom_filter: Bloom<String>,
}

impl RecordSearchData {
    fn new(content: String) -> Self {
        let mut bloom_filter =
            Bloom::new_for_fp_rate(BLOOM_FILTER_ITEMS, BLOOM_FILTER_FP_RATE).unwrap();

        // 将内容的所有可能搜索词汇添加到bloom filter
        let search_terms = Self::extract_search_terms(&content);
        log::debug!(
            "为记录内容创建布隆过滤器 - 内容{}, \n分词结果: {:?}, ",
            content,
            search_terms
        );
        for term in search_terms {
            bloom_filter.set(&term);
        }

        Self {
            content,
            bloom_filter,
        }
    }

    /// 混合 n-gram 滑动窗口 + 空格分词的内容分词方法
    pub fn extract_search_terms(text: &str) -> HashSet<String> {
        let mut tokens = HashSet::new();
        let cleaned_text = Self::clean_text(text).to_lowercase();

        // ===== 1. 统一提取字母和数字序列 =====
        for cap in WORD_REGEX.find_iter(&cleaned_text) {
            tokens.insert(cap.as_str().to_string());
        }

        // ===== 2. 结构化内容处理 =====
        if text.contains('<') && text.contains('>') {
            Self::extract_xml_tokens(text, &mut tokens);
        }

        // ===== 3. 中文n-gram处理 =====
        Self::extract_cjk_ngrams(&cleaned_text, &mut tokens);

        // ===== 4. 空格分词补充 =====
        for word in cleaned_text.split_whitespace() {
            if word.len() >= 2 && !tokens.contains(word) {
                tokens.insert(word.to_string());
            }
        }

        tokens
    }

    // XML/HTML标签处理（独立函数）
    fn extract_xml_tokens(text: &str, tokens: &mut HashSet<String>) {
        for cap in TAG_REGEX.captures_iter(text) {
            if let Some(tag) = cap.get(1) {
                tokens.insert(tag.as_str().to_string());
            }
        }

        for cap in ATTR_REGEX.captures_iter(text) {
            if let Some(name) = cap.get(1) {
                tokens.insert(name.as_str().to_string());
            }
            if let Some(value) = cap.get(2) {
                let val = value.as_str().to_lowercase();
                if val.len() >= 2 {
                    tokens.insert(val.clone());

                    // 属性值分词
                    for word in val.split_whitespace() {
                        if word.len() >= 2 {
                            tokens.insert(word.to_string());
                        }
                    }
                }
            }
        }
    }

    // 中日韩n-gram处理
    fn extract_cjk_ngrams(text: &str, tokens: &mut HashSet<String>) {
        let cjk_text: String = text
            .chars()
            .filter(|&c| ('\u{4e00}'..='\u{9fff}').contains(&c))
            .collect();

        let chars: Vec<char> = cjk_text.chars().collect();
        let len = chars.len();

        for n in 2..=4 {
            if len < n {
                continue;
            }

            for i in 0..=(len - n) {
                let gram: String = chars[i..i + n].iter().collect();
                tokens.insert(gram);
            }
        }
    }

    // 清理文本（保留字母、数字、空格、汉字）
    fn clean_text(text: &str) -> String {
        text.chars()
            .filter(|&c| {
                c.is_alphabetic()
                    || c.is_numeric()
                    || c.is_whitespace()
                    || ('\u{4e00}'..='\u{9fff}').contains(&c)
            })
            .collect()
    }

    /// 布隆过滤器快速过滤 + 可选精确匹配
    fn smart_search(
        &self,
        query: &str,
        bloom_trust_threshold: usize,
        direct_contains_threshold: usize,
    ) -> bool {
        let normalized_query = query.trim().to_lowercase();
        let content_size = self.content.as_bytes().len();
        // 如果内容大小小于配置的direct_contains_threshold，直接使用contains搜索
        if content_size < direct_contains_threshold {
            log::debug!(
                "内容大小 {} 字节小于直接搜索阈值 {} 字节，使用直接contains搜索",
                content_size,
                direct_contains_threshold
            );
            return self.content_contains(&normalized_query);
        }

        // 查询内容分词（使用和索引一致的分词方式）
        let query_terms = Self::extract_search_terms(&normalized_query);

        // all_terms_in_bloom表示分词后的每个结果是否都在布隆过滤器中命中
        let all_terms_in_bloom = query_terms
            .iter()
            .all(|term| !term.is_empty() && self.bloom_filter.check(term));

        // 如果内容大小大于配置的bloom_trust_threshold，直接信任布隆过滤器结果
        if content_size >= bloom_trust_threshold {
            log::debug!(
                "内容大小 {} 字节超过阈值 {} 字节，信任布隆过滤器结果",
                content_size,
                bloom_trust_threshold
            );
            return all_terms_in_bloom;
        }
        // 所有关键词都未命中
        return self.content_contains(&normalized_query);
    }

    /// 内容包含搜索
    fn content_contains(&self, query: &str) -> bool {
        let normalized_content = self.content.to_lowercase();
        let normalized_query = query.to_lowercase();

        // 直接字符串包含搜索
        normalized_content.contains(&normalized_query)
    }
}

struct SimpleSearchIndex {
    records: DashMap<String, RecordSearchData>,
}

impl SimpleSearchIndex {
    fn new() -> Self {
        Self {
            records: DashMap::new(),
        }
    }

    /// 添加记录
    fn add_record(&self, id: &str, content: &str) {
        let search_data = RecordSearchData::new(content.to_string());
        self.records.insert(id.to_string(), search_data);
    }

    /// 移除记录
    fn remove_records(&self, ids: &[String]) {
        for id in ids {
            self.records.remove(id);
        }
    }

    /// 搜索包含指定内容的记录ID
    fn search(
        &self,
        query: &str,
        bloom_trust_threshold: usize,
        direct_contains_threshold: usize,
    ) -> Vec<String> {
        if query.is_empty() {
            return Vec::new();
        }

        let mut results = Vec::new();
        for entry in self.records.iter() {
            let (id, search_data) = (entry.key(), entry.value());
            // 布隆过滤器优先 + 内容包含搜索
            if search_data.smart_search(query, bloom_trust_threshold, direct_contains_threshold) {
                results.push(id.clone());
            }
        }

        results
    }

    /// 清空所有记录
    fn clear(&self) {
        self.records.clear();
    }

    /// 获取统计信息
    fn get_stats(&self) -> usize {
        self.records.len()
    }
}

/// 进程内搜索索引，拥有自己的索引数据并共享运行期设置缓存。
pub struct InMemorySearchEngine {
    index: SimpleSearchIndex,
    settings: Arc<RwLock<Settings>>,
}

impl InMemorySearchEngine {
    pub fn new(settings: Arc<RwLock<Settings>>) -> Self {
        Self {
            index: SimpleSearchIndex::new(),
            settings,
        }
    }

    fn thresholds(&self) -> (usize, usize) {
        safe_read_lock(&self.settings)
            .map(|settings| {
                (
                    settings
                        .bloom_filter_trust_threshold
                        .unwrap_or(DEFAULT_BLOOM_FILTER_TRUST_THRESHOLD),
                    settings
                        .direct_contains_threshold
                        .unwrap_or(DEFAULT_DIRECT_CONTAINS_THRESHOLD),
                )
            })
            .unwrap_or((
                DEFAULT_BLOOM_FILTER_TRUST_THRESHOLD,
                DEFAULT_DIRECT_CONTAINS_THRESHOLD,
            ))
    }
}

#[async_trait]
impl SearchEngine for InMemorySearchEngine {
    async fn search(&self, query: &str) -> Vec<String> {
        let (bloom_threshold, direct_threshold) = self.thresholds();
        self.index.search(query, bloom_threshold, direct_threshold)
    }

    async fn add(&self, id: &str, content: &str) -> AppResult<()> {
        self.index.add_record(id, content);
        log::debug!(
            "添加记录到搜索索引 - ID: {}, 内容长度: {}",
            id,
            content.len()
        );
        Ok(())
    }

    async fn remove(&self, ids: &[String]) -> AppResult<()> {
        if ids.is_empty() {
            return Ok(());
        }
        self.index.remove_records(ids);
        log::debug!("从搜索索引中删除 {} 个记录", ids.len());
        Ok(())
    }

    async fn initialize(&self, records: Vec<ClipRecord>) -> AppResult<()> {
        self.index.clear();
        let total_count = records.len();
        let mut indexed_count = 0;

        for record in records {
            let should_index = match record.r#type.as_str() {
                value if value == ClipType::Text.to_string() => record
                    .content
                    .as_str()
                    .and_then(
                        |content| match crate::utils::aes_util::decrypt_content(content) {
                            Ok(content) => {
                                self.index.add_record(&record.id, &content);
                                Some(())
                            }
                            Err(error) => {
                                log::warn!(
                                    "解密内容失败，跳过索引 - ID: {}, 错误: {}",
                                    record.id,
                                    error
                                );
                                None
                            }
                        },
                    )
                    .is_some(),
                value if value == ClipType::File.to_string() => record
                    .content
                    .as_str()
                    .map(|content| self.index.add_record(&record.id, content))
                    .is_some(),
                _ => false,
            };

            if should_index {
                indexed_count += 1;
            }
        }

        log::info!(
            "搜索索引初始化完成 - 总记录: {}, 已索引记录: {}, 当前索引记录数: {}",
            total_count,
            indexed_count,
            self.index.get_stats()
        );
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn adds_searches_and_removes_records() {
        let engine = InMemorySearchEngine::new(Arc::new(RwLock::new(Settings::default())));
        engine.add("record-1", "hello clipboard").await.unwrap();

        assert_eq!(engine.search("clipboard").await, vec!["record-1"]);

        engine.remove(&["record-1".to_string()]).await.unwrap();
        assert!(engine.search("clipboard").await.is_empty());
    }
}

use crate::biz::clip_record::ClipRecord;
use crate::biz::system_setting::{
    DEFAULT_BLOOM_FILTER_TRUST_THRESHOLD, DEFAULT_DIRECT_CONTAINS_THRESHOLD,
};
use crate::utils::lock_utils::lock_utils::safe_read_lock;
use crate::{CONTEXT, biz::system_setting::Settings};
use crate::errors::AppResult;
use bloomfilter::Bloom;
use dashmap::DashMap;
use once_cell::sync::Lazy;
use regex::Regex;
use std::collections::HashSet;
use std::sync::{Arc, RwLock};

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
        let mut bloom_filter = Bloom::new_for_fp_rate(BLOOM_FILTER_ITEMS, BLOOM_FILTER_FP_RATE);

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
        let word_regex = Regex::new(r"\b[a-z]{2,}\b|\b\d{2,}\b").unwrap();
        for cap in word_regex.find_iter(&cleaned_text) {
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
        let tag_regex = Regex::new(r"</?([a-z][a-z0-9]*)\b").unwrap();
        for cap in tag_regex.captures_iter(text) {
            if let Some(tag) = cap.get(1) {
                tokens.insert(tag.as_str().to_string());
            }
        }

        let attr_regex = Regex::new(r#"(\w+)=["']([^"']*)["']"#).unwrap();
        for cap in attr_regex.captures_iter(text) {
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
    fn smart_search(&self, query: &str) -> bool {
        // 获取配置
        let (bloom_trust_threshold, direct_contains_threshold) = {
            let lock = CONTEXT.get::<Arc<RwLock<Settings>>>().clone();
            let guard = safe_read_lock(&lock);
            match guard {
                Ok(settings) => {
                    let bloom_threshold = settings
                        .bloom_filter_trust_threshold
                        .unwrap_or(DEFAULT_BLOOM_FILTER_TRUST_THRESHOLD);
                    let direct_threshold = settings
                        .direct_contains_threshold
                        .unwrap_or(DEFAULT_DIRECT_CONTAINS_THRESHOLD);
                    (bloom_threshold, direct_threshold)
                }
                Err(_) => (
                    DEFAULT_BLOOM_FILTER_TRUST_THRESHOLD,
                    DEFAULT_DIRECT_CONTAINS_THRESHOLD,
                ),
            }
        };

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
    fn search(&self, query: &str) -> Vec<String> {
        if query.is_empty() {
            return Vec::new();
        }

        let mut results = Vec::new();
        for entry in self.records.iter() {
            let (id, search_data) = (entry.key(), entry.value());
            // 布隆过滤器优先 + 内容包含搜索
            if search_data.smart_search(query) {
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

// 全局搜索索引
static SEARCH_INDEX: Lazy<Arc<SimpleSearchIndex>> =
    Lazy::new(|| Arc::new(SimpleSearchIndex::new()));

/// 添加内容到搜索索引
pub async fn add_content_to_index(id: &str, content: &str) -> AppResult<()> {
    SEARCH_INDEX.add_record(id, content);
    log::debug!(
        "添加记录到搜索索引 - ID: {}, 内容长度: {}",
        id,
        content.len()
    );
    Ok(())
}

/// 根据内容搜索ID列表
pub async fn search_ids_by_content(content: &str) -> Vec<String> {
    SEARCH_INDEX.search(content)
}

/// 删除ID并更新索引
pub async fn remove_ids_from_index(ids: &[String]) -> AppResult<()> {
    if ids.is_empty() {
        return Ok(());
    }

    SEARCH_INDEX.remove_records(ids);
    log::debug!("从搜索索引中删除 {} 个记录", ids.len());
    Ok(())
}

/// 异步初始化搜索索引，从现有记录中构建
pub async fn initialize_search_index(clips: Vec<ClipRecord>) -> AppResult<()> {
    tokio::spawn(async move {
        // 清空现有索引
        SEARCH_INDEX.clear();

        let total_count = clips.len();
        let mut indexed_count = 0;

        // 处理记录
        for record in clips {
            let should_index = match record.r#type.as_str() {
                "Text" => {
                    if let Some(content) = record.content.as_str() {
                        // 解密文本内容
                        match crate::utils::aes_util::decrypt_content(content) {
                            Ok(decrypted_content) => {
                                SEARCH_INDEX.add_record(&record.id, &decrypted_content);
                                true
                            }
                            Err(e) => {
                                log::warn!(
                                    "解密内容失败，跳过索引 - ID: {}, 错误: {}",
                                    record.id,
                                    e
                                );
                                false
                            }
                        }
                    } else {
                        false
                    }
                }
                "File" => {
                    if let Some(file_paths) = record.content.as_str() {
                        SEARCH_INDEX.add_record(&record.id, file_paths);
                        true
                    } else {
                        false
                    }
                }
                _ => false, // 图片类型不参与搜索
            };

            if should_index {
                indexed_count += 1;
            }
        }

        let record_count = SEARCH_INDEX.get_stats();
        log::info!(
            "搜索索引初始化完成 - 总记录: {}, 已索引记录: {}, 当前索引记录数: {}",
            total_count,
            indexed_count,
            record_count
        );
    });

    Ok(())
}

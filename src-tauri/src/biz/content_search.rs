use crate::biz::clip_record::ClipRecord;
use crate::{CONTEXT, errors::lock_utils::safe_lock, biz::system_setting::Settings};
use anyhow::Result;
use bloomfilter::Bloom;
use dashmap::DashMap;
use once_cell::sync::Lazy;
use regex::Regex;
use std::sync::{Arc, Mutex};

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
        for term in search_terms {
            bloom_filter.set(&term);
        }
        
        Self {
            content,
            bloom_filter,
        }
    }
    
    /// 混合 n-gram 滑动窗口 + 空格分词的内容分词方法
    fn extract_search_terms(content: &str) -> Vec<String> {
        let mut terms = Vec::new();
        
        // 预处理：去除标点符号和特殊字符，只保留字母、数字、中文、空格
        let cleaned = Self::clean_content(content);
        let normalized = cleaned.to_lowercase();
        
        // 1. 空格分词 - 处理英文和数字
        for word in normalized.split_whitespace() {
            if Self::is_english_or_number(word) && word.len() >= 2 {
                terms.push(word.to_string());
            }
        }
        
        // 2. 中文 n-gram 滑动窗口分词 (仅2-3字，减少词汇数量)
        let chinese_text = Self::extract_chinese_only(&normalized);
        if !chinese_text.is_empty() {
            let chars: Vec<char> = chinese_text.chars().collect();
            
            // 只生成2字和3字n-gram，避免过多词汇
            // 2字 n-gram
            for i in 0..=chars.len().saturating_sub(2) {
                let bigram: String = chars[i..i+2].iter().collect();
                terms.push(bigram);
            }
            
            // 3字 n-gram (限制数量)
            if chars.len() >= 3 && chars.len() <= 10 {
                for i in 0..=chars.len().saturating_sub(3) {
                    let trigram: String = chars[i..i+3].iter().collect();
                    terms.push(trigram);
                }
            }
        }
        
        // 3. 添加清理后的完整内容
        terms.push(normalized.clone());
        
        // 去重
        terms.sort();
        terms.dedup();
        
        terms
    }
    
    /// 清理内容，去除标点符号和特殊字符
    fn clean_content(content: &str) -> String {
        let regex = Regex::new(r"[^\w\s\u4e00-\u9fff]").unwrap();
        regex.replace_all(content, " ").to_string()
    }
    
    /// 检查是否为英文或数字
    fn is_english_or_number(text: &str) -> bool {
        text.chars().all(|c| c.is_ascii_alphanumeric())
    }
    
    /// 提取纯中文内容
    fn extract_chinese_only(text: &str) -> String {
        text.chars().filter(|c| '\u{4e00}' <= *c && *c <= '\u{9fff}').collect()
    }
    
    /// 布隆过滤器快速过滤 + 可选精确匹配
    fn smart_search(&self, query: &str) -> bool {
        // 获取内容大小阈值配置，当内容大于此值时直接信任布隆过滤器
        let bloom_trust_threshold = {
            let lock = CONTEXT.get::<Arc<Mutex<Settings>>>().clone();
            let threshold = match safe_lock(&lock) {
                Ok(settings) => settings.bloom_filter_trust_threshold.unwrap_or(1 * 1024 * 1024),
                Err(_) => 1 * 1024 * 1024, // 默认2M
            };
            threshold
        };
        
        // 对于复杂查询，先用布隆过滤器快速过滤
        let query_terms = Self::extract_search_terms(query);
        
        // 检查关键词汇是否在布隆过滤器中
        let mut has_match = false;
        for term in &query_terms {
            if term.len() >= 2 && self.bloom_filter.check(term) {
                has_match = true;
                break;
            }
        }
        
        // 根据内容大小决定是否直接信任布隆过滤器
        if has_match {
            let content_size = self.content.as_bytes().len();
            if content_size > bloom_trust_threshold {
                // 内容大于阈值，直接信任布隆过滤器结果
                log::debug!("内容大小 {} 字节超过阈值 {} 字节，直接信任布隆过滤器结果", 
                          content_size, bloom_trust_threshold);
                return true;
            } else {
                // 内容小于阈值，进行精确搜索确认
                return self.content_contains(query);
            }
        }
        
        false
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
pub async fn add_content_to_index(id: &str, content: &str) -> Result<()> {
    SEARCH_INDEX.add_record(id, content);
    log::debug!("添加记录到搜索索引 - ID: {}, 内容长度: {}", id, content.len());
    Ok(())
}

/// 根据内容搜索ID列表
pub async fn search_ids_by_content(content: &str) -> Vec<String> {
    let start_time = std::time::Instant::now();
    let results = SEARCH_INDEX.search(content);
    let elapsed = start_time.elapsed();
    
    log::debug!(
        "搜索完成 - 查询: '{}', 结果数: {}, 耗时: {:?}",
        content, results.len(), elapsed
    );
    
    results
}

/// 删除ID并更新索引
pub async fn remove_ids_from_index(ids: &[String]) -> Result<()> {
    if ids.is_empty() {
        return Ok(());
    }

    SEARCH_INDEX.remove_records(ids);
    log::debug!("从搜索索引中删除 {} 个记录", ids.len());
    Ok(())
}

/// 异步初始化搜索索引，从现有记录中构建
pub async fn initialize_search_index(clips: Vec<ClipRecord>) -> Result<()> {
    log::info!("开始初始化搜索索引...");
    
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
                            log::warn!("解密内容失败，跳过索引 - ID: {}, 错误: {}", record.id, e);
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
        total_count, indexed_count, record_count
    );
    
    Ok(())
}

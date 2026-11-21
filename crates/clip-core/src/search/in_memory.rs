//! 每条记录维护独立内存索引，文本由注入的解密器读取，文件只索引显示名称。
use crate::domain::clip::ClipRecord;
use crate::domain::settings::{
    Settings, DEFAULT_BLOOM_FILTER_TRUST_THRESHOLD, DEFAULT_DIRECT_CONTAINS_THRESHOLD,
};
use crate::errors::AppResult;
use super::{SearchEngine, TextDecoder};
use async_trait::async_trait;
use bloomfilter::Bloom;
use crate::domain::clip::{TEXT_TYPE, FILE_TYPE};
use dashmap::DashMap;
use std::collections::HashSet;
use std::sync::{Arc, RwLock};

/// 搜索索引配置
const BLOOM_FILTER_FP_RATE: f64 = 0.01; // 1%的误报率
const MAX_CJK_NGRAM_SIZE: usize = 4;

/// 记录搜索结构 - 每条记录独立维护
#[derive(Debug)]
struct RecordSearchData {
    /// 记录的原始内容（解密后）
    content: String,
    /// 该记录的bloom filter
    bloom_filter: Bloom<String>,
}

impl RecordSearchData {
    /// 为一条明文或文件名建立分词集合和布隆过滤器，不保存数据库密文。
    fn new(content: String) -> Self {
        // 将内容的所有可能搜索词汇添加到bloom filter
        let search_terms = Self::extract_search_terms(&content);
        let mut bloom_filter =
            Bloom::new_for_fp_rate(search_terms.len().max(1), BLOOM_FILTER_FP_RATE).unwrap();
        log::debug!(
            "为记录内容创建布隆过滤器 - 内容长度: {}, 分词数量: {}",
            content.len(),
            search_terms.len()
        );
        for term in &search_terms {
            bloom_filter.set(term);
        }

        Self {
            content,
            bloom_filter,
        }
    }

    /// 字母数字按连续词切分，中日韩文字按连续区段生成 n-gram。
    pub fn extract_search_terms(text: &str) -> HashSet<String> {
        let mut tokens = HashSet::new();
        let mut word = String::new();
        let mut cjk_run = Vec::new();

        for original in text.chars() {
            for c in original.to_lowercase() {
                if Self::is_cjk(c) {
                    Self::flush_word(&mut word, &mut tokens);
                    cjk_run.push(c);
                } else if c.is_alphanumeric() {
                    Self::flush_cjk_run(&mut cjk_run, &mut tokens);
                    word.push(c);
                } else {
                    // 标点和空白都是词边界，不能删除后拼接两侧内容。
                    Self::flush_word(&mut word, &mut tokens);
                    Self::flush_cjk_run(&mut cjk_run, &mut tokens);
                }
            }
        }
        Self::flush_word(&mut word, &mut tokens);
        Self::flush_cjk_run(&mut cjk_run, &mut tokens);

        tokens
    }

    /// 收集当前连续字母数字词，并清空缓冲准备下一个词。
    fn flush_word(word: &mut String, tokens: &mut HashSet<String>) {
        if !word.is_empty() {
            tokens.insert(std::mem::take(word));
        }
    }

    /// 将连续中日韩字符生成 1～4 字组合，不跨标点拼接关键词。
    fn flush_cjk_run(run: &mut Vec<char>, tokens: &mut HashSet<String>) {
        let max_size = run.len().min(MAX_CJK_NGRAM_SIZE);
        for size in 1..=max_size {
            for window in run.windows(size) {
                tokens.insert(window.iter().collect());
            }
        }
        run.clear();
    }

    /// 判断字符是否属于本索引支持的中日韩文字区段。
    fn is_cjk(c: char) -> bool {
        matches!(
            c,
            '\u{1100}'..='\u{11ff}'
                | '\u{3040}'..='\u{30ff}'
                | '\u{3100}'..='\u{318f}'
                | '\u{31a0}'..='\u{31bf}'
                | '\u{31f0}'..='\u{31ff}'
                | '\u{3400}'..='\u{4dbf}'
                | '\u{4e00}'..='\u{9fff}'
                | '\u{a960}'..='\u{a97f}'
                | '\u{ac00}'..='\u{d7af}'
                | '\u{d7b0}'..='\u{d7ff}'
                | '\u{f900}'..='\u{faff}'
                | '\u{ff66}'..='\u{ff9d}'
                | '\u{20000}'..='\u{2ebef}'
                | '\u{30000}'..='\u{323af}'
        )
    }

    /// 布隆过滤器快速过滤 + 可选精确匹配
    fn smart_search(
        &self,
        normalized_query: &str,
        query_terms: &HashSet<String>,
        bloom_trust_threshold: usize,
        direct_contains_threshold: usize,
    ) -> bool {
        let content_size = self.content.as_bytes().len();
        // 如果内容大小小于配置的direct_contains_threshold，直接使用contains搜索
        if content_size < direct_contains_threshold {
            log::debug!(
                "内容大小 {} 字节小于直接搜索阈值 {} 字节，使用直接contains搜索",
                content_size,
                direct_contains_threshold
            );
            return self.content_contains(normalized_query);
        }

        // 纯标点或 emoji 等没有分词结果时，回退到精确包含搜索。
        if query_terms.is_empty() {
            return self.content_contains(normalized_query);
        }

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
        // 未到信任阈值时，以原文包含结果为准，不能仅凭分词命中认定匹配。
        self.content_contains(normalized_query)
    }

    /// 内容包含搜索
    fn content_contains(&self, normalized_query: &str) -> bool {
        let normalized_content = self.content.to_lowercase();

        // 直接字符串包含搜索
        normalized_content.contains(normalized_query)
    }
}

/// 并发可读写的记录索引，键为 IPC 使用的字符串记录 ID。
struct SimpleSearchIndex {
    records: DashMap<String, RecordSearchData>,
}

impl SimpleSearchIndex {
    /// 创建空索引，记录会由启动重建或后续采集填充。
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
        let normalized_query = query.trim().to_lowercase();
        if normalized_query.is_empty() {
            return Vec::new();
        }
        // 查询内容对所有记录相同，只在进入记录遍历前分词一次。
        let query_terms = RecordSearchData::extract_search_terms(&normalized_query);

        let mut results = Vec::new();
        for entry in self.records.iter() {
            let (id, search_data) = (entry.key(), entry.value());
            // 布隆过滤器优先 + 内容包含搜索
            if search_data.smart_search(
                &normalized_query,
                &query_terms,
                bloom_trust_threshold,
                direct_contains_threshold,
            ) {
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
    decoder: Arc<dyn TextDecoder>,
}

impl InMemorySearchEngine {
    /// 共享当前设置和解密器；索引内容只保留在本进程内存。
    pub fn new(settings: Arc<RwLock<Settings>>, decoder: Arc<dyn TextDecoder>) -> Self {
        Self {
            index: SimpleSearchIndex::new(),
            settings,
            decoder,
        }
    }

    /// 读取布隆信任阈值和直接搜索阈值；读锁失败时使用默认配置。
    fn thresholds(&self) -> (usize, usize) {
        self.settings.read()
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
    /// 根据当前阈值搜索候选 ID，最终列表排序和记录有效性由仓储处理。
    async fn search(&self, query: &str) -> Vec<String> {
        let (bloom_threshold, direct_threshold) = self.thresholds();
        self.index.search(query, bloom_threshold, direct_threshold)
    }

    /// 添加或替换一条索引，重复复制使用同一记录 ID，不增加索引条数。
    async fn add(&self, id: &str, content: &str) -> AppResult<()> {
        self.index.add_record(id, content);
        log::debug!(
            "添加记录到搜索索引 - ID: {}, 内容长度: {}",
            id,
            content.len()
        );
        Ok(())
    }

    /// 批量删除已移除记录的索引；空列表无需处理。
    async fn remove(&self, ids: &[String]) -> AppResult<()> {
        if ids.is_empty() {
            return Ok(());
        }
        self.index.remove_records(ids);
        log::debug!("从搜索索引中删除 {} 个记录", ids.len());
        Ok(())
    }

    /// 从数据库完整快照重建索引，单条文本解密失败只记录日志并跳过。
    async fn initialize(&self, records: Vec<ClipRecord>) -> AppResult<()> {
        self.index.clear();
        let total_count = records.len();
        let mut indexed_count = 0;

        for record in records {
            // 数据库主键为 i64；索引与 IPC 统一使用十进制字符串。
            let id = record.id.to_string();
            let should_index = match record.r#type.as_str() {
                TEXT_TYPE => record
                    .content
                    .as_deref()
                    .and_then(
                        |content| match self.decoder.decode(content) {
                            Ok(content) => {
                                self.index.add_record(&id, &content);
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
                FILE_TYPE | crate::domain::clip::DIRECTORY_TYPE => {
                    // 只索引用户显示名，不把随机缓存路径作为搜索内容。
                    self.index.add_record(&id, &record.search_names()); true
                }
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

    /// 测试解密替身：直接返回输入，让索引测试不依赖真实密钥。
    fn test_decode(text: &str) -> AppResult<String> { Ok(text.to_owned()) }

    /// 验证重建通过注入能力解密文本，并能搜索文件原始显示名。
    #[tokio::test]
    async fn rebuild_uses_injected_decoder_and_keeps_file_names_searchable() {
        let decoder = |text: &str| {
            if text == "encrypted" { Ok("解密后的内容".to_owned()) }
            else { Err(crate::errors::AppError::Crypto("invalid test ciphertext".into())) }
        };
        let engine = InMemorySearchEngine::new(Arc::new(RwLock::new(Settings::default())), Arc::new(decoder));
        let make_record = |id: i64, record_type: &str, content: &str| ClipRecord {
            id, r#type: record_type.into(), content: Some(content.into()), ..Default::default()
        };
        let mut file=make_record(3,FILE_TYPE,"");
        file.resources.push(crate::domain::clip::ClipRecordResource{display_name:"今日总结.md".into(),..Default::default()});
        engine.initialize(vec![make_record(1,TEXT_TYPE,"encrypted"),make_record(2,TEXT_TYPE,"broken"),file]).await.unwrap();
        assert_eq!(engine.search("解密").await, vec!["1"]);
        assert_eq!(engine.search("总结").await, vec!["3"]);
        assert!(engine.search("encrypted").await.is_empty());
        assert!(engine.search("broken").await.is_empty());
    }

    /// 验证添加、搜索、删除索引形成完整生命周期。
    #[tokio::test]
    async fn adds_searches_and_removes_records() {
        let engine = InMemorySearchEngine::new(Arc::new(RwLock::new(Settings::default())), Arc::new(test_decode));
        engine.add("record-1", "hello clipboard").await.unwrap();

        assert_eq!(engine.search("clipboard").await, vec!["record-1"]);

        engine.remove(&["record-1".to_string()]).await.unwrap();
        assert!(engine.search("clipboard").await.is_empty());
    }

    /// 验证纯空白查询不返回全部记录。
    #[tokio::test]
    async fn blank_query_returns_no_records() {
        let engine = InMemorySearchEngine::new(Arc::new(RwLock::new(Settings::default())), Arc::new(test_decode));
        engine.add("record-1", "hello clipboard").await.unwrap();

        assert!(engine.search(" \t\r\n ").await.is_empty());
    }

    /// 验证标点和表情无法分词时仍可用原文包含搜索。
    #[tokio::test]
    async fn query_without_tokens_uses_exact_contains_search() {
        let settings = Settings {
            bloom_filter_trust_threshold: Some(0),
            direct_contains_threshold: Some(0),
            ..Settings::default()
        };
        let engine = InMemorySearchEngine::new(Arc::new(RwLock::new(settings)), Arc::new(test_decode));
        engine.add("matching", "value ! 😀").await.unwrap();
        engine.add("other", "value without marker").await.unwrap();

        assert_eq!(engine.search("!").await, vec!["matching"]);
        assert_eq!(engine.search("😀").await, vec!["matching"]);
    }

    /// 验证标点分隔两侧单词，不把它们错误拼成一个词。
    #[test]
    fn tokenizer_uses_punctuation_as_a_boundary() {
        let tokens = RecordSearchData::extract_search_terms("Foo-bar hello.world");

        assert!(tokens.contains("foo"));
        assert!(tokens.contains("bar"));
        assert!(tokens.contains("hello"));
        assert!(tokens.contains("world"));
        assert!(!tokens.contains("foobar"));
        assert!(!tokens.contains("helloworld"));
    }

    /// 验证中文组合不跨标点或空白生成。
    #[test]
    fn tokenizer_keeps_cjk_ngrams_inside_contiguous_runs() {
        let tokens = RecordSearchData::extract_search_terms("甲-乙 中文");

        assert!(tokens.contains("甲"));
        assert!(tokens.contains("乙"));
        assert!(tokens.contains("中"));
        assert!(tokens.contains("中文"));
        assert!(!tokens.contains("甲乙"));
        assert!(!tokens.contains("乙中"));
    }

    /// 验证大小写转换、带重音字母以及日文韩文分词。
    #[test]
    fn tokenizer_supports_unicode_words_and_cjk_scripts() {
        let tokens = RecordSearchData::extract_search_terms("CAFÉ42 かな 한글");

        assert!(tokens.contains("café42"));
        assert!(tokens.contains("かな"));
        assert!(tokens.contains("한글"));
    }

    /// 验证中文短语和单字在布隆搜索路径中可被检索。
    #[tokio::test]
    async fn bloom_search_matches_embedded_cjk_phrase_and_single_character() {
        let settings = Settings {
            bloom_filter_trust_threshold: Some(0),
            direct_contains_threshold: Some(0),
            ..Settings::default()
        };
        let engine = InMemorySearchEngine::new(Arc::new(RwLock::new(settings)), Arc::new(test_decode));
        engine
            .add("record-1", "前缀中华人民共和国后缀")
            .await
            .unwrap();

        assert_eq!(engine.search("中华人民共和国").await, vec!["record-1"]);
        assert_eq!(engine.search("国").await, vec!["record-1"]);
    }
}

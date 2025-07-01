use crate::{
    biz::clip_record::ClipRecord,
    utils::{file_dir::get_data_dir, aes_util::decrypt_content},
};
use anyhow::{Context, Result};
use bincode::{Decode, Encode, config};
use dashmap::DashMap;
use once_cell::sync::Lazy;
use std::{
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
    time::Duration,
    collections::HashMap,
};
use tokio::{sync::Mutex, time::sleep};
use crate::CONTEXT;
use crate::biz::system_setting::Settings;

// 配置常量
const BINCODE_CONFIG: config::Configuration = config::standard();
const INDEX_FILE_NAME: &str = "clip_pal.bin";
const DEBOUNCE_DURATION: Duration = Duration::from_secs(2);
const DEFAULT_MAX_CONTENT_SIZE: usize = 2 * 1024 * 1024; // 2MB

// 简化版本管理
static CURRENT_VERSION: AtomicU64 = AtomicU64::new(0);
static PERSIST_SCHEDULED: AtomicBool = AtomicBool::new(false);

/// 可序列化的索引结构 
#[derive(Encode, Decode, Debug, Clone)]
struct SearchIndex {
    id_to_content_lower: HashMap<String, String>, 
    version: u64,
}

/// 并发搜索索引
struct ConcurrentSearchIndex {
    id_to_content_lower: DashMap<String, String>,
}

impl ConcurrentSearchIndex {
    fn new() -> Self {
        Self {
            id_to_content_lower: DashMap::new(),
        }
    }

    fn add_content(&self, id: &str, content: &str) {
        // 直接存储小写
        let content_lower = content.to_lowercase();
        self.id_to_content_lower.insert(id.to_string(), content_lower);
    }

    fn remove_ids(&self, ids: &[String]) {
        for id in ids {
            self.id_to_content_lower.remove(id);
        }
    }

    fn search_by_content(&self, query: &str) -> Vec<String> {
        if query.is_empty() {
            return Vec::new();
        }

        let query_lower = query.to_lowercase();
        let mut results = Vec::with_capacity(32);

        for entry in self.id_to_content_lower.iter() {
            if entry.value().contains(&query_lower) {
                results.push(entry.key().clone());
            }
        }

        results
    }

    fn to_serializable(&self) -> SearchIndex {
        // 预分配HashMap容量
        let mut id_to_content_lower = HashMap::with_capacity(self.id_to_content_lower.len());
        
        for entry in self.id_to_content_lower.iter() {
            id_to_content_lower.insert(entry.key().clone(), entry.value().clone());
        }

        SearchIndex {
            id_to_content_lower,
            version: CURRENT_VERSION.load(Ordering::Acquire),
        }
    }

    fn len(&self) -> usize {
        self.id_to_content_lower.len()
    }

    fn clear(&self) {
        self.id_to_content_lower.clear();
    }
}

// 全局索引
static SEARCH_INDEX: Lazy<Arc<ConcurrentSearchIndex>> =
    Lazy::new(|| Arc::new(ConcurrentSearchIndex::new()));

static PERSIST_MUTEX: Lazy<Mutex<()>> = Lazy::new(|| Mutex::new(()));
static PERSIST_TASK_MUTEX: Lazy<Mutex<Option<tokio::task::JoinHandle<()>>>> =
    Lazy::new(|| Mutex::new(None));

fn index_path() -> Result<PathBuf> {
    let mut path = get_data_dir().context("Get data dir failed")?;
    path.push(INDEX_FILE_NAME);
    Ok(path)
}

/// 加载索引从磁盘
pub async fn load_index_from_disk() -> Result<()> {
    let path = index_path()?;
    if !path.exists() {
        log::debug!("Search index file not found, will create on first update");
        return Ok(());
    }

    let file_data = tokio::fs::read(&path).await
        .context("Failed to read search index file")?;

    if file_data.is_empty() {
        log::warn!("Search index file is empty");
        return Ok(());
    }

    let (index, _) = bincode::decode_from_slice(&file_data, BINCODE_CONFIG)
        .context("Failed to decode search index")?;
    let index: SearchIndex = index;
    
    log::debug!("Loaded search index version {} from disk", index.version);

    // 批量插入到内存索引
    for (id, content_lower) in index.id_to_content_lower {
        SEARCH_INDEX.id_to_content_lower.insert(id, content_lower);
    }

    CURRENT_VERSION.store(index.version, Ordering::Release);
    log::info!("搜索索引加载完成，共 {} 条记录", SEARCH_INDEX.len());
    
    Ok(())
}

/// 持久化索引到磁盘
async fn persist_index() -> Result<()> {
    let _guard = PERSIST_MUTEX.lock().await;

    let path = index_path()?;
    let serializable_index = SEARCH_INDEX.to_serializable();
    
    let bin = bincode::encode_to_vec(&serializable_index, BINCODE_CONFIG)
        .context("Failed to encode search index")?;
    
    let tmp = path.with_extension("tmp");

    // 使用tokio异步文件操作
    tokio::fs::write(&tmp, &bin).await
        .context("Write tmp file failed")?;

    tokio::fs::rename(&tmp, &path).await
        .context("Rename failed")?;

    log::debug!("Persisted search index version {}", serializable_index.version);
    Ok(())
}

/// 调度后台持久化任务
async fn schedule_persist_task() {
    if PERSIST_SCHEDULED.swap(true, Ordering::SeqCst) {
        return;
    }

    let mut task_guard = PERSIST_TASK_MUTEX.lock().await;

    if let Some(handle) = task_guard.take() {
        handle.abort();
    }

    let handle = tokio::spawn(async move {
        sleep(DEBOUNCE_DURATION).await;
        PERSIST_SCHEDULED.store(false, Ordering::SeqCst);

        if let Err(e) = persist_index().await {
            log::error!("Persist failed: {}", e);
        }

        let mut task_guard = PERSIST_TASK_MUTEX.lock().await;
        *task_guard = None;
    });

    *task_guard = Some(handle);
}

/// 优化的设置获取函数
fn get_max_content_size() -> usize {
    CONTEXT.try_get::<Arc<std::sync::Mutex<Settings>>>()
        .and_then(|lock| lock.lock().ok())
        .and_then(|settings| settings.search_index_max_content_size)
        .unwrap_or(DEFAULT_MAX_CONTENT_SIZE)
}

/// 添加内容到搜索索引 
pub async fn add_content_to_index(id: &str, content: &str) -> Result<()> {
    let max_size = get_max_content_size();
    if content.len() > max_size {
        log::debug!("内容过大，跳过搜索索引 - ID: {}, 大小: {:.2} MB", 
                   id, content.len() as f64 / 1024.0 / 1024.0);
        return Ok(());
    }
    
    SEARCH_INDEX.add_content(id, content); // 自动转换为小写存储
    CURRENT_VERSION.fetch_add(1, Ordering::SeqCst);
    schedule_persist_task().await;
    Ok(())
}

/// 根据内容搜索ID列表
pub async fn search_ids_by_content(content: &str) -> Vec<String> {
    SEARCH_INDEX.search_by_content(content)
}

/// 删除ID并更新索引
pub async fn remove_ids_from_index(ids: &[String]) -> Result<()> {
    if ids.is_empty() {
        return Ok(());
    }

    SEARCH_INDEX.remove_ids(ids);
    CURRENT_VERSION.fetch_add(1, Ordering::SeqCst);
    schedule_persist_task().await;
    Ok(())
}

/// 重建索引
pub async fn rebuild_index_after_crash<F, Fut>(fetch_all_clips: F) -> Result<()>
where
    F: Fn() -> Fut,
    Fut: std::future::Future<Output = Vec<ClipRecord>>,
{
    let current_index_size = SEARCH_INDEX.len();
    let all_clips: Vec<ClipRecord> = fetch_all_clips().await;
    
    // 检查是否需要重建索引
    let should_rebuild = match (current_index_size, all_clips.len()) {
        (0, n) if n > 0 => {
            log::info!("索引为空但数据库有 {} 条记录，需要重建索引", n);
            true
        }
        (index_size, db_size) if index_size > 0 && db_size > index_size * 2 => {
            log::warn!("数据库有 {} 条记录，但索引只有 {} 条，可能需要重建", db_size, index_size);
            true
        }
        _ => false,
    };
    
    if should_rebuild {
        log::info!("开始重建搜索索引...");
        
        // 清空现有索引
        SEARCH_INDEX.clear();
        
        let max_size = get_max_content_size();
        let mut indexed_count = 0;
        let mut skipped_count = 0;
        
        for record in all_clips {
            let should_index = match record.r#type.as_str() {
                "Text" => {
                    if let Some(encrypted_content) = record.content.as_str() {
                        match decrypt_content(encrypted_content) {
                            Ok(decrypted_content) => {
                                if decrypted_content.len() <= max_size {
                                    SEARCH_INDEX.add_content(&record.id, &decrypted_content);
                                    true
                                } else {
                                    log::debug!("重建索引时跳过大文件 - ID: {}, 大小: {:.2} MB", 
                                               record.id, decrypted_content.len() as f64 / 1024.0 / 1024.0);
                                    false
                                }
                            }
                            Err(e) => {
                                log::warn!("解密内容失败，跳过记录 - ID: {}, 错误: {}", record.id, e);
                                false
                            }
                        }
                    } else {
                        false
                    }
                }
                "File" => {
                    if let Some(file_paths) = record.content.as_str() {
                        if file_paths.len() <= max_size {
                            SEARCH_INDEX.add_content(&record.id, file_paths);
                            true
                        } else {
                            log::debug!("重建索引时跳过大文件路径 - ID: {}, 大小: {:.2} MB", 
                                       record.id, file_paths.len() as f64 / 1024.0 / 1024.0);
                            false
                        }
                    } else {
                        false
                    }
                }
                _ => false, // 图片类型不参与搜索
            };

            if should_index {
                indexed_count += 1;
            } else {
                skipped_count += 1;
            }
        }
        
        // 重建完成后，重置版本号并持久化
        CURRENT_VERSION.store(1, Ordering::Release);
        persist_index().await?;
        
        log::info!("索引重建完成 - 已索引: {} 条, 跳过: {} 条", indexed_count, skipped_count);
    } else if current_index_size > 0 {
        log::debug!("索引已存在 {} 条记录，数据库有 {} 条记录，无需重建", current_index_size, all_clips.len());
    } else {
        log::debug!("数据库为空，无需重建索引");
    }
    
    Ok(())
} 
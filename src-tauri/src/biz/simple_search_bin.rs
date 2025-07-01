use crate::{
    biz::clip_record::ClipRecord,
    utils::file_dir::get_data_dir,
};
use anyhow::{Context, Result};
use bincode::{Decode, Encode, config};
use dashmap::DashMap;
use once_cell::sync::Lazy;
use std::{
    fs::{File, OpenOptions},
    io::{Read, Write},
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
    time::Duration,
};
use tokio::{sync::Mutex, time::sleep};

// 配置
const BINCODE_CONFIG: config::Configuration = config::standard();
const INDEX_FILE_NAME: &str = "simple_search.bin";
const DEBOUNCE_DURATION: Duration = Duration::from_secs(2);

// 全局版本号
static CURRENT_VERSION: AtomicU64 = AtomicU64::new(0);
static LAST_PERSISTED_VERSION: AtomicU64 = AtomicU64::new(0);
static PERSIST_SCHEDULED: AtomicBool = AtomicBool::new(false);

/// 可序列化的索引结构
#[derive(Encode, Decode, Debug, Clone)]
struct SearchIndex {
    id_to_content: std::collections::HashMap<String, String>,
    version: u64,
}

/// 并发搜索索引
struct ConcurrentSearchIndex {
    id_to_content: DashMap<String, String>,
    version: AtomicU64,
}

impl ConcurrentSearchIndex {
    fn new() -> Self {
        Self {
            id_to_content: DashMap::new(),
            version: AtomicU64::new(0),
        }
    }

    fn add_content(&self, id: &str, content: &str) {
        self.id_to_content.insert(id.to_string(), content.to_string());
    }

    fn remove_ids(&self, ids: &Vec<String>) {
        for id in ids {
            self.id_to_content.remove(id);
        }
    }

    /// 简单字符串包含搜索，支持大小写不敏感
    fn search_by_content(&self, query: &str) -> Vec<String> {
        if query.is_empty() {
            return Vec::new();
        }

        let query_lower = query.to_lowercase();
        let mut results: Vec<(String, usize)> = Vec::new();

        for entry in self.id_to_content.iter() {
            let content_lower = entry.value().to_lowercase();
            if content_lower.contains(&query_lower) {
                // 计算匹配位置，越靠前优先级越高
                let position = content_lower.find(&query_lower).unwrap_or(usize::MAX);
                results.push((entry.key().clone(), position));
            }
        }

        // 按匹配位置排序，然后按ID排序
        results.sort_by(|a, b| {
            a.1.cmp(&b.1).then_with(|| a.0.cmp(&b.0))
        });

        results.into_iter().map(|(id, _)| id).collect()
    }

    fn to_serializable(&self) -> SearchIndex {
        let id_to_content = self
            .id_to_content
            .iter()
            .map(|entry| (entry.key().clone(), entry.value().clone()))
            .collect();

        SearchIndex {
            id_to_content,
            version: self.version.load(Ordering::Acquire),
        }
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
        log::debug!("Simple search index file not found, will create on first update");
        return Ok(());
    }

    let mut file = File::open(&path).context("Failed to open search index file")?;
    let mut buf = Vec::new();
    file.read_to_end(&mut buf).context("Failed to read index")?;

    if buf.is_empty() {
        log::warn!("Search index file is empty");
        return Ok(());
    }

    let index: SearchIndex = bincode::decode_from_slice(&buf, BINCODE_CONFIG)
        .context("Failed to decode search index")?
        .0;

    log::debug!("Loaded search index version {} from disk", index.version);

    // 加载到内存索引
    for (id, content) in &index.id_to_content {
        SEARCH_INDEX.id_to_content.insert(id.clone(), content.clone());
    }

    // 设置版本号
    CURRENT_VERSION.store(index.version, Ordering::Release);
    LAST_PERSISTED_VERSION.store(index.version, Ordering::Release);
    SEARCH_INDEX.version.store(index.version, Ordering::Release);

    log::info!("简单搜索索引加载完成，共 {} 条记录", index.id_to_content.len());

    Ok(())
}

/// 持久化索引到磁盘
async fn persist_index() -> Result<()> {
    let _guard = PERSIST_MUTEX.lock().await;

    let current_version = CURRENT_VERSION.load(Ordering::Acquire);
    let index_version = SEARCH_INDEX.version.load(Ordering::Acquire);
    
    if index_version < current_version {
        log::warn!(
            "Skipping persist for outdated index version: {} < {}",
            index_version,
            current_version
        );
        return Ok(());
    }

    let path = index_path()?;
    let serializable_index = SEARCH_INDEX.to_serializable();
    let bin = bincode::encode_to_vec(&serializable_index, BINCODE_CONFIG)
        .context("Failed to encode search index")?;
    let tmp = path.with_extension("tmp");

    // 原子写入
    let mut file = OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(&tmp)
        .context("Create tmp file failed")?;

    file.write_all(&bin).context("Write tmp failed")?;
    file.sync_all().ok();

    std::fs::rename(&tmp, &path).context("Rename failed")?;

    LAST_PERSISTED_VERSION.store(index_version, Ordering::Release);
    log::debug!("Persisted search index version {}", index_version);
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

/// 添加内容到搜索索引
pub async fn add_content_to_index(id: &str, content: &str) -> Result<()> {
    SEARCH_INDEX.add_content(id, content);

    let new_version = CURRENT_VERSION.fetch_add(1, Ordering::SeqCst) + 1;
    SEARCH_INDEX.version.store(new_version, Ordering::Release);

    schedule_persist_task().await;
    Ok(())
}

/// 根据内容搜索ID列表
pub async fn search_ids_by_content(content: &str) -> Vec<String> {
    SEARCH_INDEX.search_by_content(content)
}

/// 删除ID并更新索引
pub async fn remove_ids_from_index(ids: &Vec<String>) -> Result<()> {
    if ids.is_empty() {
        return Ok(());
    }

    SEARCH_INDEX.remove_ids(ids);

    let new_version = CURRENT_VERSION.fetch_add(1, Ordering::SeqCst) + 1;
    SEARCH_INDEX.version.store(new_version, Ordering::Release);

    schedule_persist_task().await;
    Ok(())
}

/// 崩溃后重建索引
pub async fn rebuild_index_after_crash<F, Fut>(fetch_all_clips: F) -> Result<()>
where
    F: Fn() -> Fut,
    Fut: std::future::Future<Output = Vec<ClipRecord>>,
{
    let last_persisted = LAST_PERSISTED_VERSION.load(Ordering::Acquire);
    let current_version = CURRENT_VERSION.load(Ordering::Acquire);

    if last_persisted == current_version {
        log::debug!("Index is up-to-date, no rebuild needed");
        return Ok(());
    }

    log::warn!(
        "Rebuilding index due to version mismatch: persisted={}, current={}",
        last_persisted,
        current_version
    );

    let all_clips: Vec<ClipRecord> = fetch_all_clips().await;

    for record in all_clips {
        if let Some(content_str) = record.content.as_str() {
            SEARCH_INDEX.add_content(&record.id, content_str);
        }
    }

    SEARCH_INDEX.version.store(current_version, Ordering::Release);
    persist_index().await?;

    Ok(())
} 
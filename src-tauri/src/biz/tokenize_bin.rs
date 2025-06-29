use crate::{
    biz::clip_record::ClipRecord,
    utils::{file_dir::get_data_dir, tokenize_util::tokenize_str},
};
use anyhow::{Context, Result};
use bincode::{Decode, Encode, config};
use dashmap::{DashMap, DashSet};
use once_cell::sync::Lazy;
use std::{
    collections::{HashMap, HashSet},
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
const INDEX_FILE_NAME: &str = "clip_tokens.bin";
const DEBOUNCE_DURATION: Duration = Duration::from_secs(2);

// 全局版本号（原子操作，确保线程安全）
static CURRENT_VERSION: AtomicU64 = AtomicU64::new(0);
static LAST_PERSISTED_VERSION: AtomicU64 = AtomicU64::new(0);

// 计划持久化标志位
static PERSIST_SCHEDULED: AtomicBool = AtomicBool::new(false);

/// 索引文件结构   这个用于持久化到磁盘
#[derive(Encode, Decode, Debug, Clone)]
struct TokenIndex {
    token_to_ids: HashMap<String, HashSet<String>>,
    id_to_tokens: HashMap<String, HashSet<String>>,
    version: u64,
}

/// 使用 DashMap 实现高性能并发索引  
/// 用于内存操作   DashMap和DashSet无法直接使用bincode序列化
struct ConcurrentTokenIndex {
    token_to_ids: DashMap<String, DashSet<String>>,
    id_to_tokens: DashMap<String, DashSet<String>>,
    version: AtomicU64,
}

impl ConcurrentTokenIndex {
    fn new() -> Self {
        Self {
            token_to_ids: DashMap::new(),
            id_to_tokens: DashMap::new(),
            version: AtomicU64::new(0),
        }
    }

    fn add_content(&self, id: &str, tokens: &DashSet<String>) {
        // 如果 ID 已存在，先移除旧索引
        if let Some(old_tokens) = self.id_to_tokens.get(id) {
            for token in old_tokens.iter() {
                if let Some(set) = self.token_to_ids.get_mut(token.as_str()) {
                    set.remove(id);
                }
            }
        }

        // 添加新索引
        for token in tokens.iter() {
            self.token_to_ids
                .entry(token.to_string())
                .or_insert_with(DashSet::new)
                .insert(id.to_string());
        }

        self.id_to_tokens.insert(id.to_string(), tokens.clone());
    }

    fn remove_ids(&self, ids: &Vec<String>) {
        for id in ids {
            if let Some((_, tokens)) = self.id_to_tokens.remove(id) {
                for token in tokens.iter() {
                    if let Some(set) = self.token_to_ids.get_mut(token.as_str()) {
                        set.remove(id);
                    }
                }
            }
        }
    }

    #[allow(dead_code)]
    fn get_ids_for_token(&self, token: &str) -> HashSet<String> {
        self.token_to_ids
            .get(token)
            .map(|set| set.iter().map(|s| s.clone()).collect())
            .unwrap_or_default()
    }

    #[allow(dead_code)]
    fn get_tokens_for_id(&self, id: &str) -> HashSet<String> {
        self.id_to_tokens
            .get(id)
            .map(|set| set.iter().map(|s| s.clone()).collect())
            .unwrap_or_default()
    }

    fn to_serializable(&self) -> TokenIndex {
        let token_to_ids = self
            .token_to_ids
            .iter()
            .map(|entry| {
                (
                    entry.key().clone(),
                    entry.value().iter().map(|x| x.to_string()).collect(),
                )
            })
            .collect();

        let id_to_tokens = self
            .id_to_tokens
            .iter()
            .map(|entry| {
                (
                    entry.key().clone(),
                    entry.value().iter().map(|x| x.to_string()).collect(),
                )
            })
            .collect();

        TokenIndex {
            token_to_ids,
            id_to_tokens,
            version: self.version.load(Ordering::Acquire),
        }
    }

    /// 根据多个 token 获取匹配的 ID 列表，按匹配的 token 数量排序
    fn get_ids_by_tokens(&self, tokens: &Vec<String>) -> Vec<String> {
        if tokens.is_empty() {
            return Vec::new();
        }

        let mut id_counts: HashMap<String, u32> = HashMap::new();

        for token in tokens {
            if let Some(id_set) = self.token_to_ids.get(token) {
                for id in id_set.iter() {
                    *id_counts.entry(id.clone()).or_insert(0) += 1;
                }
            }
        }

        // 转换为 Vec 并排序
        let mut result: Vec<(String, u32)> = id_counts.into_iter().collect();
        result.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));

        result.into_iter().map(|(id, _)| id).collect()
    }
}

// 全局索引状态
static TOKEN_INDEX: Lazy<Arc<ConcurrentTokenIndex>> =
    Lazy::new(|| Arc::new(ConcurrentTokenIndex::new()));

// 使用 tokio::sync::Mutex 替代 std::sync::Mutex
static PERSIST_MUTEX: Lazy<Mutex<()>> = Lazy::new(|| Mutex::new(()));
static PERSIST_TASK_MUTEX: Lazy<Mutex<Option<tokio::task::JoinHandle<()>>>> =
    Lazy::new(|| Mutex::new(None));

fn token_index_path() -> Result<PathBuf> {
    let mut path = get_data_dir().context("Get data dir failed")?;
    path.push(INDEX_FILE_NAME);
    Ok(path)
}

/// 加载磁盘索引并初始化版本号
pub async fn load_index_from_disk() -> Result<()> {
    let path = token_index_path()?;
    if !path.exists() {
        log::debug!("Token index file not found, will create on first update");
        return Ok(());
    }

    let mut file = File::open(&path).context("Failed to open token index file")?;
    let mut buf = Vec::new();
    file.read_to_end(&mut buf).context("Failed to read index")?;

    if buf.is_empty() {
        log::warn!("Token index file is empty");
        return Ok(());
    }

    let index: TokenIndex = bincode::decode_from_slice(&buf, BINCODE_CONFIG)
        .context("Failed to decode token index")?
        .0; // decode_from_slice 返回 (T, usize)
    log::debug!("Loaded token index version {} from disk", index.version);

    // 将磁盘索引加载到并发索引中
    for (id, tokens) in &index.id_to_tokens {
        let dash_tokens: DashSet<String> = tokens.iter().cloned().collect();
        TOKEN_INDEX
            .id_to_tokens
            .insert(id.clone(), dash_tokens.clone());

        for token in tokens {
            TOKEN_INDEX
                .token_to_ids
                .entry(token.clone())
                .or_insert_with(DashSet::new)
                .insert(id.clone());
        }
    }

    // 设置版本号
    CURRENT_VERSION.store(index.version, Ordering::Release);
    LAST_PERSISTED_VERSION.store(index.version, Ordering::Release);
    TOKEN_INDEX.version.store(index.version, Ordering::Release);

    Ok(())
}

/// 持久化索引到磁盘（带版本检查）
async fn persist_index() -> Result<()> {
    let _guard = PERSIST_MUTEX.lock().await; // 使用异步锁

    // 获取当前最新版本号
    let current_version = CURRENT_VERSION.load(Ordering::Acquire);

    // 检查索引版本是否过时
    let index_version = TOKEN_INDEX.version.load(Ordering::Acquire);
    if index_version < current_version {
        log::warn!(
            "Skipping persist for outdated index version: {} < {}",
            index_version,
            current_version
        );
        return Ok(());
    }

    let path = token_index_path()?;
    let serializable_index = TOKEN_INDEX.to_serializable();
    let bin = bincode::encode_to_vec(&serializable_index, BINCODE_CONFIG)
        .context("Failed to encode token index")?;
    let tmp = path.with_extension("tmp");

    // 原子写入
    let mut file = OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(&tmp)
        .context("Create tmp file failed")?;

    file.write_all(&bin).context("Write tmp failed")?;
    file.sync_all().ok(); // 确保数据落盘

    std::fs::rename(&tmp, &path).context("Rename failed")?;

    // 更新最后持久化的版本号
    LAST_PERSISTED_VERSION.store(index_version, Ordering::Release);

    log::debug!("Persisted token index version {}", index_version);
    Ok(())
}

/// 调度后台持久化任务（带防抖）
async fn schedule_persist_task() {
    // 检查是否已有任务计划
    if PERSIST_SCHEDULED.swap(true, Ordering::SeqCst) {
        return;
    }

    // 获取任务锁
    let mut task_guard = PERSIST_TASK_MUTEX.lock().await;

    // 取消现有任务（如果有）
    if let Some(handle) = task_guard.take() {
        handle.abort();
    }

    // 创建新任务
    let handle = tokio::spawn(async move {
        sleep(DEBOUNCE_DURATION).await;

        // 重置计划标志
        PERSIST_SCHEDULED.store(false, Ordering::SeqCst);

        // 执行持久化
        if let Err(e) = persist_index().await {
            log::error!("Persist failed: {}", e);
        }

        // 清除任务句柄
        let mut task_guard = PERSIST_TASK_MUTEX.lock().await;
        *task_guard = None;
    });

    // 存储任务句柄
    *task_guard = Some(handle);
}

/// 处理内容并更新索引
pub async fn content_tokenize_save_bin(id: &str, content: &str) -> Result<()> {
    let tokens: DashSet<String> = tokenize_str(content).await.into_iter().collect();

    // 更新内存索引
    TOKEN_INDEX.add_content(id, &tokens);

    // 更新版本号
    let new_version = CURRENT_VERSION.fetch_add(1, Ordering::SeqCst) + 1;
    TOKEN_INDEX.version.store(new_version, Ordering::Release);

    // 调度后台持久化任务
    schedule_persist_task().await;

    Ok(())
}

/// 根据多个 token 获取匹配的 ID 列表，按匹配的 token 数量排序
pub async fn get_ids_by_content(content: &str) -> Vec<String> {
    let res = tokenize_str(content).await;
    let res_vec: Vec<String> = res.into_iter().collect();
    TOKEN_INDEX.get_ids_by_tokens(&res_vec)
}

/// 删除ID并更新索引
pub async fn remove_ids_from_token_bin(ids: &Vec<String>) -> Result<()> {
    if ids.is_empty() {
        return Ok(());
    }

    // 更新内存索引
    TOKEN_INDEX.remove_ids(ids);

    // 更新版本号
    let new_version = CURRENT_VERSION.fetch_add(1, Ordering::SeqCst) + 1;
    TOKEN_INDEX.version.store(new_version, Ordering::Release);

    // 调度后台持久化任务
    schedule_persist_task().await;

    Ok(())
}

/// 崩溃后重建索引（应在程序启动时调用）
pub async fn rebuild_index_after_crash<F, Fut>(fetch_all_clips: F) -> Result<()>
where
    F: Fn() -> Fut,
    Fut: std::future::Future<Output = Vec<ClipRecord>>,
{
    // 获取最后持久化的版本号
    let last_persisted = LAST_PERSISTED_VERSION.load(Ordering::Acquire);
    let current_version = CURRENT_VERSION.load(Ordering::Acquire);

    // 如果版本一致，说明没有丢失更新
    if last_persisted == current_version {
        log::debug!("Index is up-to-date, no rebuild needed");
        return Ok(());
    }

    log::warn!(
        "Rebuilding index due to version mismatch: persisted={}, current={}",
        last_persisted,
        current_version
    );

    // 获取所有剪贴板内容
    let all_clips: Vec<ClipRecord> = fetch_all_clips().await;

    // 重建索引
    for record in all_clips {
        let tokens: DashSet<String> = tokenize_str(&record.content.to_string())
            .await
            .into_iter()
            .collect();
        TOKEN_INDEX.add_content(&record.id, &tokens);
    }

    // 更新版本号
    TOKEN_INDEX
        .version
        .store(current_version, Ordering::Release);

    // 立即持久化
    persist_index().await?;

    Ok(())
}

// 查询函数
#[allow(dead_code)]
pub async fn get_ids_for_token(token: &str) -> HashSet<String> {
    TOKEN_INDEX.get_ids_for_token(token)
}

#[allow(dead_code)]
pub async fn get_tokens_for_id(id: &str) -> HashSet<String> {
    TOKEN_INDEX.get_tokens_for_id(id)
}

// src/utils/token_index.rs

use crate::{
    biz::clip_record::ClipRecord,
    utils::{file_dir::get_data_dir, tokenize_util::tokenize_str},
};
use anyhow::{Context, Result};
use bincode::{Decode, Encode, config};
use log::{error, info, warn};
use once_cell::sync::Lazy;
use std::{
    collections::{HashMap, HashSet},
    fs::{File, OpenOptions},
    io::{Read, Write},
    path::PathBuf,
    sync::{
        Arc, Mutex,
        atomic::{AtomicU64, Ordering},
    },
    time::Duration,
};
use tokio::{sync::RwLock, time::sleep};

// 配置
const BINCODE_CONFIG: config::Configuration = config::standard();
const INDEX_FILE_NAME: &str = "clip_tokens.bin";

// 全局版本号（原子操作，确保线程安全）
static CURRENT_VERSION: AtomicU64 = AtomicU64::new(0);
static LAST_PERSISTED_VERSION: AtomicU64 = AtomicU64::new(0);

#[derive(Encode, Decode, Debug, PartialEq, Clone)]
struct TokenIndex {
    token_to_ids: HashMap<String, HashSet<String>>,
    id_to_tokens: HashMap<String, HashSet<String>>,
    version: u64, // 添加版本号字段
}

impl TokenIndex {
    fn new() -> Self {
        Self {
            token_to_ids: HashMap::new(),
            id_to_tokens: HashMap::new(),
            version: 0,
        }
    }

    fn from_bytes(data: &[u8]) -> Result<Self> {
        let (index, _) = bincode::decode_from_slice(data, BINCODE_CONFIG)
            .context("Failed to decode token index")?;
        Ok(index)
    }

    fn to_bytes(&self) -> Result<Vec<u8>> {
        let mut buffer = Vec::new();
        bincode::encode_into_std_write(self, &mut buffer, BINCODE_CONFIG)
            .context("Failed to encode token index")?;
        Ok(buffer)
    }

    fn add_content(&mut self, id: &str, tokens: &HashSet<String>) {
        // 移除旧索引（如果存在）
        if let Some(old_tokens) = self.id_to_tokens.get(id) {
            for token in old_tokens {
                if let Some(ids) = self.token_to_ids.get_mut(token) {
                    ids.remove(id);
                }
            }
        }

        // 添加新索引
        for token in tokens {
            self.token_to_ids
                .entry(token.clone())
                .or_default()
                .insert(id.to_string());
        }

        self.id_to_tokens.insert(id.to_string(), tokens.clone());
    }

    fn remove_ids(&mut self, ids: &[String]) {
        for id in ids {
            if let Some(tokens) = self.id_to_tokens.remove(id) {
                for token in tokens {
                    if let Some(set) = self.token_to_ids.get_mut(&token) {
                        set.remove(id);
                    }
                }
            }
        }
        self.token_to_ids.retain(|_, v| !v.is_empty());
    }

    fn get_ids_for_token(&self, token: &str) -> HashSet<String> {
        self.token_to_ids.get(token).cloned().unwrap_or_default()
    }

    fn get_tokens_for_id(&self, id: &str) -> HashSet<String> {
        self.id_to_tokens.get(id).cloned().unwrap_or_default()
    }
}

// 使用 Lazy 延迟初始化全局状态
static TOKEN_INDEX: Lazy<RwLock<Arc<TokenIndex>>> =
    Lazy::new(|| RwLock::new(Arc::new(TokenIndex::new())));

// 防止并发持久化任务的互斥锁
static PERSIST_TASK_MUTEX: Lazy<Mutex<()>> = Lazy::new(|| Mutex::new(()));

fn token_index_path() -> Result<PathBuf> {
    let mut path = get_data_dir().context("Get data dir failed")?;
    path.push(INDEX_FILE_NAME);
    Ok(path)
}

/// 加载磁盘索引并初始化版本号
pub async fn load_index_from_disk() -> Result<()> {
    let path = token_index_path()?;
    if !path.exists() {
        // 文件不存在时创建空索引
        let mut lock = TOKEN_INDEX.write().await;
        *lock = Arc::new(TokenIndex::new());
        info!("Created new empty token index");
        return Ok(());
    }

    let mut file = File::open(&path).context("Failed to open token index file")?;
    let mut buf = Vec::new();
    file.read_to_end(&mut buf).context("Failed to read index")?;

    if buf.is_empty() {
        warn!("Token index file is empty");
        return Ok(());
    }

    let index = TokenIndex::from_bytes(&buf)?;
    info!("Loaded token index version {} from disk", index.version);

    // 更新内存索引和版本号
    let mut lock = TOKEN_INDEX.write().await;
    *lock = Arc::new(index.clone());

    // 设置版本号
    CURRENT_VERSION.store(index.version, Ordering::Release);
    LAST_PERSISTED_VERSION.store(index.version, Ordering::Release);

    Ok(())
}

/// 持久化索引到磁盘（带版本检查）
async fn persist_index(index: Arc<TokenIndex>) -> Result<()> {
    let _guard = PERSIST_TASK_MUTEX.lock().unwrap(); // 使用阻塞锁

    // 获取当前最新版本号
    let current_version = CURRENT_VERSION.load(Ordering::Acquire);

    // 检查索引版本是否过时
    if index.version < current_version {
        warn!(
            "Skipping persist for outdated index version: {} < {}",
            index.version, current_version
        );
        return Ok(());
    }

    let path = token_index_path()?;
    let bin = index.to_bytes()?;
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
    LAST_PERSISTED_VERSION.store(index.version, Ordering::Release);

    info!("Persisted token index version {}", index.version);
    Ok(())
}

/// 创建新版本的索引
fn create_new_index_with_version(current: &TokenIndex) -> TokenIndex {
    let mut new_index = current.clone();
    let new_version = CURRENT_VERSION.fetch_add(1, Ordering::SeqCst) + 1;
    new_index.version = new_version;
    new_index
}

/// 处理内容并更新索引
pub async fn content_tokenize_save_bin(id: &str, content: &str) -> Result<()> {
    let tokens = tokenize_str(content)
        .await
        .into_iter()
        .collect::<HashSet<_>>();

    // 更新内存索引
    let mut lock = TOKEN_INDEX.write().await;
    let current = lock.as_ref(); // 直接获取引用
    let mut new_index = create_new_index_with_version(current);
    new_index.add_content(id, &tokens);

    let new_arc = Arc::new(new_index);
    *lock = new_arc.clone();

    // 启动后台持久化任务
    tokio::spawn(async move {
        sleep(Duration::from_secs(2)).await;

        // 检查版本是否仍然有效
        let current_version = CURRENT_VERSION.load(Ordering::Acquire);
        if new_arc.version < current_version {
            info!(
                "Skipping persist for outdated version: {} < {}",
                new_arc.version, current_version
            );
            return;
        }

        if let Err(e) = persist_index(new_arc).await {
            error!("Persist failed: {}", e);
        }
    });

    Ok(())
}

/// 删除ID并更新索引
pub async fn remove_ids_from_token_bin(ids: &[String]) -> Result<()> {
    if ids.is_empty() {
        return Ok(());
    }

    // 更新内存索引
    let mut lock = TOKEN_INDEX.write().await;
    let current = lock.as_ref(); // 直接获取引用
    let mut new_index = create_new_index_with_version(current);
    new_index.remove_ids(ids);

    let new_arc = Arc::new(new_index);
    *lock = new_arc.clone();

    // 启动后台持久化任务
    tokio::spawn(async move {
        sleep(Duration::from_secs(2)).await;

        // 检查版本是否仍然有效
        let current_version = CURRENT_VERSION.load(Ordering::Acquire);
        if new_arc.version < current_version {
            info!(
                "Skipping persist for outdated version: {} < {}",
                new_arc.version, current_version
            );
            return;
        }

        if let Err(e) = persist_index(new_arc).await {
            error!("Persist failed: {}", e);
        }
    });

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
        info!("Index is up-to-date, no rebuild needed");
        return Ok(());
    }

    warn!(
        "Rebuilding index due to version mismatch: persisted={}, current={}",
        last_persisted, current_version
    );

    // 获取所有剪贴板内容
    let all_clips: Vec<ClipRecord> = fetch_all_clips().await;

    // 创建新索引
    let mut new_index = TokenIndex::new();
    new_index.version = current_version;

    // 重建索引
    for record in all_clips {
        let tokens = tokenize_str(&record.content.to_string())
            .await
            .into_iter()
            .collect::<HashSet<_>>();
        new_index.add_content(&record.id, &tokens);
    }

    // 更新内存索引
    let mut lock = TOKEN_INDEX.write().await;
    *lock = Arc::new(new_index.clone());

    // 立即持久化
    persist_index(Arc::new(new_index)).await?;

    Ok(())
}

// 查询函数
pub async fn get_ids_for_token(token: &str) -> HashSet<String> {
    let lock = TOKEN_INDEX.read().await;
    lock.get_ids_for_token(token)
}

pub async fn get_tokens_for_id(id: &str) -> HashSet<String> {
    let lock = TOKEN_INDEX.read().await;
    lock.get_tokens_for_id(id)
}

pub async fn backup_token_index() -> Result<()> {
    let lock = TOKEN_INDEX.read().await;
    let bin = lock.to_bytes()?;
    let mut path = token_index_path()?;
    path.set_extension("bak");
    let mut file = OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(path)?;
    file.write_all(&bin)?;
    Ok(())
}

//! 单机版应用组装核心。自建版在此基础上扩展，不引入 Tauri 或商业服务。
use std::sync::atomic::AtomicBool;

use std::{
    path::Path,
    sync::{Arc, RwLock},
};
use rbatis::RBatis;
use clip_core::{
    domain::settings::Settings,
    errors::{AppError, AppResult},
    repositories::{AppRepositories, FileSettingsRepository, SettingsRepository},
    search::{InMemorySearchEngine, SearchEngine, TextDecoder},
    storage::SqliteClipRecordRepository,
};
pub use clip_core;

/// 应用核心资源。
///
/// 这里放应用启动时必须初始化完成的资源。
/// 如果这些资源缺失，应用通常无法正常运行。
pub struct LocalContext {
    /// SQLite / RBatis 连接实例。
    ///
    /// 这是后端业务数据访问的基础依赖。
    db: RBatis,

    /// 应用数据访问接口。
    repositories: AppRepositories,

    /// 剪贴记录搜索引擎。
    search_engine: Arc<dyn SearchEngine>,

    /// 系统设置缓存。
    ///
    /// 使用 `Arc<RwLock<Settings>>` 是为了让多个模块共享同一份设置：
    /// 读取配置时拿读锁，保存配置时拿写锁。
    settings: Arc<RwLock<Settings>>,
    decoder: Arc<dyn TextDecoder>,
    resources_dir: Option<std::path::PathBuf>,
    pub(crate) cleaning: AtomicBool,
    settings_save_lock: tokio::sync::Mutex<()>,
    pub(crate) capture_lock: tokio::sync::Mutex<()>,
}

impl LocalContext {
    /// 只接收调用方明确给出的路径和解密能力，不读取桌面应用的全局配置。
    pub async fn open(
        db_path: &Path,
        settings_path: Option<std::path::PathBuf>,
        decoder: Arc<dyn TextDecoder>,
    ) -> AppResult<Self> {
        Self::open_with_resources(db_path, settings_path, decoder, None).await
    }

    /// 打开本机数据库，并使用调用方提供的资源根目录。
    pub async fn open_with_resources(
        db_path: &Path,
        settings_path: Option<std::path::PathBuf>,
        decoder: Arc<dyn TextDecoder>,
        resources_dir: Option<std::path::PathBuf>,
    ) -> AppResult<Self> {
        let settings_repository = Arc::new(FileSettingsRepository::new(settings_path));
        let settings = Arc::new(RwLock::new(settings_repository.load()));
        let db = clip_core::storage::connect(db_path).await?;
        let repositories = AppRepositories::new(
            Arc::new(SqliteClipRecordRepository::new(db.clone())),
            settings_repository,
        );
        let search_engine = Arc::new(InMemorySearchEngine::new(settings.clone(), decoder.clone()));
        Ok(Self { db, repositories, search_engine, settings, decoder, resources_dir,
            settings_save_lock: tokio::sync::Mutex::new(()),
            cleaning: AtomicBool::new(false), capture_lock: tokio::sync::Mutex::new(()) })
    }

    /// 为已打开的应用上下文设置资源目录。
    pub fn with_resources_dir(mut self, path: Option<std::path::PathBuf>) -> Self {
        self.resources_dir = path;
        self
    }

    /// 串行保存设置，避免两个保存请求相互覆盖系统状态和配置文件。
    pub fn settings_save_lock(&self) -> &tokio::sync::Mutex<()> { &self.settings_save_lock }

    /// 返回当前组装时指定的资源目录，不猜测或读取用户的正式目录。
    pub fn resources_dir(&self) -> Option<std::path::PathBuf> { self.resources_dir.clone() }

    /// 默认备份目录与数据库目录并列。自定义目录只影响新采集的文件。
    pub fn file_backup_dir(&self) -> Option<std::path::PathBuf> {
        self.with_settings(|settings| {
            settings.file_backup_dir.as_deref().map(std::path::PathBuf::from)
        }).ok().flatten().or_else(|| self.resources_dir())
    }

    /// 旧记录继续使用原有副本，按相对路径在使用过的备份目录中查找。
    pub(crate) fn file_backup_roots(&self) -> Vec<std::path::PathBuf> {
        let mut roots: Vec<_> = self.file_backup_dir().into_iter().collect();
        roots.extend(self.resources_dir());
        if let Ok(previous) = self.with_settings(|settings| settings.previous_file_backup_dirs.clone()) {
            roots.extend(previous.into_iter().map(std::path::PathBuf::from));
        }
        roots
    }
    /// 取得注入的解密能力；密钥管理仍归桌面层。
    pub fn decoder(&self) -> &dyn TextDecoder { self.decoder.as_ref() }

    /// 共享当前 SQLite 连接池，供同库扩展事务使用。
    pub fn db(&self) -> &RBatis {
        &self.db
    }

    /// 同一套记录写入与索引更新共用此锁，外部应用扩展完成提交后再释放它。
    pub fn record_write_lock(&self) -> &tokio::sync::Mutex<()> {
        &self.capture_lock
    }

    /// 获取统一仓储入口，业务服务不需要自己创建数据库连接。
    pub fn repositories(&self) -> &AppRepositories {
        &self.repositories
    }

    /// 获取内存搜索索引；索引是查询加速层，数据库仍为记录来源。
    pub fn search_engine(&self) -> &dyn SearchEngine {
        self.search_engine.as_ref()
    }

    /// 共享设置缓存；需要短暂读取时优先使用 with_settings。
    pub fn settings(&self) -> Arc<RwLock<Settings>> {
        self.settings.clone()
    }

    /// 在短期读锁内取得设置值；闭包结束即释放锁，不跨异步等待持有。
    pub fn with_settings<R>(&self, f: impl FnOnce(&Settings) -> R) -> AppResult<R> {
        let settings = self.settings.read()
            .map_err(|e| AppError::Lock(format!("无法获取读锁: {}", e)))?;
        Ok(f(&settings))
    }

    /// 只更新共享内存；持久化仍由设置应用服务负责。
    pub fn update_settings<R>(&self, f: impl FnOnce(&mut Settings) -> R) -> AppResult<R> {
        let mut settings = self.settings.write()
            .map_err(|e| AppError::Lock(format!("无法获取写锁: {}", e)))?;
        Ok(f(&mut settings))
    }

    /// 搜索索引初始化失败不阻断应用启动，保持原有降级策略。
    pub async fn initialize_search_index(&self) {
        let records = self.repositories.clip_records().list_all().await.unwrap_or_else(|error| {
            log::error!("获取剪贴板记录失败: {}", error);
            vec![]
        });
        if let Err(error) = self.search_engine.initialize(records).await {
            log::error!("搜索索引初始化失败: {}", error);
        }
    }
}

pub mod dto;
pub mod records;
pub mod capture;
pub mod cleanup;
pub mod temporary_files;
pub mod update;
pub mod settings;

pub mod clipboard;

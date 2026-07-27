use std::sync::{Arc, RwLock};

use clipboard_listener::{ClipboardEvent, EventManager};
use log::LevelFilter;
use rbatis::RBatis;

use crate::{
    app_context::AppContext,
    biz::{clip_record::ClipRecord, clip_record_sync::ClipboardEventTigger},
    errors::AppResult,
    infra::{
        db,
        http::{HttpAuthClient, HttpVipClient},
        repositories::{
            AppRepositories, FileSettingsRepository, SettingsRepository, SqliteClipRecordRepository,
        },
        search::InMemorySearchEngine,
        security::SecureAuthStore,
        storage::SecureVipStore,
    },
    log_config::init_logging,
};

/// 启动后续阶段需要共享的核心资源。
///
/// 它不是业务上下文，而是启动编排层在 `setup`、`runtime` 阶段传递资源用的结构。
/// 真正给业务代码长期使用的统一入口仍然是 `AppContext`。
#[derive(Clone)]
pub struct BootstrapCore {
    /// 新架构下的应用上下文。
    pub app_context: Arc<AppContext>,
    /// SQLite / RBatis 连接。
    ///
    /// 启动阶段和后台任务共享同一个数据库连接实例。
    pub db: RBatis,
    /// 剪贴板事件管理器。
    ///
    /// setup 阶段会交给剪贴板插件开始监听，runtime 阶段会启动事件循环和退出清理。
    pub clipboard_event_manager: Arc<EventManager<ClipboardEvent>>,
}

/// 初始化应用核心资源。
///
/// 1. 初始化日志；
/// 2. 加载设置；
/// 3. 创建剪贴板事件管理器；
/// 4. 初始化 SQLite；
/// 5. 创建 `AppContext`；
/// 6. 初始化搜索索引。
///
pub async fn init_core() -> AppResult<BootstrapCore> {
    init_logging(LevelFilter::Info);

    let settings_repository = Arc::new(FileSettingsRepository);
    let settings = Arc::new(RwLock::new(settings_repository.load()));

    let clipboard_event_manager: Arc<EventManager<ClipboardEvent>> =
        Arc::new(EventManager::default());
    clipboard_event_manager.add_event_listener(Arc::new(ClipboardEventTigger));

    let db = db::connect().await?;
    // 所有 Repository implementation 在启动阶段只创建一次，service 运行时只依赖 trait。
    let repositories = AppRepositories::new(
        Arc::new(SqliteClipRecordRepository::new(db.clone())),
        settings_repository,
    );
    // 搜索引擎与仓储一并注入 AppContext，避免 service 访问全局实现对象。
    let search_engine = Arc::new(InMemorySearchEngine::new(settings.clone()));
    let app_context = Arc::new(AppContext::new(
        db.clone(),
        settings,
        repositories,
        Arc::new(HttpAuthClient),
        Arc::new(HttpVipClient),
        Arc::new(SecureAuthStore),
        Arc::new(SecureVipStore),
        search_engine,
    ));
    crate::app_context::set_app_context(app_context.clone())?;

    initialize_search_index_from_database(&db, app_context.as_ref()).await;

    Ok(BootstrapCore {
        app_context,
        db,
        clipboard_event_manager,
    })
}

/// 从数据库加载已有剪贴记录并初始化搜索索引。
///
/// 搜索索引初始化失败不阻断应用启动。
async fn initialize_search_index_from_database(db: &RBatis, context: &AppContext) {
    // 启动时加载全部记录建立内存索引；失败只影响搜索，不阻断应用启动。
    let all_clips = ClipRecord::select_order_by(db).await.unwrap_or_else(|e| {
        log::error!("获取剪贴板记录失败: {}", e);
        vec![]
    });

    if let Err(e) = context.search_engine().initialize(all_clips).await {
        log::error!("搜索索引初始化失败: {}", e);
    }
}

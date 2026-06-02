use std::sync::Arc;

use clipboard_listener::{ClipboardEvent, EventManager};
use log::LevelFilter;
use rbatis::RBatis;

use crate::{
    app_context::AppContext,
    biz::{
        clip_record::ClipRecord, clip_record_sync::ClipboardEventTigger,
        content_search::initialize_search_index, system_setting::load_settings_context,
    },
    errors::AppResult,
    log_config::init_logging,
    sqlite_storage,
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

    let settings = load_settings_context();

    let clipboard_event_manager: Arc<EventManager<ClipboardEvent>> =
        Arc::new(EventManager::default());
    clipboard_event_manager.add_event_listener(Arc::new(ClipboardEventTigger));

    let db = sqlite_storage::init_sqlite().await?;
    let app_context = Arc::new(AppContext::new(db.clone(), settings));

    initialize_search_index_from_database(&db).await;

    Ok(BootstrapCore {
        app_context,
        db,
        clipboard_event_manager,
    })
}

/// 从数据库加载已有剪贴记录并初始化搜索索引。
///
/// 搜索索引初始化失败不阻断应用启动。
async fn initialize_search_index_from_database(db: &RBatis) {
    let all_clips = ClipRecord::select_order_by(db).await.unwrap_or_else(|e| {
        log::error!("获取剪贴板记录失败: {}", e);
        vec![]
    });

    if let Err(e) = initialize_search_index(all_clips).await {
        log::error!("搜索索引初始化失败: {}", e);
    }
}

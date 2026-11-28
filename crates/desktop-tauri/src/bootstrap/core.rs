use std::sync::Arc;

use clipboard_listener::{ClipboardEvent, EventManager};
use log::LevelFilter;

use {
    crate::app_context::AppContext,
    crate::core::application::capture_listener::ClipboardEventTrigger,
    crate::errors::AppResult,
    crate::desktop::logging::init_logging,
};

/// 启动后续阶段需要共享的核心资源。
///
/// 它不是业务上下文，而是启动编排层在 `setup`、`runtime` 阶段传递资源用的结构。
/// 真正给业务代码长期使用的统一入口仍然是 `AppContext`。
#[derive(Clone)]
pub struct BootstrapCore {
    /// 新架构下的应用上下文。
    pub app_context: Arc<AppContext>,
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

    let db_path = crate::desktop::paths::get_data_dir()
        .ok_or_else(|| crate::errors::AppError::Config("无法获取数据目录".into()))?
        .join("clip_record.db");
    let local = desktop_app_core::LocalContext::open_with_resources(
        &db_path,
        crate::desktop::paths::get_config_dir().map(|dir| dir.join("settings.json")),
        Arc::new(crate::desktop::security::decrypt_content),
        crate::desktop::paths::get_resources_dir(),
    ).await?;
    let policy: Arc<dyn crate::core::policy::ApplicationPolicy>;
    #[cfg(feature = "commercial-extension")]
    {
        policy = Arc::new(crate::commercial::sync_policy::CommercialPolicy);
    }
    #[cfg(not(feature = "commercial-extension"))]
    {
        #[cfg(feature = "sync")]
        { policy = Arc::new(crate::sync::policy::SelfhostPolicy {
            max_file_size_bytes: clip_sync_client::protocol::DEFAULT_RECORD_BYTES as u64,
        }); }
        #[cfg(not(feature = "sync"))]
        { policy = Arc::new(crate::core::policy::LocalPolicy); }
    }
    #[cfg(feature = "sync")]
    let application = {
        use clip_sync_client::auth::SyncAuthProvider;
        #[cfg(not(feature = "commercial-extension"))]
        use clip_sync_client::auth::DisabledSyncAuth;
        let auth: Arc<dyn SyncAuthProvider>;
        #[cfg(feature = "commercial-extension")]
        {
            auth = Arc::new(crate::commercial::sync_policy::CommercialSyncAuth {
                endpoint: crate::desktop::config::get_cloud_sync_domain()?.to_owned(),
            });
        }
        #[cfg(not(feature = "commercial-extension"))]
        {
            auth = match crate::desktop::config::get_cloud_sync().map_err(|error| error.to_string())
                .and_then(|config| selfhost_app_core::deployment::DeploymentConfig::new(
                    &config.domain, config.deployment_token.as_deref().unwrap_or(""),
                )) {
                Ok(config) => Arc::new(config),
                Err(error) => {
                    log::warn!("自建同步配置不可用，同步保持禁用: {}", error);
                    Arc::new(DisabledSyncAuth)
                }
            };
        }
        selfhost_app_core::SelfhostContext::with_auth(local, auth)
    };
    #[cfg(not(feature = "sync"))]
    let application = local;

    let clipboard_event_manager: Arc<EventManager<ClipboardEvent>> =
        Arc::new(EventManager::default());
    clipboard_event_manager.add_event_listener(Arc::new(ClipboardEventTrigger));

    // 单机核心先组装；自建版增加同步资源；商业资源作为独立扩展注入。
    let app_context = Arc::new(AppContext::new(
        application,
        #[cfg(feature = "commercial-extension")]
        crate::commercial::context::CommercialContext::new(
            Arc::new(crate::commercial::http::HttpAuthClient),
            Arc::new(crate::commercial::http::HttpVipClient),
            Arc::new(crate::commercial::storage::SecureAuthStore),
            Arc::new(crate::commercial::storage::SecureVipStore),
        ),
        policy,
    ));
    crate::app_context::set_app_context(app_context.clone())?;

    // 数据库打开阶段已过滤旧删除记录；此处清理本机超限记录，再建立搜索索引。
    if let Err(error) = crate::core::application::capture_listener::clean_records(app_context.as_ref()).await {
        log::error!("启动时清理剪贴记录失败: {}", error);
    }
    app_context.core().initialize_search_index().await;

    // 在打开剪贴板监听前恢复本机队列，避免离线启动时漏记第一条复制。
    #[cfg(all(feature = "sync", not(feature = "commercial-extension")))]
    crate::sync::runtime::start(&app_context).await;

    Ok(BootstrapCore {
        app_context,
        clipboard_event_manager,
    })
}

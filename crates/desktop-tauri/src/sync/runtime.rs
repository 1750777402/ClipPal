//! 桌面生命周期只负责启停会话和转发列表变化。
use crate::app_context::AppContext;
use std::sync::Arc;
use tauri::Emitter;

/// 自建同步由安装包明确启用，旧配置默认保持关闭。
pub(crate) fn enabled() -> bool {
    crate::desktop::config::get_cloud_sync().is_ok_and(|config| config.enabled)
}

/// 恢复本机绑定和队列。握手在后台运行，失败时仍能打开单机界面。
pub(crate) async fn start(context: &Arc<AppContext>) {
    if !enabled() { return; }
    let weak = Arc::downgrade(context);
    let notify = Arc::new(move || {
        if let Some(app) = weak.upgrade().and_then(|context| context.try_app_handle()) {
            let _ = app.emit("clip_record_change", ());
        }
    });
    if let Err(error) = context.sync().enable(notify).await {
        log::error!("启动自建文本同步失败：{error}");
    }
}

/// 请求退出。已发送但未确认的动作仍保存在 SQLite，下一次启动会重试。
pub(crate) fn stop(context: &Arc<AppContext>) {
    let context = context.clone();
    tauri::async_runtime::spawn(async move { context.sync().disable().await; });
}

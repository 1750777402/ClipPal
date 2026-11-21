use crate::{
    bootstrap::core::BootstrapCore,
};

#[cfg(feature = "sync")]
use crate::sync::{consumer::consume_clip_record_queue, runtime::start_upload_cloud_timer};

/// 处理 Tauri 运行期事件。
///
/// 它只做生命周期编排，不承载具体业务细节。
pub fn handle_run_event(event: tauri::RunEvent, core: BootstrapCore) {
    match event {
        tauri::RunEvent::ExitRequested { api: _, .. } => handle_exit_requested(&core),
        tauri::RunEvent::Ready { .. } => handle_ready(core),
        _ => {}
    }
}

/// 应用退出前的清理逻辑。
fn handle_exit_requested(core: &BootstrapCore) {
    if let Some(app) = core.app_context.try_app_handle() {
        use tauri::Manager;
        if let Some(clipboard) = app.try_state::<tauri_plugin_clipboard_pal::desktop::ClipboardPal>() {
            if let Err(error) = clipboard.stop_monitor() { log::warn!("停止剪贴板监听失败: {}", error); }
        }
    }
    core.clipboard_event_manager.request_shutdown();
}

/// 应用 Ready 后启动的运行期任务。
///
/// 这些任务放在 Ready 阶段，是为了保持和当前启动顺序一致：
/// - 开始消费剪贴记录同步队列；
/// - 启动文件上传同步定时任务；
/// - 启动剪贴板事件循环；
/// - 用户已登录时初始化 VIP 状态并执行权益限制检查。
fn handle_ready(core: BootstrapCore) {
    #[cfg(feature = "sync")]
    {
        consume_clip_record_queue(core.app_context.sync().record_queue());
        start_upload_cloud_timer();
    }
    core.clipboard_event_manager.start_event_loop();
    #[cfg(feature = "commercial-extension")]
    start_vip_initialization_task(core);
}

/// 用户已登录时初始化 VIP 状态。
#[cfg(feature = "commercial-extension")]
fn start_vip_initialization_task(core: BootstrapCore) {
    use {
    crate::commercial::vip_service::VipService,
    crate::commercial::token_manager,
};
    tokio::spawn(async move {
        if token_manager::has_valid_auth() {
            log::info!("用户已登录，开始初始化VIP状态并执行权益限制检查");
            if let Err(e) = VipService::from_context(core.app_context.as_ref())
                .initialize_and_enforce_limits()
                .await
            {
                log::error!("VIP状态初始化失败: {}", e);
            }
        } else {
            log::info!("用户未登录，跳过VIP状态检查");
        }

        drop(core);
    });
}

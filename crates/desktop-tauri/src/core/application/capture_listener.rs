//! 系统剪贴板事件适配；本地业务由 desktop-app-core 执行。
use clipboard_listener::{ClipBoardEventListener, ClipType, ClipboardEvent};
use tauri::Emitter;
use crate::{app_context::{app_context, AppContext}, errors::AppResult};
use desktop_app_core::capture::CaptureInput;

#[derive(Debug, Clone)]
/// 系统事件监听器，只转换事件并通知页面，本地入库规则归 CaptureService。
pub struct ClipboardEventTrigger;
#[async_trait::async_trait]
impl ClipBoardEventListener<ClipboardEvent> for ClipboardEventTrigger {
    /// 将支持的事件交给采集服务，随后执行本机容量清理并通知列表刷新。
    async fn handle_event(&self, event: &ClipboardEvent) {
        let Ok(context) = app_context() else {
            log::error!("AppContext 尚未初始化，跳过剪贴板事件");
            return;
        };
        // 按事件类型借用数据；不在适配层重新计算内容身份或处理密文。
        let input = match event.r#type {
            ClipType::Text => CaptureInput::Text(&event.content),
            ClipType::Image => CaptureInput::Image(event.file.as_ref()),
            ClipType::File => CaptureInput::Files(event.file_path_vec.as_ref()),
            _ => return,
        };
        let result = context.capture_clipboard(input).await;
        if let Err(error) = &result { log::error!("处理剪贴板事件失败: {}", error); }
        if let Err(error) = clean_records(context.as_ref()).await { log::error!("清理剪贴记录失败: {}", error); }
        // 事件只表示列表可能变化，不向前端广播完整正文或图片字节。
        if let Some(app) = context.try_app_handle() { let _ = app.emit("clip_record_change", ()); }

    }
}

/// 供启动阶段和剪贴板事件共用的本地清理入口，不等待云端确认。
pub async fn clean_records(context: &AppContext) -> AppResult<()> {
    desktop_app_core::cleanup::CleanupService::new(context.core()).try_clean().await
}

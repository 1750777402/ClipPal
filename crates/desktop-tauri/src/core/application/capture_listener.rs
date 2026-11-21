//! 系统剪贴板事件适配；本地业务由 desktop-app-core 执行。
use clipboard_listener::{ClipBoardEventListener, ClipType, ClipboardEvent};
use tauri::Emitter;
use crate::{app_context::{app_context, AppContext}, errors::AppResult};
use desktop_app_core::capture::{CaptureInput, CapturePolicy, CaptureService, DeviceIdentity};

struct DesktopCapturePolicy<'a>(&'a AppContext);
#[async_trait::async_trait]
impl CapturePolicy for DesktopCapturePolicy<'_> {
    fn cached_sync_file_limit(&self) -> u64 {
        self.0.policy().cached_sync_file_limit(self.0).unwrap_or(0)
    }
    async fn file_copy_limit(&self) -> u64 { self.0.policy().file_copy_limit(self.0).await }
}

#[derive(Debug, Clone)]
pub struct ClipboardEventTrigger;
#[async_trait::async_trait]
impl ClipBoardEventListener<ClipboardEvent> for ClipboardEventTrigger {
    async fn handle_event(&self, event: &ClipboardEvent) {
        let Ok(context) = app_context() else {
            log::error!("AppContext 尚未初始化，跳过剪贴板事件");
            return;
        };
        let input = match event.r#type {
            ClipType::Text => CaptureInput::Text(&event.content),
            ClipType::Image => CaptureInput::Image(event.file.as_ref()),
            ClipType::File => CaptureInput::Files(event.file_path_vec.as_ref()),
            _ => return,
        };
        let identity = DeviceIdentity {
            device_id: crate::desktop::device::GLOBAL_DEVICE_ID.clone(),
            os_type: crate::desktop::device::GLOBAL_OS_TYPE.clone(),
        };
        let policy = DesktopCapturePolicy(context.as_ref());
        let encoder = crate::desktop::security::encrypt_content;
        let result = CaptureService::new(context.core(), &encoder, &identity, &policy).capture(input).await;
        if let Err(error) = &result { log::error!("处理剪贴板事件失败: {}", error); }
        if let Err(error) = clean_records(context.as_ref()).await { log::error!("清理剪贴记录失败: {}", error); }
        if let Some(app) = context.try_app_handle() { let _ = app.emit("clip_record_change", ()); }
        #[cfg(feature = "sync")]
        if let Ok(Some(record)) = result {
            if record.sync_flag != Some(clip_core::domain::clip::SKIP_SYNC) && context.cloud_sync_enabled().unwrap_or(false) {
                let queue = context.sync().record_queue();
                if !queue.is_full() { let _ = queue.send_add(record).await; }
            }
        }
    }
}

pub async fn clean_records(context: &AppContext) -> AppResult<()> {
    desktop_app_core::cleanup::CleanupService::new(context.core(), cfg!(feature = "sync")).try_clean().await
}

#[cfg(feature = "sync")]
pub async fn try_clean_clip_record() {
    if let Ok(context) = app_context() {
        if let Err(error) = clean_records(context.as_ref()).await { log::error!("清理剪贴记录失败: {}", error); }
    }
}

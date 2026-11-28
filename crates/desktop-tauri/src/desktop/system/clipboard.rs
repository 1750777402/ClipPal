use tauri::Manager;
use tauri_plugin_clipboard_pal::desktop::ClipboardPal;
use tauri_plugin_dialog::DialogExt;

use {
    crate::app_context::AppContext,
    clip_core::domain::clip::ClipRecord,
};

/// 根据剪贴记录类型，将领域内容写入系统文本、图片或文件剪贴板。
pub async fn write_record(context: &AppContext, record: &ClipRecord) -> Result<(), String> {
    let payload=desktop_app_core::clipboard::prepare_copy(context.core(),record,None).map_err(String::from)?;
    write_payload(context,payload).await
}

/// 将单机核心已校验的内容写入系统；这里只处理系统 API 和临时文件适配。
pub async fn write_payload(context:&AppContext,payload:desktop_app_core::clipboard::ClipboardPayload)->Result<(),String>{
    use desktop_app_core::clipboard::ClipboardPayload;
    let app=context.app_handle().map_err(String::from)?;
    let clipboard=app.state::<ClipboardPal>();
    match payload{
        ClipboardPayload::Text(text)=>clipboard.write_text(text)?,
        ClipboardPayload::Image(path)=>{
            let bytes=tokio::fs::read(path).await.map_err(|error|{log::error!("读取复制图片失败: {}",error);"图片资源不可用".to_string()})?;
            clipboard.write_image_binary(bytes)?;
        }
        ClipboardPayload::Files{names,paths,..}=>{
            // 只把独立副本交给目标应用，保护剪贴板历史中的备份。
            let files = create_named_temp_files(&names, &paths).await?;
            clipboard.write_files_uris(files)?;
        }
    }Ok(())
}

pub use desktop_app_core::temporary_files::create_named_temp_files;

/// 权限提示标记和隐藏保护跟随非阻塞对话框，回调结束后一起释放。
struct PermissionPromptGuard {
    open: std::sync::Arc<std::sync::atomic::AtomicBool>,
    _window: crate::desktop::shell::window::WindowHideGuard,
}
impl PermissionPromptGuard {
    /// 原子抢占提示资格，同时禁止窗口自动隐藏；已有提示时返回 None。
    fn try_acquire(open: std::sync::Arc<std::sync::atomic::AtomicBool>, flag: std::sync::Arc<crate::desktop::shell::window::WindowHideFlag>) -> Option<Self> {
        use std::sync::atomic::Ordering;
        open.compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst).ok()?;
        Some(Self { open, _window: crate::desktop::shell::window::WindowHideGuard::new(flag) })
    }
}
impl Drop for PermissionPromptGuard {
    /// 对话框关闭时释放提示资格，窗口隐藏保护也随字段一并释放。
    fn drop(&mut self) { self.open.store(false, std::sync::atomic::Ordering::SeqCst); }
}

/// 在 macOS 等需要辅助功能权限的平台展示自动粘贴权限提示。
pub fn show_accessibility_dialog(context: &crate::app_context::AppContext) -> crate::errors::AppResult<()> {
    use tauri_plugin_dialog::{MessageDialogButtons, MessageDialogResult};
    if super::accessibility::status() != super::accessibility::AutoPastePermission::Required { return Ok(()); }
    let app = context.app_handle()?;
    let context = app.state::<std::sync::Arc<AppContext>>().inner().clone();
    let open = context.auto_paste_state().permission_prompt_open.clone();
    let Some(guard) = PermissionPromptGuard::try_acquire(open, context.window_hide_flag()) else { return Ok(()); };
    app.dialog()
        .message("内容已复制。自动粘贴需要辅助功能权限。请选择请求系统授权，或打开系统设置 → 隐私与安全性 → 辅助功能，启用 ClipPal。授权后请返回应用重新选择记录粘贴。")
        .title("需要辅助功能权限")
        .kind(tauri_plugin_dialog::MessageDialogKind::Warning)
        .buttons(MessageDialogButtons::YesNoCancelCustom("请求授权".into(), "打开系统设置".into(), "稍后".into()))
        .show_with_result(move |result| {
            // 对话框结束后释放保护；不自动重试粘贴，系统设置不应成为粘贴目标。
            drop(guard);
            match result {
                MessageDialogResult::Yes | MessageDialogResult::Ok => request_native_permission(context),
                MessageDialogResult::No => open_permission_settings(&context),
                MessageDialogResult::Custom(label) if label == "请求授权" => request_native_permission(context),
                MessageDialogResult::Custom(label) if label == "打开系统设置" => open_permission_settings(&context),
                _ => {}
            }
        });
    Ok(())
}

/// 异步请求系统辅助功能授权；只发起请求，不自动重放之前的粘贴操作。
fn request_native_permission(context: std::sync::Arc<AppContext>) {
    tauri::async_runtime::spawn(async move {
        if let Err(error) = super::accessibility::request(&context).await {
            log::error!("请求辅助功能授权失败: {}", error);
        }
    });
}

/// 打开固定的系统辅助功能页面，失败细节仅写入日志。
fn open_permission_settings(context: &AppContext) {
    if let Err(error) = super::accessibility::open_settings(context) {
        log::error!("打开辅助功能设置失败: {}", error);
    }
}

#[cfg(test)]
mod permission_tests {
    use super::PermissionPromptGuard;
    use crate::desktop::shell::window::{WindowHideFlag, WindowHideGuard};
    use std::sync::{Arc, atomic::AtomicBool};

    /// 验证权限提示只显示一次，释放时不破坏其他弹窗的隐藏保护。
    #[test]
    fn permission_prompt_is_unique_and_preserves_other_dialog_guards() {
        let open = Arc::new(AtomicBool::new(false));
        let flag = Arc::new(WindowHideFlag::default());
        let save = WindowHideGuard::new(flag.clone());
        let prompt = PermissionPromptGuard::try_acquire(open.clone(), flag.clone()).unwrap();
        assert!(PermissionPromptGuard::try_acquire(open.clone(), flag.clone()).is_none());
        drop(prompt);
        assert!(!flag.is_can_hide());
        let prompt = PermissionPromptGuard::try_acquire(open, flag.clone()).unwrap();
        drop(save);
        assert!(!flag.is_can_hide());
        drop(prompt);
        assert!(flag.is_can_hide());
    }
}

use clipboard_listener::ClipType;
use tauri::Manager;
use tauri_plugin_clipboard_pal::desktop::ClipboardPal;
use tauri_plugin_dialog::DialogExt;

use {
    crate::app_context::AppContext,
    clip_core::domain::clip::ClipRecord,
    desktop_app_core::content::ContentProcessor,
    crate::desktop::security::decrypt_content,
    clip_core::files::paths::generate_file_not_found_error,
};

/// 根据剪贴记录类型，将领域内容写入系统文本、图片或文件剪贴板。
pub async fn write_record(context: &AppContext, record: &ClipRecord) -> Result<(), String> {
    let app_handle = context.app_handle().map_err(|error| error.to_string())?;
    let clipboard = app_handle.state::<ClipboardPal>();
    // 领域记录保存字符串类型，进入系统层后解析为剪贴板插件支持的类型。
    let clip_type: ClipType = record.r#type.parse().unwrap_or(ClipType::Text);

    match clip_type {
        ClipType::Text => {
            // 文本在数据库中为加密内容，写入系统剪贴板前必须解密。
            let content = decrypt_content(
                ContentProcessor::process_text_content(record.content.clone()).as_str(),
            )
            .map_err(|_| "文本解密失败".to_string())?;
            clipboard.write_text(content)?;
        }
        ClipType::Image => {
            // 图片记录保存资源目录相对路径，系统剪贴板需要实际二进制数据。
            if let Some(path) = record.content.as_str() {
                if let Some(base_path) = crate::desktop::paths::get_resources_dir() {
                    let absolute_path = base_path.join(path);
                    if !absolute_path.exists() {
                        return Err("图片资源不存在，无法复制".to_string());
                    }
                    if let Ok(image_bytes) = std::fs::read(absolute_path) {
                        clipboard.write_image_binary(image_bytes)?;
                    } else {
                        return Err("图片资源读取失败，无法复制".to_string());
                    }
                } else {
                    return Err("资源目录获取失败".to_string());
                }
            } else {
                return Err("图片路径无效".to_string());
            }
        }
        ClipType::File => {
            // 文件显示名称和真实路径分别保存，并通过相同索引建立对应关系。
            let display_names = record.content.as_str().unwrap_or("");
            let actual_paths = record.local_file_path.as_deref().unwrap_or("");

            if display_names.is_empty() || actual_paths.is_empty() {
                return Err("文件信息无效".to_string());
            }

            let display_list: Vec<String> =
                display_names.split(":::").map(str::to_string).collect();
            let actual_list: Vec<String> = actual_paths.split(":::").map(str::to_string).collect();

            // 写入前一次性验证所有真实文件，避免系统剪贴板只包含部分文件。
            let mut not_found = Vec::new();
            for (index, actual_path) in actual_list.iter().enumerate() {
                let actual_path = actual_path.as_str();
                if actual_path.is_empty() {
                    continue;
                }
                if !std::path::Path::new(actual_path).exists() {
                    let display_name = display_list
                        .get(index)
                        .cloned()
                        .unwrap_or_else(|| actual_path.to_string());
                    not_found.push(display_name);
                }
            }
            if !not_found.is_empty() {
                return Err(generate_file_not_found_error(&not_found));
            }

            // 临时文件用于恢复原始显示名称；创建失败时退回真实路径。
            match create_named_temp_files(&display_list, &actual_list).await {
                Ok(temp_files) => {
                    clipboard.write_files_uris(temp_files)?;
                }
                Err(error) => {
                    log::warn!("创建带显示名称的临时文件失败，使用真实路径: {}", error);
                    clipboard.write_files_uris(actual_list)?;
                }
            }
        }
        _ => {}
    }

    Ok(())
}

pub use desktop_app_core::temporary_files::create_named_temp_files;

/// 权限提示标记和隐藏保护跟随非阻塞对话框，回调结束后一起释放。
struct PermissionPromptGuard {
    open: std::sync::Arc<std::sync::atomic::AtomicBool>,
    _window: crate::desktop::shell::window::WindowHideGuard,
}
impl PermissionPromptGuard {
    fn try_acquire(open: std::sync::Arc<std::sync::atomic::AtomicBool>, flag: std::sync::Arc<crate::desktop::shell::window::WindowHideFlag>) -> Option<Self> {
        use std::sync::atomic::Ordering;
        open.compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst).ok()?;
        Some(Self { open, _window: crate::desktop::shell::window::WindowHideGuard::new(flag) })
    }
}
impl Drop for PermissionPromptGuard {
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

fn request_native_permission(context: std::sync::Arc<AppContext>) {
    tauri::async_runtime::spawn(async move {
        if let Err(error) = super::accessibility::request(&context).await {
            log::error!("请求辅助功能授权失败: {}", error);
        }
    });
}

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

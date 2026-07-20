use clipboard_listener::ClipType;
use tauri::{AppHandle, Manager};
use tauri_plugin_clipboard_pal::desktop::ClipboardPal;
use tauri_plugin_dialog::DialogExt;

use crate::{
    app_context::AppContext,
    domain::clip::{ClipRecord, ContentProcessor},
    utils::{aes_util::decrypt_content, path_utils::generate_file_not_found_error},
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
            let _ = clipboard.write_text(content);
        }
        ClipType::Image => {
            // 图片记录保存资源目录相对路径，系统剪贴板需要实际二进制数据。
            if let Some(path) = record.content.as_str() {
                if let Some(base_path) = crate::utils::file_dir::get_resources_dir() {
                    let absolute_path = base_path.join(path);
                    if !absolute_path.exists() {
                        return Err("图片资源不存在，无法复制".to_string());
                    }
                    if let Ok(image_bytes) = std::fs::read(absolute_path) {
                        let _ = clipboard.write_image_binary(image_bytes);
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
                let actual_path = actual_path.trim();
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
                    let _ = clipboard.write_files_uris(temp_files);
                }
                Err(_) => {
                    let _ = clipboard.write_files_uris(actual_list);
                }
            }
        }
        _ => {}
    }

    Ok(())
}

/// 为文件记录创建带显示名称的临时硬链接，必要时退回文件复制。
pub async fn create_named_temp_files(
    display_names: &[String],
    actual_paths: &[String],
) -> Result<Vec<String>, String> {
    use std::path::Path;

    if display_names.len() != actual_paths.len() {
        return Err("显示名称和实际路径数量不匹配".to_string());
    }

    let temp_dir = std::env::temp_dir().join("clip_pal_temp");
    std::fs::create_dir_all(&temp_dir).map_err(|error| format!("创建临时目录失败: {}", error))?;

    let mut temp_file_paths = Vec::new();
    for (display_name, actual_path) in display_names.iter().zip(actual_paths.iter()) {
        let actual_path = actual_path.trim();
        let display_name = display_name.trim();

        if actual_path.is_empty() || display_name.is_empty() {
            continue;
        }

        let source_path = Path::new(actual_path);
        if !source_path.exists() {
            return Err(format!("源文件不存在: {}", actual_path));
        }

        let temp_file_path = temp_dir.join(display_name);
        if temp_file_path.exists() {
            let _ = std::fs::remove_file(&temp_file_path);
        }

        // 同盘优先使用硬链接，跨盘或文件系统不支持时复制文件。
        match std::fs::hard_link(source_path, &temp_file_path) {
            Ok(_) => temp_file_paths.push(temp_file_path.to_string_lossy().to_string()),
            Err(_) => {
                std::fs::copy(source_path, &temp_file_path)
                    .map_err(|error| format!("创建临时文件失败: {}", error))?;
                temp_file_paths.push(temp_file_path.to_string_lossy().to_string());
            }
        }
    }

    if temp_file_paths.is_empty() {
        return Err("没有创建任何临时文件".to_string());
    }

    // 给目标应用留出读取时间，随后异步清理临时文件，避免长期占用磁盘。
    let temp_dir_for_cleanup = temp_dir.clone();
    tokio::spawn(async move {
        tokio::time::sleep(tokio::time::Duration::from_secs(60)).await;
        let _ = cleanup_temp_files(&temp_dir_for_cleanup).await;
    });

    Ok(temp_file_paths)
}

/// 删除临时文件及目录；目录不存在时视为已经清理完成。
async fn cleanup_temp_files(temp_dir: &std::path::Path) -> Result<(), String> {
    if !temp_dir.exists() {
        return Ok(());
    }

    match std::fs::read_dir(temp_dir) {
        Ok(entries) => {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_file() {
                    let _ = std::fs::remove_file(&path);
                }
            }
            let _ = std::fs::remove_dir(temp_dir);
        }
        Err(error) => return Err(format!("读取临时目录失败: {}", error)),
    }

    Ok(())
}

/// 在 macOS 等需要辅助功能权限的平台展示自动粘贴权限提示。
pub fn show_accessibility_dialog(app_handle: &AppHandle) {
    app_handle
        .dialog()
        .message("自动粘贴功能需要辅助功能权限。")
        .title("需要辅助功能权限")
        .kind(tauri_plugin_dialog::MessageDialogKind::Warning)
        .blocking_show();
}

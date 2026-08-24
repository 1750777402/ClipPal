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
            clipboard.write_text(content)?;
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

/// 为文件记录创建带显示名称的临时硬链接，必要时退回文件复制。
pub async fn create_named_temp_files(
    display_names: &[String],
    actual_paths: &[String],
) -> Result<Vec<String>, String> {
    use std::path::{Component, Path};

    if display_names.len() != actual_paths.len() {
        return Err("显示名称和实际路径数量不匹配".to_string());
    }

    // 在创建任何文件前验证所有输入，避免处理中途失败后留下部分临时文件。
    let mut file_entries = Vec::new();
    for (display_name, actual_path) in display_names.iter().zip(actual_paths.iter()) {
        let actual_path = actual_path.trim();
        let display_name = display_name.trim();

        if actual_path.is_empty() || display_name.is_empty() {
            continue;
        }

        let mut components = Path::new(display_name).components();
        let is_single_file_name = matches!(components.next(), Some(Component::Normal(name)) if name == display_name)
            && components.next().is_none();
        if !is_single_file_name {
            return Err(format!("文件显示名称包含无效路径: {}", display_name));
        }

        let source_path = Path::new(actual_path);
        if !source_path.exists() {
            return Err(format!("源文件不存在: {}", actual_path));
        }

        file_entries.push((display_name, source_path));
    }

    if file_entries.is_empty() {
        return Err("没有可创建的临时文件".to_string());
    }

    let temp_root = std::env::temp_dir().join("clip_pal_temp");
    std::fs::create_dir_all(&temp_root).map_err(|error| format!("创建临时目录失败: {}", error))?;
    let temp_root_metadata = std::fs::symlink_metadata(&temp_root)
        .map_err(|error| format!("检查临时目录失败: {}", error))?;
    if !temp_root_metadata.file_type().is_dir() || temp_root_metadata.file_type().is_symlink() {
        return Err("临时目录不是受支持的本地目录".to_string());
    }

    // 每次复制使用独立目录，避免并发操作和同名文件互相覆盖。
    let operation_temp_dir = temp_root.join(uuid::Uuid::new_v4().to_string());
    std::fs::create_dir(&operation_temp_dir)
        .map_err(|error| format!("创建本次临时目录失败: {}", error))?;

    let mut temp_file_paths = Vec::new();
    for (index, (display_name, source_path)) in file_entries.into_iter().enumerate() {
        // 同名文件分别放入独立子目录，目标应用仍会看到原始显示名称。
        let file_temp_dir = operation_temp_dir.join(index.to_string());
        if let Err(error) = std::fs::create_dir(&file_temp_dir) {
            let _ = std::fs::remove_dir_all(&operation_temp_dir);
            return Err(format!("创建文件临时目录失败: {}", error));
        }
        let temp_file_path = file_temp_dir.join(display_name);

        // 同盘优先使用硬链接，跨盘或文件系统不支持时复制文件。
        match std::fs::hard_link(source_path, &temp_file_path) {
            Ok(_) => temp_file_paths.push(temp_file_path.to_string_lossy().to_string()),
            Err(_) => {
                if let Err(error) = std::fs::copy(source_path, &temp_file_path) {
                    let _ = std::fs::remove_dir_all(&operation_temp_dir);
                    return Err(format!("创建临时文件失败: {}", error));
                }
                temp_file_paths.push(temp_file_path.to_string_lossy().to_string());
            }
        }
    }

    // 给目标应用留出读取时间，随后异步清理临时文件，避免长期占用磁盘。
    let temp_dir_for_cleanup = operation_temp_dir.clone();
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

    std::fs::remove_dir_all(temp_dir).map_err(|error| format!("清理临时目录失败: {}", error))?;

    // 没有其他复制任务时顺带删除空的公共根目录。
    if let Some(temp_root) = temp_dir.parent() {
        let _ = std::fs::remove_dir(temp_root);
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

#[cfg(test)]
mod tests {
    use super::{cleanup_temp_files, create_named_temp_files};
    use std::fs;
    use std::path::PathBuf;

    fn test_path(name: &str) -> PathBuf {
        std::env::temp_dir()
            .join(format!(
                "clip-pal-clipboard-test-{}-{}",
                std::process::id(),
                uuid::Uuid::new_v4()
            ))
            .join(name)
    }

    #[tokio::test]
    async fn rejects_display_names_with_path_components() {
        let source_path = test_path("source.txt");
        fs::create_dir_all(source_path.parent().unwrap()).unwrap();
        fs::write(&source_path, b"test").unwrap();

        let result = create_named_temp_files(
            &["../outside.txt".to_string()],
            &[source_path.to_string_lossy().to_string()],
        )
        .await;

        assert!(result.is_err());
        fs::remove_dir_all(source_path.parent().unwrap()).unwrap();
    }

    #[tokio::test]
    async fn keeps_duplicate_display_names_in_separate_paths() {
        let source_dir = test_path("sources");
        fs::create_dir_all(&source_dir).unwrap();
        let first_source = source_dir.join("first.txt");
        let second_source = source_dir.join("second.txt");
        fs::write(&first_source, b"first").unwrap();
        fs::write(&second_source, b"second").unwrap();

        let result = create_named_temp_files(
            &["same.txt".to_string(), "same.txt".to_string()],
            &[
                first_source.to_string_lossy().to_string(),
                second_source.to_string_lossy().to_string(),
            ],
        )
        .await
        .unwrap();

        assert_eq!(result.len(), 2);
        assert_ne!(result[0], result[1]);
        assert_eq!(fs::read(&result[0]).unwrap(), b"first");
        assert_eq!(fs::read(&result[1]).unwrap(), b"second");

        let operation_dir = PathBuf::from(&result[0])
            .parent()
            .and_then(|path| path.parent())
            .unwrap()
            .to_path_buf();
        cleanup_temp_files(&operation_dir).await.unwrap();
        fs::remove_dir_all(source_dir.parent().unwrap()).unwrap();
    }
}

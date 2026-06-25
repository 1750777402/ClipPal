use clipboard_listener::ClipType;
use rbatis::RBatis;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tauri::{AppHandle, Manager};
use tauri_plugin_clipboard_pal::desktop::ClipboardPal;
use tauri_plugin_dialog::DialogExt;

use crate::{
    app_context::AppContext,
    auto_paste,
    biz::{
        clip_record::ClipRecord, content_processor::ContentProcessor,
        content_search::remove_ids_from_index,
    },
    response::{string_result, CommandResponse},
    utils::{
        aes_util::decrypt_content,
        path_utils::{generate_file_not_found_error, str_to_safe_string},
    },
    window::WindowHideGuard,
};

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct CopyClipRecord {
    pub record_id: String,
}

#[tauri::command]
pub async fn copy_clip_record(
    state: tauri::State<'_, Arc<AppContext>>,
    param: CopyClipRecord,
) -> Result<CommandResponse<String>, String> {
    Ok(string_result(async {
        copy_record_to_clipboard(&state, &param).await?;

        let auto_paste_enabled = state
            .with_settings(|settings| settings.auto_paste == 1)
            .unwrap_or(false);

        if auto_paste_enabled {
            let app_handle = state.app_handle().map_err(|e| e.to_string())?;
            std::thread::spawn(move || {
                std::thread::sleep(std::time::Duration::from_millis(100));
                if let Err(e) = auto_paste::auto_paste_to_previous_window() {
                    let error_msg = e.to_string();
                    if error_msg.contains("权限") {
                        let app_handle_for_dialog = app_handle.clone();
                        let _ = app_handle.run_on_main_thread(move || {
                            show_accessibility_permission_dialog(&app_handle_for_dialog);
                        });
                    }
                }
            });
        }

        Ok(String::new())
    }
    .await))
}

#[tauri::command]
pub async fn copy_clip_record_no_paste(
    state: tauri::State<'_, Arc<AppContext>>,
    param: CopyClipRecord,
) -> Result<CommandResponse<String>, String> {
    Ok(string_result(async {
        copy_record_to_clipboard(&state, &param).await?;
        Ok(String::new())
    }
    .await))
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct PinnedClipRecord {
    pub record_id: String,
    pub pinned_flag: i32,
}

#[tauri::command]
pub async fn set_pinned(
    state: tauri::State<'_, Arc<AppContext>>,
    param: PinnedClipRecord,
) -> Result<CommandResponse<String>, String> {
    Ok(string_result(async {
        let rb: &RBatis = state.db();
        let _ = ClipRecord::update_pinned(rb, &param.record_id, param.pinned_flag).await;
        Ok(String::new())
    }
    .await))
}

#[tauri::command]
pub async fn del_record(
    state: tauri::State<'_, Arc<AppContext>>,
    param: CopyClipRecord,
) -> Result<CommandResponse<String>, String> {
    Ok(string_result(async {
        let rb: &RBatis = state.db();
        let ids = vec![param.record_id.clone()];

        let record_result = ClipRecord::select_by_id(rb, &param.record_id).await;
        match record_result {
            Ok(records) => {
                if !records.is_empty() {
                    if ClipRecord::update_del_by_ids(rb, &ids).await.is_ok() {
                        let cloud_sync_enabled = state
                            .with_settings(|settings| settings.cloud_sync == 1)
                            .unwrap_or(false);
                        if cloud_sync_enabled {
                            let async_queue = state.clip_record_queue();
                            if !async_queue.is_full() {
                                let _ = async_queue.send_delete(records[0].clone()).await;
                            }
                        }
                        tokio::spawn(async move {
                            if let Err(e) = remove_ids_from_index(&ids).await {
                                log::error!("从搜索索引删除记录失败: {}", e);
                            }
                        });
                    }
                }
                Ok(String::new())
            }
            Err(_) => Err("未找到该记录".to_string()),
        }
    }
    .await))
}

#[tauri::command]
pub async fn image_save_as(
    state: tauri::State<'_, Arc<AppContext>>,
    param: CopyClipRecord,
) -> Result<CommandResponse<String>, String> {
    Ok(string_result(async {
        let rb: &RBatis = state.db();
        let record_res = ClipRecord::select_by_id(rb, param.record_id.as_str()).await;
        match record_res {
            Ok(records) => {
                let record = records.first().ok_or("未找到指定的剪贴板记录")?;
                if record.r#type != ClipType::Image.to_string() {
                    return Err("仅支持图片类型另存为".to_string());
                }
                let rel_path = record.content.as_str().ok_or("图片路径无效")?;
                let base_path =
                    crate::utils::file_dir::get_resources_dir().ok_or("资源目录获取失败")?;
                let abs_path = base_path.join(rel_path);
                if !abs_path.exists() {
                    return Err("图片资源丢失".to_string());
                }

                let app_handle = state.app_handle().map_err(|e| e.to_string())?;
                let abs_path_clone = abs_path.clone();
                let guard_flag = state.window_hide_flag();
                app_handle
                    .dialog()
                    .file()
                    .add_filter("图片", &["png"])
                    .set_file_name(format!("clip_{}", record.id))
                    .save_file(move |file_path| {
                        let _guard = WindowHideGuard::new(guard_flag.as_ref());
                        if let Some(select_path) = file_path.and_then(|p| p.as_path().map(|p| p.to_path_buf())) {
                            let _ = std::fs::copy(&abs_path_clone, &select_path);
                        }
                    });
                Ok("图片已成功保存".to_string())
            }
            Err(_) => Err("未找到该记录".to_string()),
        }
    }
    .await))
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct CopySingleFileRecord {
    pub record_id: String,
    pub file_path: String,
}

#[tauri::command]
pub async fn copy_single_file(
    state: tauri::State<'_, Arc<AppContext>>,
    param: CopySingleFileRecord,
) -> Result<CommandResponse<String>, String> {
    Ok(string_result(async {
        let rb: &RBatis = state.db();
        let record = match ClipRecord::select_by_id(rb, param.record_id.as_str()).await {
            Ok(data) => data.get(0).cloned().ok_or("记录不存在".to_string())?,
            Err(_) => return Err("剪贴记录查询失败".to_string()),
        };

        if record.r#type != ClipType::File.to_string() {
            return Err("只支持文件类型的单个文件复制".to_string());
        }

        let app_handle = state.app_handle().map_err(|e| e.to_string())?;
        let clipboard = app_handle.state::<ClipboardPal>();

        let display_names = record.content.as_str().unwrap_or("");
        let actual_paths = record.local_file_path.as_deref().unwrap_or("");
        if display_names.is_empty() || actual_paths.is_empty() {
            return Err("文件信息无效".to_string());
        }

        let display_list: Vec<String> = display_names.split(":::").map(|s| s.to_string()).collect();
        let actual_list: Vec<String> = actual_paths.split(":::").map(|s| s.to_string()).collect();

        let file_index = display_list.iter().position(|name| name == &param.file_path);
        let actual_file_path = match file_index {
            Some(index) if index < actual_list.len() => &actual_list[index],
            _ => return Err("指定的文件不在此记录中".to_string()),
        };

        if !std::path::Path::new(actual_file_path).exists() {
            let safe_path = str_to_safe_string(&param.file_path);
            return Err(format!("文件不存在 {}", safe_path));
        }

        match create_temp_files_with_correct_names(
            &[param.file_path.clone()],
            &[actual_file_path.clone()],
        )
        .await
        {
            Ok(temp_files) => {
                let _ = clipboard.write_files_uris(temp_files);
            }
            Err(_) => {
                let _ = clipboard.write_files_uris(vec![actual_file_path.clone()]);
            }
        }

        Ok(String::new())
    }
    .await))
}

async fn copy_record_to_clipboard(
    state: &AppContext,
    param: &CopyClipRecord,
) -> Result<(), String> {
    let rb: &RBatis = state.db();
    let record = match ClipRecord::select_by_id(rb, param.record_id.as_str()).await {
        Ok(data) => data[0].clone(),
        Err(_) => return Err("剪贴记录查询失败".to_string()),
    };

    let app_handle = state.app_handle().map_err(|e| e.to_string())?;
    let clipboard = app_handle.state::<ClipboardPal>();
    let clip_type: ClipType = record.r#type.parse().unwrap_or(ClipType::Text);

    match clip_type {
        ClipType::Text => {
            let content = decrypt_content(
                ContentProcessor::process_text_content(record.content).as_str(),
            )
            .map_err(|_| "文本解密失败".to_string())?;
            let _ = clipboard.write_text(content);
        }
        ClipType::Image => {
            if let Some(path) = record.content.as_str() {
                if let Some(base_path) = crate::utils::file_dir::get_resources_dir() {
                    let abs_path = base_path.join(path);
                    if !abs_path.exists() {
                        return Err("图片资源不存在，无法复制".to_string());
                    }
                    if let Ok(img_bytes) = std::fs::read(abs_path) {
                        let _ = clipboard.write_image_binary(img_bytes);
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
            let display_names = record.content.as_str().unwrap_or("");
            let actual_paths = record.local_file_path.as_deref().unwrap_or("");

            if display_names.is_empty() || actual_paths.is_empty() {
                return Err("文件信息无效".to_string());
            }

            let display_list: Vec<String> =
                display_names.split(":::").map(|s| s.to_string()).collect();
            let actual_list: Vec<String> =
                actual_paths.split(":::").map(|s| s.to_string()).collect();

            let mut not_found: Vec<String> = vec![];
            for (i, actual_path) in actual_list.iter().enumerate() {
                let actual_path = actual_path.trim();
                if actual_path.is_empty() {
                    continue;
                }
                if !std::path::Path::new(actual_path).exists() {
                    let display_name = display_list
                        .get(i)
                        .cloned()
                        .unwrap_or_else(|| actual_path.to_string());
                    not_found.push(display_name);
                }
            }
            if !not_found.is_empty() {
                return Err(generate_file_not_found_error(&not_found));
            }

            match create_temp_files_with_correct_names(&display_list, &actual_list).await {
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

async fn create_temp_files_with_correct_names(
    display_names: &[String],
    actual_paths: &[String],
) -> Result<Vec<String>, String> {
    use std::path::Path;

    if display_names.len() != actual_paths.len() {
        return Err("显示名称和实际路径数量不匹配".to_string());
    }

    let temp_dir = std::env::temp_dir().join("clip_pal_temp");
    std::fs::create_dir_all(&temp_dir).map_err(|e| format!("创建临时目录失败: {}", e))?;

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

        match std::fs::hard_link(source_path, &temp_file_path) {
            Ok(_) => temp_file_paths.push(temp_file_path.to_string_lossy().to_string()),
            Err(_) => {
                std::fs::copy(source_path, &temp_file_path)
                    .map_err(|e| format!("创建临时文件失败: {}", e))?;
                temp_file_paths.push(temp_file_path.to_string_lossy().to_string());
            }
        }
    }

    if temp_file_paths.is_empty() {
        return Err("没有创建任何临时文件".to_string());
    }

    let temp_dir_for_cleanup = temp_dir.clone();
    tokio::spawn(async move {
        tokio::time::sleep(tokio::time::Duration::from_secs(60)).await;
        let _ = cleanup_temp_files(&temp_dir_for_cleanup).await;
    });

    Ok(temp_file_paths)
}

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
        Err(e) => return Err(format!("读取临时目录失败: {}", e)),
    }

    Ok(())
}

fn show_accessibility_permission_dialog(app_handle: &AppHandle) {
    let message = "自动粘贴功能需要辅助功能权限。";

    app_handle
        .dialog()
        .message(message)
        .title("需要辅助功能权限")
        .kind(tauri_plugin_dialog::MessageDialogKind::Warning)
        .blocking_show();
}

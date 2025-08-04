use clipboard_listener::ClipType;

use rbatis::RBatis;
use serde::{Deserialize, Serialize};
use std::sync::{Arc, RwLock};
use tauri::{AppHandle, Manager};
use tauri_plugin_clipboard_pal::desktop::ClipboardPal;
use tauri_plugin_dialog::DialogExt;

use crate::{
    CONTEXT, auto_paste,
    biz::{
        clip_record::ClipRecord, content_processor::ContentProcessor,
        content_search::remove_ids_from_index, system_setting::Settings,
    },
    errors::lock_utils::safe_read_lock,
    utils::{
        aes_util::decrypt_content,
        path_utils::{generate_file_not_found_error, str_to_safe_string},
    },
    window::{WindowHideFlag, WindowHideGuard},
};

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct CopyClipRecord {
    pub record_id: String,
}

#[tauri::command]
pub async fn copy_clip_record(param: CopyClipRecord) -> Result<String, String> {
    let rb: &RBatis = CONTEXT.get::<RBatis>();
    let record = match ClipRecord::select_by_id(rb, param.record_id.as_str()).await {
        Ok(data) => data[0].clone(),
        Err(_) => return Err("粘贴记录查询失败".to_string()),
    };

    let app_handle = CONTEXT.get::<AppHandle>();
    let clipboard = app_handle.state::<ClipboardPal>();
    let clip_type: ClipType = record.r#type.parse().unwrap_or(ClipType::Text);

    match clip_type {
        ClipType::Text => {
            let content = match decrypt_content(
                ContentProcessor::process_text_content(record.content).as_str(),
            ) {
                Ok(text) => text,
                Err(e) => {
                    log::error!("解密文本内容失败: {}", e);
                    return Err("文本解密失败".to_string());
                }
            };
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
            if let Some(paths) = record.content.as_str() {
                let restored: Vec<String> = paths.split(":::").map(|s| s.to_string()).collect();
                let mut not_found: Vec<String> = vec![];
                for file_path in &restored {
                    let file_path = file_path.trim();
                    if file_path.is_empty() {
                        continue;
                    }
                    if !std::path::Path::new(file_path).exists() {
                        not_found.push(file_path.to_string());
                    }
                }
                if !not_found.is_empty() {
                    return Err(generate_file_not_found_error(&not_found));
                }
                let _ = clipboard.write_files_uris(restored);
            } else {
                return Err("文件路径无效".to_string());
            }
        }
        _ => {}
    }

    // 检查是否启用自动粘贴功能
    let auto_paste_enabled = {
        let settings_lock = CONTEXT.get::<Arc<RwLock<Settings>>>();
        match safe_read_lock(&settings_lock) {
            Ok(settings) => settings.auto_paste == 1,
            Err(e) => {
                log::warn!("无法获取设置: {}", e);
                false // 如果无法获取设置，默认不启用自动粘贴
            }
        }
    };

    // 只有在启用自动粘贴时才执行
    if auto_paste_enabled {
        // 使用异步任务避免阻塞主线程
        tokio::spawn(async {
            // 等待一小段时间确保剪贴板内容已经更新
            tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

            // 尝试自动粘贴到之前获得焦点的窗口
            if let Err(e) = auto_paste::auto_paste_to_previous_window() {
                log::warn!("自动粘贴失败: {}", e);
                // 自动粘贴失败不影响复制功能，只记录警告日志
            }
        });
    }

    Ok(String::new())
}

/// 只复制到剪贴板，不触发自动粘贴功能
#[tauri::command]
pub async fn copy_clip_record_no_paste(param: CopyClipRecord) -> Result<String, String> {
    let rb: &RBatis = CONTEXT.get::<RBatis>();
    let record = match ClipRecord::select_by_id(rb, param.record_id.as_str()).await {
        Ok(data) => data[0].clone(),
        Err(_) => return Err("粘贴记录查询失败".to_string()),
    };

    let app_handle = CONTEXT.get::<AppHandle>();
    let clipboard = app_handle.state::<ClipboardPal>();
    let clip_type: ClipType = record.r#type.parse().unwrap_or(ClipType::Text);

    match clip_type {
        ClipType::Text => {
            let content = match decrypt_content(
                ContentProcessor::process_text_content(record.content).as_str(),
            ) {
                Ok(text) => text,
                Err(e) => {
                    log::error!("解密文本内容失败: {}", e);
                    return Err("文本解密失败".to_string());
                }
            };
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
            if let Some(paths) = record.content.as_str() {
                let restored: Vec<String> = paths.split(":::").map(|s| s.to_string()).collect();
                let mut not_found: Vec<String> = vec![];
                for file_path in &restored {
                    let file_path = file_path.trim();
                    if file_path.is_empty() {
                        continue;
                    }
                    if !std::path::Path::new(file_path).exists() {
                        not_found.push(file_path.to_string());
                    }
                }
                if !not_found.is_empty() {
                    return Err(generate_file_not_found_error(&not_found));
                }
                let _ = clipboard.write_files_uris(restored);
            } else {
                return Err("文件路径无效".to_string());
            }
        }
        _ => {}
    }

    // 注意：这个函数不执行自动粘贴功能
    log::debug!("仅复制到剪贴板，不触发自动粘贴");
    Ok(String::new())
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct PinnedClipRecord {
    pub record_id: String,
    pub pinned_flag: i32,
}

#[tauri::command]
pub async fn set_pinned(param: PinnedClipRecord) -> Result<String, String> {
    let rb: &RBatis = CONTEXT.get::<RBatis>();
    let _ = ClipRecord::update_pinned(rb, &param.record_id, param.pinned_flag).await;
    Ok(String::new())
}

#[tauri::command]
pub async fn del_record(param: CopyClipRecord) -> Result<String, String> {
    let rb: &RBatis = CONTEXT.get::<RBatis>();
    let ids = vec![param.record_id];
    let res = ClipRecord::update_del_by_ids(rb, &ids).await;
    if let Ok(_) = res {
        // 异步从搜索索引中移除记录
        tokio::spawn(async move {
            if let Err(e) = remove_ids_from_index(&ids).await {
                log::error!("从搜索索引删除记录失败: {}", e);
            }
        });
    }
    Ok(String::new())
}

#[tauri::command]
pub async fn image_save_as(param: CopyClipRecord) -> Result<String, String> {
    let rb: &RBatis = CONTEXT.get::<RBatis>();
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

            let window_hide_flag = CONTEXT.get::<WindowHideFlag>();
            // 用Arc包裹WindowHideGuard，延长生命周期到回调闭包
            let guard = Arc::new(WindowHideGuard::new(window_hide_flag));
            let app_handle = CONTEXT.get::<AppHandle>();
            let abs_path_clone = abs_path.clone();
            let guard_clone = guard.clone();
            app_handle
                .dialog()
                .file()
                .add_filter("图片", &["png"])
                .set_file_name(format!("clip_{}", record.id))
                .save_file(move |file_path| {
                    // guard_clone在闭包内，作用域结束时自动drop，恢复窗口可隐藏
                    let _guard = guard_clone;
                    if let Some(select_path) = file_path {
                        let select_path = select_path.as_path();
                        if let Some(select_path) = select_path {
                            if let Err(e) = std::fs::copy(&abs_path_clone, &select_path) {
                                let source_path = abs_path_clone.to_string_lossy();
                                let dest_path = select_path.to_string_lossy();
                                log::error!(
                                    "复制图片失败: {}, 源文件: {}, 目标文件: {}",
                                    e,
                                    source_path,
                                    dest_path
                                );
                            }
                        }
                    }
                });
            Ok("图片已成功保存".to_string())
        }
        Err(_) => Err("未找到该记录".to_string()),
    }
}

/// 复制单个文件
#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct CopySingleFileRecord {
    pub record_id: String,
    pub file_path: String,
}

#[tauri::command]
pub async fn copy_single_file(param: CopySingleFileRecord) -> Result<String, String> {
    let rb: &RBatis = CONTEXT.get::<RBatis>();
    let record = match ClipRecord::select_by_id(rb, param.record_id.as_str()).await {
        Ok(data) => data.get(0).cloned().ok_or("记录不存在".to_string())?,
        Err(_) => return Err("粘贴记录查询失败".to_string()),
    };

    // 只处理文件类型
    if record.r#type != ClipType::File.to_string() {
        return Err("只支持文件类型的单个文件复制".to_string());
    }

    let app_handle = CONTEXT.get::<AppHandle>();
    let clipboard = app_handle.state::<ClipboardPal>();

    // 检查指定的文件路径是否存在于记录中
    if let Some(paths) = record.content.as_str() {
        let restored: Vec<String> = paths.split(":::").map(|s| s.to_string()).collect();

        // 验证指定的文件路径是否在记录中
        if !restored.contains(&param.file_path) {
            return Err("指定的文件路径不在此记录中".to_string());
        }

        // 检查文件是否存在
        if !std::path::Path::new(&param.file_path).exists() {
            let safe_path = str_to_safe_string(&param.file_path);
            return Err(format!("文件不存在: {}", safe_path));
        }

        // 复制单个文件
        let _ = clipboard.write_files_uris(vec![param.file_path]);
    } else {
        return Err("文件路径无效".to_string());
    }

    log::debug!("已复制单个文件到剪贴板");
    Ok(String::new())
}

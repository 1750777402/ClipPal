use clipboard_listener::ClipType;
use log::error;
use rbatis::RBatis;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tauri::{AppHandle, Manager};
use tauri_plugin_clipboard_pal::desktop::ClipboardPal;
use tauri_plugin_dialog::DialogExt;

use crate::{
    CONTEXT,
    biz::{
        clip_record::ClipRecord, content_processor::ContentProcessor,
        tokenize_bin::remove_ids_from_token_bin,
    },

    utils::aes_util::decrypt_content,
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
            let content = match decrypt_content(ContentProcessor::process_text_content(record.content).as_str()) {
                Ok(text) => text,
                Err(e) => {
                    error!("解密文本内容失败: {}", e);
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
                    return Err(format!(
                        "以下文件不存在，无法复制:\n{}",
                        not_found.join("\n")
                    ));
                }
                let _ = clipboard.write_files_uris(restored);
            } else {
                return Err("文件路径无效".to_string());
            }
        }
        _ => {}
    }

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
    let res = ClipRecord::del_by_ids(rb, &ids).await;
    if let Ok(_) = res {
        let _ = remove_ids_from_token_bin(&ids);
    }
    Ok(String::new())
}

#[tauri::command]
pub async fn image_save_as(param: CopyClipRecord) -> Result<String, String> {
    let rb: &RBatis = CONTEXT.get::<RBatis>();
    let record_res = ClipRecord::select_by_id(rb, param.record_id.as_str()).await;
    match record_res {
        Ok(records) => {
            let record = records.first()
                .ok_or("未找到指定的剪贴板记录")?;
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
                                error!("Copy image error: {}", e);
                            }
                        }
                    }
                });
            Ok("图片已成功保存".to_string())
        }
        Err(_) => Err("未找到该记录".to_string()),
    }
}

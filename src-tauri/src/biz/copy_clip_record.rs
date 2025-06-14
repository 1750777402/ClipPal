use std::fs;

use clipboard_listener::ClipType;
use rbatis::RBatis;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tauri::{AppHandle, Manager};
use tauri_plugin_clipboard_pal::desktop::ClipboardPal;

use crate::{CONTEXT, biz::clip_record::ClipRecord, utils::file_dir::get_resources_dir};

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
    // let content: String = from_str(&record.content).unwrap_or_default();
    match clip_type {
        ClipType::Text => {
            let raw_content = match record.content.clone() {
                Value::String(s) => {
                    // 普通文本
                    s
                }
                Value::Object(obj) => {
                    // JSON对象，比如 {"parent_id": "xxx"}
                    serde_json::to_string(&obj).unwrap_or_default()
                }
                Value::Array(arr) => {
                    // JSON数组，比如 ["a", "b"]
                    serde_json::to_string(&arr).unwrap_or_default()
                }
                _ => String::new(),
            };
            let _ = clipboard.write_text(raw_content);
        }
        ClipType::Image => {
            let base_path = get_resources_dir().unwrap();
            let abs_path = base_path.join(record.content.as_str().unwrap_or_default().to_string());
            println!("abs_path:{:?}", abs_path.clone());
            match fs::read(abs_path) {
                Ok(img_bytes) => {
                    let _ = clipboard.write_image_binary(img_bytes);
                }
                Err(e) => println!("图片粘贴失败:{}", e),
            }
        }
        ClipType::File => {
            let restored: Vec<String> = record
                .content
                .as_str()
                .unwrap_or_default()
                .to_string()
                .split(":::")
                .map(|s| s.to_string())
                .collect();
            let _ = clipboard.write_files_uris(restored);
        }
        _ => {}
    }
    Ok(String::new())
}

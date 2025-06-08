use std::{env::current_dir, fs};

use clipboard_listener::ClipType;
use rbatis::RBatis;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};
use tauri_plugin_clipboard_pal::desktop::ClipboardPal;

use crate::{biz::clip_record::ClipRecord, utils::file_dir::get_resources_dir, CONTEXT};

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
            let _ = clipboard.write_text(record.content);
        }
        ClipType::Image => {
            let base_path = get_resources_dir().unwrap();
            let abs_path = base_path.join(record.content.to_string());
            if let Ok(img_bytes) = fs::read(abs_path) {
                let _ = clipboard.write_image_binary(img_bytes);
            }
        }
        ClipType::File => {
            let restored: Vec<String> =
                record.content.split(":::").map(|s| s.to_string()).collect();
            let _ = clipboard.write_files_uris(restored);
        }
        _ => {}
    }
    Ok(String::new())
}

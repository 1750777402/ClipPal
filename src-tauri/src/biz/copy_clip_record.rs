use clipboard_listener::ClipType;
use rbatis::RBatis;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};
use tauri_plugin_clipboard_pal::desktop::ClipboardPal;

use crate::{
    CONTEXT,
    biz::{clip_record::ClipRecord, content_processor::ContentProcessor},
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
            let content = ContentProcessor::process_text_content(record.content);
            let _ = clipboard.write_text(content);
        }
        ClipType::Image => {
            if let Some(path) = record.content.as_str() {
                if let Some(base_path) = crate::utils::file_dir::get_resources_dir() {
                    let abs_path = base_path.join(path);
                    if let Ok(img_bytes) = std::fs::read(abs_path) {
                        let _ = clipboard.write_image_binary(img_bytes);
                    }
                }
            }
        }
        ClipType::File => {
            if let Some(paths) = record.content.as_str() {
                let restored: Vec<String> = paths.split(":::").map(|s| s.to_string()).collect();
                let _ = clipboard.write_files_uris(restored);
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
    let _ = ClipRecord::del_by_ids(rb, vec![param.record_id]).await;
    Ok(String::new())
}

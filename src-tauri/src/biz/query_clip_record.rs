use std::path::Path;

use base64::{Engine as _, engine::general_purpose};
use clipboard_listener::ClipType;
use rbatis::RBatis;
use serde::{Deserialize, Serialize};
use serde_json::from_str;
use std::fs;

use crate::{CONTEXT, biz::clip_record::ClipRecord, utils::file_dir::get_resources_dir};

#[derive(Debug, Serialize, Deserialize)]
pub struct QueryParam {
    pub page: i32,
    pub size: i32,
}

#[tauri::command]
pub async fn get_clip_records(param: QueryParam) -> Vec<ClipRecord> {
    let offset = (param.page - 1) * param.size;
    let rb: &RBatis = CONTEXT.get::<RBatis>();
    let mut all_data = match ClipRecord::select_order_by_limit(rb, param.size, offset).await {
        Ok(data) => data,
        Err(e) => {
            println!("查询粘贴记录失败:{:?}", e);
            return vec![];
        }
    };
    if all_data.is_empty() {
        return vec![];
    }
    let base_path = match get_resources_dir() {
        Some(p) => p,
        None => return all_data,
    };

    for item in &mut all_data {
        let content: String = from_str(&item.content).unwrap_or_default();
        if item.r#type == ClipType::Text.to_string() {
            item.content = content.clone();
        }
        if item.r#type == ClipType::Image.to_string() {
            let abs_path = base_path.join(content.clone());
            if let Some(base64_img) = file_to_base64(&abs_path) {
                item.content = base64_img;
            }
        }
        if item.r#type == ClipType::File.to_string() {
            let restored: Vec<String> = content.split(":::").map(|s| s.to_string()).collect();
            item.content = serde_json::to_string(&restored).unwrap_or("".to_string());
        }
    }

    all_data
}

pub fn file_to_base64(file_path: &Path) -> Option<String> {
    let bytes = fs::read(file_path).ok()?;
    let encoded = general_purpose::STANDARD.encode(&bytes);
    let ext = file_path.extension()?.to_str()?.to_lowercase();

    let mime = match ext.as_str() {
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        _ => "application/octet-stream",
    };

    Some(format!("data:{};base64,{}", mime, encoded))
}

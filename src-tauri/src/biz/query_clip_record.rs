use std::path::Path;

use base64::{Engine as _, engine::general_purpose};
use clipboard_listener::ClipType;
use rbatis::RBatis;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fs;

use crate::{CONTEXT, biz::clip_record::ClipRecord, utils::file_dir::get_resources_dir};

#[derive(Debug, Serialize, Deserialize)]
pub struct QueryParam {
    pub page: i32,
    pub size: i32,
    pub search: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ClipRecordDTO {
    pub id: String,
    // 类型
    pub r#type: String,
    // 内容
    pub content: String,
    // os类型
    pub os_type: String,
}

#[tauri::command]
pub async fn get_clip_records(param: QueryParam) -> Vec<ClipRecordDTO> {
    let offset = (param.page - 1) * param.size;
    let rb: &RBatis = CONTEXT.get::<RBatis>();
    // 执行数据库查询逻辑
    let query_result = match param.search.as_deref().filter(|s| !s.is_empty()) {
        Some(search) => {
            let content: String = format!("%{}%", search);
            ClipRecord::select_where_order_by_limit(rb, content.as_str(), param.size, offset).await
        }
        None => ClipRecord::select_order_by_limit(rb, param.size, offset).await,
    };
    let mut all_data = match query_result {
        Ok(data) => data,
        Err(e) => {
            eprintln!("查询粘贴记录失败: {:?}", e);
            return vec![];
        }
    };

    if all_data.is_empty() {
        return vec![];
    }
    let mut res_data: Vec<ClipRecordDTO> = vec![];

    for item in &mut all_data {
        let mut dto = ClipRecordDTO {
            id: item.id.clone(),
            r#type: item.r#type.clone(),
            content: String::new(),
            os_type: item.os_type.clone(),
        };
        let raw_content = match item.content.clone() {
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
        match dto.r#type.as_str() {
            t if t == ClipType::Text.to_string() => {
                dto.content = raw_content;
            }
            t if t == ClipType::Image.to_string() => {
                let base_path = match get_resources_dir() {
                    Some(p) => p,
                    None => return res_data,
                };
                let abs_path = base_path.join(&raw_content);
                if let Some(base64_img) = file_to_base64(&abs_path) {
                    dto.content = base64_img;
                }
            }
            t if t == ClipType::File.to_string() => {
                let restored: Vec<String> =
                    raw_content.split(":::").map(|s| s.to_string()).collect();
                dto.content = serde_json::to_string(&restored).unwrap_or_default();
            }
            _ => {}
        }
        res_data.push(dto);
    }

    res_data
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

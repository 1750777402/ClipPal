use clipboard_listener::ClipType;
use rbatis::RBatis;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;
use std::sync::Arc;

use crate::{
    app_context::AppContext,
    biz::{
        clip_record::ClipRecord, content_processor::ContentProcessor,
        content_search::search_ids_by_content,
    },
    errors::{AppError, AppResult},
    response::{command_result, string_result, CommandResponse},
};

#[derive(Debug, Serialize, Deserialize)]
pub struct QueryParam {
    pub page: i32,
    pub size: i32,
    pub search: Option<String>,
}

#[allow(dead_code)]
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ClipRecordDTO {
    pub id: String,
    pub r#type: String,
    pub content: String,
    pub os_type: String,
    pub created: u64,
    pub pinned_flag: i32,
    pub file_info: Vec<FileInfo>,
    pub image_info: Option<ImageInfo>,
    pub sync_flag: Option<i32>,
    pub cloud_source: Option<i32>,
    pub content_truncated: bool,
    pub original_content_length: Option<usize>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ClipRecordLiteDTO {
    pub id: String,
    pub r#type: String,
    pub content: String,
    pub os_type: String,
    pub created: u64,
    pub pinned_flag: i32,
    pub file_info: Vec<FileInfo>,
    pub sync_flag: Option<i32>,
    pub cloud_source: Option<i32>,
    pub content_truncated: bool,
    pub original_content_length: Option<usize>,
    pub has_image: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FileInfo {
    pub path: String,
    pub size: i32,
    pub r#type: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ImageInfo {
    pub path: String,
    pub size: u64,
    pub width: Option<u32>,
    pub height: Option<u32>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GetImageParam {
    pub record_id: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GetFullContentParam {
    pub record_id: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct FullContentResponse {
    pub id: String,
    pub content: String,
    pub content_length: usize,
}

#[tauri::command]
pub async fn get_clip_records(
    state: tauri::State<'_, Arc<AppContext>>,
    param: QueryParam,
) -> Result<CommandResponse<Vec<ClipRecordLiteDTO>>, String> {
    Ok(command_result(query_clip_records(state.db(), param).await))
}

async fn query_clip_records(rb: &RBatis, param: QueryParam) -> AppResult<Vec<ClipRecordLiteDTO>> {
    let offset = (param.page - 1) * param.size;
    let query_result = match param.search.as_deref().filter(|s| !s.is_empty()) {
        Some(search) => {
            let res_ids = search_ids_by_content(search).await;
            ClipRecord::select_by_ids(rb, &res_ids, param.size, offset).await
        }
        None => ClipRecord::select_order_by_limit(rb, param.size, offset).await,
    };

    let all_data = match query_result {
        Ok(data) => data,
        Err(e) => {
            log::error!("查询剪贴记录失败: {:?}", e);
            return Err(AppError::Database(e));
        }
    };

    if all_data.is_empty() {
        return Ok(vec![]);
    }

    Ok(all_data
        .into_iter()
        .map(|item| {
            if item.r#type == ClipType::File.to_string() {
                let content_str = item.content.as_str().unwrap_or_default().to_string();
                let local_paths = item
                    .local_file_path
                    .as_deref()
                    .unwrap_or_default()
                    .to_string();
                let content =
                    ContentProcessor::process_by_clip_type(&item.r#type, item.content.clone());
                ClipRecordLiteDTO {
                    id: item.id.clone(),
                    r#type: item.r#type.clone(),
                    content,
                    os_type: item.os_type.clone(),
                    created: item.created,
                    pinned_flag: item.pinned_flag,
                    file_info: get_file_info_with_paths(content_str, local_paths),
                    sync_flag: item.sync_flag,
                    cloud_source: item.cloud_source,
                    content_truncated: false,
                    original_content_length: None,
                    has_image: false,
                }
            } else if item.r#type == ClipType::Image.to_string() {
                let image_path = item.content.as_str().unwrap_or_default();
                ClipRecordLiteDTO {
                    id: item.id.clone(),
                    r#type: item.r#type.clone(),
                    content: image_path.to_string(),
                    os_type: item.os_type.clone(),
                    created: item.created,
                    pinned_flag: item.pinned_flag,
                    file_info: vec![],
                    sync_flag: item.sync_flag,
                    cloud_source: item.cloud_source,
                    content_truncated: false,
                    original_content_length: None,
                    has_image: true,
                }
            } else {
                let processed_content =
                    ContentProcessor::process_by_clip_type(&item.r#type, item.content.clone());
                let (truncated_content, is_truncated, original_length) =
                    truncate_large_text(&processed_content);

                ClipRecordLiteDTO {
                    id: item.id.clone(),
                    r#type: item.r#type.clone(),
                    content: truncated_content,
                    os_type: item.os_type.clone(),
                    created: item.created,
                    pinned_flag: item.pinned_flag,
                    file_info: vec![],
                    sync_flag: item.sync_flag,
                    cloud_source: item.cloud_source,
                    content_truncated: is_truncated,
                    original_content_length: original_length,
                    has_image: false,
                }
            }
        })
        .collect())
}

pub fn get_file_info_with_paths(content_names: String, local_paths: String) -> Vec<FileInfo> {
    let display_names = content_names.split(":::").collect::<Vec<&str>>();
    let actual_paths = local_paths.split(":::").collect::<Vec<&str>>();
    let min_len = display_names.len().min(actual_paths.len());

    (0..min_len)
        .filter_map(|i| {
            let display_name = display_names[i].trim();
            let actual_path = actual_paths[i].trim();

            if display_name.is_empty() || actual_path.is_empty() {
                return None;
            }

            let path_buf = Path::new(actual_path);
            let file_type = Path::new(display_name)
                .extension()
                .and_then(|ext| ext.to_str())
                .unwrap_or("unknown")
                .to_lowercase();

            if !path_buf.exists() {
                return Some(FileInfo {
                    path: display_name.to_string(),
                    size: -1,
                    r#type: file_type,
                });
            }

            let metadata = match fs::metadata(path_buf) {
                Ok(meta) => meta,
                Err(_) => {
                    return Some(FileInfo {
                        path: display_name.to_string(),
                        size: -2,
                        r#type: file_type,
                    });
                }
            };

            let size = if metadata.len() > i32::MAX as u64 {
                i32::MAX
            } else {
                metadata.len() as i32
            };

            Some(FileInfo {
                path: display_name.to_string(),
                size,
                r#type: file_type,
            })
        })
        .collect()
}

pub fn get_image_info(relative_path: &str) -> Option<ImageInfo> {
    if relative_path.is_empty() {
        return None;
    }

    let base_path = crate::utils::file_dir::get_resources_dir()?;
    let abs_path = base_path.join(relative_path);

    if !abs_path.exists() {
        return None;
    }

    let metadata = fs::metadata(&abs_path).ok()?;

    Some(ImageInfo {
        path: relative_path.to_string(),
        size: metadata.len(),
        width: None,
        height: None,
    })
}

#[tauri::command]
pub async fn get_image_path(
    state: tauri::State<'_, Arc<AppContext>>,
    param: GetImageParam,
) -> Result<CommandResponse<ImagePathInfo>, String> {
    Ok(string_result(async {
        let records = ClipRecord::select_by_id(state.db(), &param.record_id)
            .await
            .map_err(|e| format!("数据库查询失败: {}", e))?;

        let clip_record = records.first().ok_or("记录不存在")?;

        if clip_record.r#type != "Image" {
            return Err("记录不是图片类型".to_string());
        }

        if let Some(cache_file_path) = &clip_record.local_file_path {
            if std::path::Path::new(cache_file_path).exists() {
                return Ok(ImagePathInfo {
                    id: clip_record.id.clone(),
                    file_path: cache_file_path.to_string(),
                    protocol_url: String::new(),
                });
            }
        }

        if let Some(filename) = clip_record.content.as_str() {
            use crate::utils::file_dir::get_resources_dir;

            if let Some(resources_dir) = get_resources_dir() {
                let image_path = resources_dir.join(filename);
                if image_path.exists() {
                    let absolute_path = image_path.to_string_lossy().to_string();
                    return Ok(ImagePathInfo {
                        id: clip_record.id.clone(),
                        file_path: absolute_path.clone(),
                        protocol_url: String::new(),
                    });
                }
            }
        }

        Err("图片文件不存在".to_string())
    }
    .await))
}

#[derive(Serialize)]
pub struct ImagePathInfo {
    pub id: String,
    pub file_path: String,
    pub protocol_url: String,
}

fn truncate_large_text(content: &str) -> (String, bool, Option<usize>) {
    const MAX_PREVIEW_SIZE: usize = 8 * 1024;
    if content.len() <= MAX_PREVIEW_SIZE {
        (content.to_string(), false, None)
    } else {
        let mut end_pos = MAX_PREVIEW_SIZE;

        while end_pos > 0 && !content.is_char_boundary(end_pos) {
            end_pos -= 1;
        }

        if end_pos == 0 {
            end_pos = content
                .char_indices()
                .nth(1000)
                .map(|(i, _)| i)
                .unwrap_or(content.len().min(4096));
        }

        let truncated = content[..end_pos].to_string();
        (truncated, true, Some(content.len()))
    }
}

#[tauri::command]
pub async fn get_image_info_batch(
    state: tauri::State<'_, Arc<AppContext>>,
    record_ids: Vec<String>,
) -> Result<CommandResponse<std::collections::HashMap<String, ImageInfo>>, String> {
    Ok(string_result(async {
        use std::collections::HashMap;

        let mut result = HashMap::new();

        for id in record_ids {
            match ClipRecord::select_by_id(state.db(), &id).await {
                Ok(records) => {
                    if let Some(record) = records.first() {
                        if record.r#type == ClipType::Image.to_string() {
                            let image_path = record.content.as_str().unwrap_or_default();
                            if let Some(info) = get_image_info(image_path) {
                                result.insert(id, info);
                            }
                        }
                    }
                }
                Err(e) => {
                    log::warn!("获取图片信息失败，记录ID: {}, 错误: {}", id, e);
                }
            }
        }

        Ok(result)
    }
    .await))
}

#[tauri::command]
pub async fn get_full_text_content(
    state: tauri::State<'_, Arc<AppContext>>,
    param: GetFullContentParam,
) -> Result<CommandResponse<FullContentResponse>, String> {
    Ok(string_result(async {
        let records = ClipRecord::select_by_id(state.db(), &param.record_id)
            .await
            .map_err(|e| format!("查询记录失败: {}", e))?;

        let record = records.first().ok_or("记录不存在")?;

        if record.r#type != ClipType::Text.to_string() {
            return Err("记录类型不是文本".to_string());
        }

        let full_content =
            ContentProcessor::process_by_clip_type(&record.r#type, record.content.clone());

        Ok(FullContentResponse {
            id: param.record_id,
            content: full_content.clone(),
            content_length: full_content.len(),
        })
    }
    .await))
}

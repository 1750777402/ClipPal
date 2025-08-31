use clipboard_listener::ClipType;
use rbatis::RBatis;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

use crate::{
    CONTEXT,
    biz::{
        clip_record::ClipRecord, content_processor::ContentProcessor,
        content_search::search_ids_by_content,
    },
};

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
    // 内容 - 对于图片类型，这里只存储文件路径，不转换为base64
    pub content: String,
    // os类型
    pub os_type: String,
    // 创建时间
    pub created: u64,
    // 是否置顶
    pub pinned_flag: i32,
    // 文件内容属性
    pub file_info: Vec<FileInfo>,
    // 图片预览信息（仅用于图片类型）
    pub image_info: Option<ImageInfo>,
    // 是否已同步标识
    pub sync_flag: Option<i32>,
    // 数据来源标识
    pub cloud_source: Option<i32>,
    // 内容是否被截断
    pub content_truncated: bool,
    // 原始内容长度（字节）
    pub original_content_length: Option<usize>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FileInfo {
    // 文件路径
    pub path: String,
    // 文件大小
    pub size: i32,
    // 文件类型
    pub r#type: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ImageInfo {
    // 图片文件路径（相对路径）
    pub path: String,
    // 图片文件大小（字节）
    pub size: u64,
    // 图片尺寸信息（可选）
    pub width: Option<u32>,
    pub height: Option<u32>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GetImageParam {
    pub record_id: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ImageBase64Response {
    pub id: String,
    pub base64_data: String,
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
pub async fn get_clip_records(param: QueryParam) -> Result<Vec<ClipRecordDTO>, String> {
    let offset = (param.page - 1) * param.size;
    let rb: &RBatis = CONTEXT.get::<RBatis>();
    // 执行数据库查询逻辑
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
            log::error!("查询粘贴记录失败: {:?}", e);
            return Err("查询粘贴记录失败".to_string());
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
                return ClipRecordDTO {
                    id: item.id.clone(),
                    r#type: item.r#type.clone(),
                    content,
                    os_type: item.os_type.clone(),
                    created: item.created,
                    pinned_flag: item.pinned_flag,
                    file_info: get_file_info_with_paths(content_str, local_paths),
                    image_info: None,
                    sync_flag: item.sync_flag,
                    cloud_source: item.cloud_source,
                    content_truncated: false, // 文件类型不截断
                    original_content_length: None,
                };
            } else if item.r#type == ClipType::Image.to_string() {
                // 对于图片类型，不转换为base64，而是返回元数据
                let image_path = item.content.as_str().unwrap_or_default();
                let image_info = get_image_info(image_path);
                return ClipRecordDTO {
                    id: item.id.clone(),
                    r#type: item.r#type.clone(),
                    content: image_path.to_string(), // 只返回路径
                    os_type: item.os_type.clone(),
                    created: item.created,
                    pinned_flag: item.pinned_flag,
                    file_info: vec![],
                    image_info,
                    sync_flag: item.sync_flag,
                    cloud_source: item.cloud_source,
                    content_truncated: false, // 图片类型不截断
                    original_content_length: None,
                };
            } else {
                // 处理文本类型，如果内容过大则截断
                let processed_content =
                    ContentProcessor::process_by_clip_type(&item.r#type, item.content.clone());
                let (truncated_content, is_truncated, original_length) =
                    truncate_large_text(&processed_content);

                return ClipRecordDTO {
                    id: item.id.clone(),
                    r#type: item.r#type.clone(),
                    content: truncated_content,
                    os_type: item.os_type.clone(),
                    created: item.created,
                    pinned_flag: item.pinned_flag,
                    file_info: vec![],
                    image_info: None,
                    sync_flag: item.sync_flag,
                    cloud_source: item.cloud_source,
                    content_truncated: is_truncated,
                    original_content_length: original_length,
                };
            }
        })
        .collect())
}

/// 使用content（显示名称）和local_file_path（实际路径）获取文件信息
pub fn get_file_info_with_paths(content_names: String, local_paths: String) -> Vec<FileInfo> {
    let display_names = content_names.split(":::").collect::<Vec<&str>>();
    let actual_paths = local_paths.split(":::").collect::<Vec<&str>>();

    log::debug!(
        "正在处理文件信息: 显示名称={:?}, 实际路径={:?}",
        display_names,
        actual_paths
    );

    // 确保显示名称和实际路径数量匹配
    let min_len = display_names.len().min(actual_paths.len());

    (0..min_len)
        .filter_map(|i| {
            let display_name = display_names[i].trim();
            let actual_path = actual_paths[i].trim();

            if display_name.is_empty() || actual_path.is_empty() {
                log::debug!(
                    "跳过空路径或空名称: display={}, path={}",
                    display_name,
                    actual_path
                );
                return None;
            }

            let path_buf = Path::new(actual_path);

            // 从显示名称获取文件扩展名
            let file_type = Path::new(display_name)
                .extension()
                .and_then(|ext| ext.to_str())
                .unwrap_or("未知")
                .to_lowercase();

            if !path_buf.exists() {
                log::warn!(
                    "文件不存在，但仍显示基本信息: display={}, path={}",
                    display_name,
                    actual_path
                );
                // 文件不存在时仍然显示基本信息，方便用户了解原始文件
                return Some(FileInfo {
                    path: display_name.to_string(), // 使用显示名称而不是实际路径
                    size: -1, // 使用-1表示文件不存在，前端可以据此显示特殊状态
                    r#type: file_type,
                });
            }

            // 获取文件元数据
            let metadata = match fs::metadata(path_buf) {
                Ok(meta) => meta,
                Err(e) => {
                    log::warn!(
                        "读取文件元数据失败，但仍显示基本信息: display={}, path={}, 错误: {}",
                        display_name,
                        actual_path,
                        e
                    );
                    // 读取元数据失败时仍然显示基本信息
                    return Some(FileInfo {
                        path: display_name.to_string(),
                        size: -2, // 使用-2表示文件存在但无法读取元数据
                        r#type: file_type,
                    });
                }
            };

            // 获取文件大小，处理大文件的情况
            let size = if metadata.len() > i32::MAX as u64 {
                log::warn!(
                    "文件大小超过i32范围: {} 字节，文件: {}",
                    metadata.len(),
                    display_name
                );
                i32::MAX // 对于超大文件，使用i32最大值
            } else {
                metadata.len() as i32
            };

            Some(FileInfo {
                path: display_name.to_string(), // 返回显示名称
                size,
                r#type: file_type,
            })
        })
        .collect()
}

// 获取图片元数据信息
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
    let size = metadata.len();

    // 可以考虑使用image crate获取图片尺寸，但为了性能考虑暂时不获取
    // let dimensions = image::image_dimensions(&abs_path).ok();

    Some(ImageInfo {
        path: relative_path.to_string(),
        size,
        width: None,  // dimensions.map(|(w, _)| w),
        height: None, // dimensions.map(|(_, h)| h),
    })
}

// 按需获取图片base64数据
#[tauri::command]
pub async fn get_image_base64(param: GetImageParam) -> Result<ImageBase64Response, String> {
    let rb: &RBatis = CONTEXT.get::<RBatis>();

    // 从数据库获取记录
    let records = ClipRecord::select_by_id(rb, &param.record_id)
        .await
        .map_err(|e| format!("查询记录失败: {}", e))?;

    let record = records.first().ok_or("记录不存在")?;

    // 验证是否为图片类型
    if record.r#type != ClipType::Image.to_string() {
        return Err("记录类型不是图片".to_string());
    }

    // 获取图片路径
    let image_path = record.content.as_str().ok_or("图片路径无效")?;

    // 转换为base64
    let base64_data = ContentProcessor::process_image_content(image_path).ok_or("图片转换失败")?;

    Ok(ImageBase64Response {
        id: param.record_id,
        base64_data,
    })
}

/// 截断大文本，返回 (截断后内容, 是否被截断, 原始长度)
fn truncate_large_text(content: &str) -> (String, bool, Option<usize>) {
    const MAX_PREVIEW_SIZE: usize = 128 * 1024; // 128KB

    if content.len() <= MAX_PREVIEW_SIZE {
        (content.to_string(), false, None)
    } else {
        // 简单按字节截断，但确保不会截断到 UTF-8 字符中间
        let mut end_pos = MAX_PREVIEW_SIZE;

        // 向前查找安全的截断位置（UTF-8 字符边界）
        while end_pos > 0 && !content.is_char_boundary(end_pos) {
            end_pos -= 1;
        }

        // 如果找不到合适的边界，至少保留一些内容
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

// 获取记录的完整文本内容
#[tauri::command]
pub async fn get_full_text_content(
    param: GetFullContentParam,
) -> Result<FullContentResponse, String> {
    let rb: &RBatis = CONTEXT.get::<RBatis>();

    // 从数据库获取记录
    let records = ClipRecord::select_by_id(rb, &param.record_id)
        .await
        .map_err(|e| format!("查询记录失败: {}", e))?;

    let record = records.first().ok_or("记录不存在")?;

    // 验证是否为文本类型
    if record.r#type != ClipType::Text.to_string() {
        return Err("记录类型不是文本".to_string());
    }

    // 处理完整内容（解密等）
    let full_content =
        ContentProcessor::process_by_clip_type(&record.r#type, record.content.clone());

    Ok(FullContentResponse {
        id: param.record_id,
        content: full_content.clone(),
        content_length: full_content.len(),
    })
}

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
            if item.r#type == "File" {
                let content_str = item.content.as_str().unwrap_or_default().to_string();
                let content =
                    ContentProcessor::process_by_clip_type(&item.r#type, item.content.clone());
                return ClipRecordDTO {
                    id: item.id.clone(),
                    r#type: item.r#type.clone(),
                    content,
                    os_type: item.os_type.clone(),
                    created: item.created,
                    pinned_flag: item.pinned_flag,
                    file_info: get_file_info(content_str),
                    image_info: None,
                    sync_flag: item.sync_flag,
                };
            } else if item.r#type == "Image" {
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
                };
            } else {
                return ClipRecordDTO {
                    id: item.id.clone(),
                    r#type: item.r#type.clone(),
                    content: ContentProcessor::process_by_clip_type(&item.r#type, item.content),
                    os_type: item.os_type.clone(),
                    created: item.created,
                    pinned_flag: item.pinned_flag,
                    file_info: vec![],
                    image_info: None,
                    sync_flag: item.sync_flag,
                };
            }
        })
        .collect())
}

pub fn get_file_info(paths: String) -> Vec<FileInfo> {
    let paths = paths.split(":::").collect::<Vec<&str>>();
    paths
        .iter()
        .filter_map(|path| {
            let path = path.trim();
            if path.is_empty() {
                return None;
            }

            let path_buf = Path::new(path);
            if !path_buf.exists() {
                return None;
            }

            // 获取文件元数据
            let metadata = match fs::metadata(path_buf) {
                Ok(meta) => meta,
                Err(_) => return None,
            };

            // 获取文件扩展名
            let file_type = path_buf
                .extension()
                .and_then(|ext| ext.to_str())
                .unwrap_or("未知")
                .to_lowercase();

            // 获取文件大小
            let size = metadata.len() as i32;

            Some(FileInfo {
                path: path.to_string(),
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
    if record.r#type != "Image" {
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

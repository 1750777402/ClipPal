
use rbatis::RBatis;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

use crate::{
    CONTEXT,
    biz::{
        clip_record::ClipRecord, content_processor::ContentProcessor,
        simple_search_bin::search_ids_by_content,
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
    // 内容
    pub content: String,
    // os类型
    pub os_type: String,
    // 创建时间
    pub created: u64,
    // 是否置顶
    pub pinned_flag: i32,
    // 文件内容属性
    pub file_info: Vec<FileInfo>,
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

#[tauri::command]
pub async fn get_clip_records(param: QueryParam) -> Vec<ClipRecordDTO> {
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
            return vec![];
        }
    };
    if all_data.is_empty() {
        return vec![];
    }

    all_data
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
                };
            }
        })
        .collect()
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

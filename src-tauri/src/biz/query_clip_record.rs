use rbatis::RBatis;
use serde::{Deserialize, Serialize};

use crate::{
    CONTEXT,
    biz::{clip_record::ClipRecord, content_processor::ContentProcessor},
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
    let all_data = match query_result {
        Ok(data) => data,
        Err(e) => {
            eprintln!("查询粘贴记录失败: {:?}", e);
            return vec![];
        }
    };

    if all_data.is_empty() {
        return vec![];
    }

    all_data
        .into_iter()
        .map(|item| ClipRecordDTO {
            id: item.id.clone(),
            r#type: item.r#type.clone(),
            content: ContentProcessor::process_by_clip_type(&item.r#type, item.content),
            os_type: item.os_type.clone(),
        })
        .collect()
}

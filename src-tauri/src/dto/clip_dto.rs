use serde::{Deserialize, Serialize};

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

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ClipRecordIdParam {
    pub record_id: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PinnedClipRecordParam {
    pub record_id: String,
    pub pinned_flag: i32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CopySingleFileParam {
    pub record_id: String,
    pub file_path: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct FullContentResponse {
    pub id: String,
    pub content: String,
    pub content_length: usize,
}

#[derive(Debug, Serialize)]
pub struct ImagePathInfo {
    pub id: String,
    pub file_path: String,
    pub protocol_url: String,
}

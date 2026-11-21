use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
/// 剪贴记录分页查询参数；页码从 1 开始，搜索词为空时查询全部记录。
pub struct QueryParam {
    pub page: i32,
    pub size: i32,
    pub search: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
/// 剪贴记录列表轻量 DTO，只携带首屏展示和交互需要的数据。
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
/// 文件记录中单个文件的展示路径、大小和扩展名信息。
pub struct FileInfo {
    pub path: String,
    pub size: i32,
    pub r#type: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
/// 图片文件基础信息；宽高暂未解析时允许为空。
pub struct ImageInfo {
    pub path: String,
    pub size: u64,
    pub width: Option<u32>,
    pub height: Option<u32>,
}

#[derive(Debug, Serialize, Deserialize)]
/// 获取图片路径的 command 参数。
pub struct GetImageParam {
    pub record_id: String,
}

#[derive(Debug, Serialize, Deserialize)]
/// 获取完整文本内容的 command 参数。
pub struct GetFullContentParam {
    pub record_id: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
/// 仅包含剪贴记录 ID 的通用 command 参数。
pub struct ClipRecordIdParam {
    pub record_id: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
/// 修改记录置顶状态的 command 参数。
pub struct PinnedClipRecordParam {
    pub record_id: String,
    pub pinned_flag: i32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
/// 从多文件记录复制单个文件所需的记录 ID 和显示路径。
pub struct CopySingleFileParam {
    pub record_id: String,
    pub file_path: String,
}

#[derive(Debug, Serialize, Deserialize)]
/// 完整文本响应，包含内容和 UTF-8 字节长度。
pub struct FullContentResponse {
    pub id: String,
    pub content: String,
    pub content_length: usize,
}

#[derive(Debug, Serialize)]
/// 图片本地路径响应；协议地址字段保留给前端资源协议扩展。
pub struct ImagePathInfo {
    pub id: String,
    pub file_path: String,
    pub protocol_url: String,
}

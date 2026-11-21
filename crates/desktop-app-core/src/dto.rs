//! 前后端传输模型；不直接暴露数据库实体、加密配置或资源所有权。
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
/// 剪贴记录分页查询参数；页码从 1 开始，搜索词为空时查询全部记录。
pub struct QueryParam {
    /// 页码，从 1 开始。
    pub page: i32,
    /// 每页条数，必须大于零。
    pub size: i32,
    /// 可选关键词；不传或空字符串时按普通列表查询。
    pub search: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
/// 剪贴记录列表轻量 DTO，只携带首屏展示和交互需要的数据。
pub struct ClipRecordLiteDTO {
    /// 本地主键的十进制字符串，避免 JavaScript 整数精度丢失。
    pub id: String,
    /// 兼容现有卡片组件的 Text / Image / File 展示类型。
    pub r#type: String,
    /// 文本预览、图片资源标识，或文件显示名的 JSON 数组。
    pub content: String,
    /// 最近采集时间，毫秒；字段名保留旧页面协议。
    pub created: u64,
    /// 0 为普通记录，1 为当前置顶记录。
    pub pinned_flag: i32,
    /// 文件/目录的展示信息，文本和图片为空列表。
    pub file_info: Vec<FileInfo>,
    /// 文本预览是否截断；为 true 时需按 ID 获取全文。
    pub content_truncated: bool,
    /// 截断前的 UTF-8 字节数；未截断时为 None。
    pub original_content_length: Option<usize>,
    /// 是否图片记录，供列表决定是否按需加载图片。
    pub has_image: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
/// 文件记录中单个文件的展示路径、大小和扩展名信息。
pub struct FileInfo {
    /// 资源主键；选择单个文件时与 record_id 一起提交。
    pub resource_id: String,
    /// 原始显示名，沿用 path 字段名；不是实际磁盘路径。
    pub path: String,
    /// 当前资源字节数，文件已缺失时为 -1。
    pub size: i64,
    /// 文件扩展名，目录为 directory，无扩展名为 unknown。
    pub r#type: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
/// 图片文件基础信息；宽高暂未解析时允许为空。
pub struct ImageInfo {
    /// 后端校验过的图片本地路径。
    pub path: String,
    /// 图片文件字节数。
    pub size: u64,
    /// 图片宽度，尚未解码时为 None。
    pub width: Option<u32>,
    /// 图片高度，尚未解码时为 None。
    pub height: Option<u32>,
}

#[derive(Debug, Serialize, Deserialize)]
/// 获取图片路径的 command 参数。
pub struct GetImageParam {
    /// 需要读取的图片记录主键，不接受任意文件路径。
    pub record_id: String,
}

#[derive(Debug, Serialize, Deserialize)]
/// 获取完整文本内容的 command 参数。
pub struct GetFullContentParam {
    /// 需要展开全文的文本记录主键。
    pub record_id: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
/// 仅包含剪贴记录 ID 的通用 command 参数。
pub struct ClipRecordIdParam {
    /// 要操作的本地记录主键。
    pub record_id: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
/// 修改记录置顶状态的 command 参数。
pub struct PinnedClipRecordParam {
    /// 要切换置顶状态的本地记录主键。
    pub record_id: String,
    /// 只允许 0/1，后端再次校验。
    pub pinned_flag: i32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
/// 从多文件记录复制单个文件所需的记录 ID 和资源 ID。
pub struct CopySingleFileParam {
    /// 所属记录的主键。
    pub record_id: String,
    /// 所选资源的主键；不能使用同名文件的显示名代替。
    pub resource_id: String,
}

#[derive(Debug, Serialize, Deserialize)]
/// 完整文本响应，包含内容和 UTF-8 字节长度。
pub struct FullContentResponse {
    /// 对应文本记录的本地主键。
    pub id: String,
    /// 解密后的完整原文，保留空白和换行。
    pub content: String,
    /// UTF-8 字节数，不是字符个数。
    pub content_length: usize,
}

#[derive(Debug, Serialize)]
/// 图片本地路径响应；协议地址字段保留给前端资源协议扩展。
pub struct ImagePathInfo {
    /// 对应图片记录的本地主键。
    pub id: String,
    /// 实际存在且通过归属校验的本地文件路径。
    pub file_path: String,
    /// 当前为空字符串，保留给资源协议扩展。
    pub protocol_url: String,
}

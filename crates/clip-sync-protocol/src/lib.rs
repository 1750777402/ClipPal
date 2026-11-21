//! 当前线上协议的兼容契约，不包含 HTTP、Tauri、密钥或账号逻辑。
//! v2 的 SSE/Outbox 协议将在明确版本之后增加，当前字段保持不变。
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// 旧服务批量同步响应，只承载线协议数据，不代表本地数据库实体。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CloudSyncResponse {
    /// 服务端返回的记录清单，未返回清单时为 None。
    pub clips: Option<Vec<ClipRecordParam>>,
}
/// 旧服务记录契约。字段命名保持兼容，新单机实体不再依赖这些同步字段。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClipRecordParam {
    /// 旧兼容输入中的本地标识，不参与网络序列化。
    #[serde(skip)]
    pub id: Option<String>,
    // 类型
    pub r#type: Option<String>,
    // 内容
    pub content: Value,
    // 内容md5值
    pub md5_str: Option<String>,
    // 时间戳
    pub created: Option<u64>,
    // os类型
    pub os_type: Option<String>,
    // 排序字段
    pub sort: Option<i32>,
    // 是否置顶
    pub pinned_flag: Option<i32>,
    // 是否已同步云端  0:未同步，1:已同步
    pub sync_flag: Option<i32>,
    // 同步时间
    pub sync_time: Option<u64>,
    // 设备标识
    pub device_id: Option<String>,
    // 云同步版本号
    pub version: Option<i32>,
    // 是否逻辑删除
    pub del_flag: Option<i32>,
    // 本地文件地址
    #[serde(skip)]
    pub local_file_path: Option<String>,
}
/// 旧服务的批量同步请求；保留原字段形式供兼容测试和后续替换参考。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CloudSyncRequest {
    /// 待交换的旧协议记录。
    pub clips: Vec<ClipRecordParam>,
    /// 本次请求的毫秒时间戳。
    pub timestamp: u64,
    /// 上次同步游标时间，不是本地记录自增 ID。
    pub last_sync_time: u64,
    /// 发送设备的旧协议标识。
    pub device_id: String,
}
/// 旧单条同步响应中的服务时间。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SingleCloudSyncResponse {
    /// 服务返回的毫秒时间戳。
    pub timestamp: u64,
}
/// 旧单条同步请求，用操作类型与一条记录表达变更。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SingleCloudSyncParam {
    /// 旧服务约定的操作类型数值，当前仅保持协议原样。
    pub r#type: i32,
    /// 本次操作对应的旧协议记录。
    pub clip: ClipRecordParam,
}
/// 旧附件同步身份参数，使用服务端原有的 MD5 字段名。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FileCloudSyncParam {
    /// 旧服务附件摘要；与新版本地 content_key 分开维护。
    pub md5_str: String,
    /// 旧服务约定的记录类型。
    pub r#type: String,
}
/// 旧附件下载响应，描述临时下载位置及服务端文件名。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DownloadCloudFileResponse {
    /// 服务端提供的下载地址。
    pub url: String,
    /// 旧服务附件摘要。
    pub md5_str: String,
    /// 旧服务记录类型。
    pub r#type: String,
    /// 下载后展示给用户的文件名。
    pub file_name: String,
}
/// 根据旧摘要和类型请求附件下载信息。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DownloadCloudFileParam {
    /// 旧服务用于定位附件的摘要。
    pub md5_str: String,
    /// 请求的旧协议记录类型。
    pub r#type: String,
}

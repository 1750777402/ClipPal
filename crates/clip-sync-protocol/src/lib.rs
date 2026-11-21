//! 当前线上协议的兼容契约，不包含 HTTP、Tauri、密钥或账号逻辑。
//! v2 的 SSE/Outbox 协议将在明确版本之后增加，当前字段保持不变。
use clip_core::domain::clip::ClipRecord;
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CloudSyncResponse {
    pub clips: Option<Vec<ClipRecordParam>>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClipRecordParam {
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
impl ClipRecordParam {
    pub fn to_clip_record(&self) -> ClipRecord {
        ClipRecord {
            id: self.id.clone().unwrap_or_default(),
            r#type: self.r#type.clone().unwrap_or_default(),
            content: self.content.clone(),
            md5_str: self.md5_str.clone().unwrap_or_default(),
            local_file_path: None,
            created: self.created.unwrap_or(0),
            os_type: self.os_type.clone().unwrap_or_default(),
            sort: self.sort.unwrap_or(0),
            pinned_flag: self.pinned_flag.unwrap_or(0),
            sync_flag: self.sync_flag,
            sync_time: self.sync_time,
            device_id: self.device_id.clone(),
            version: self.version,
            del_flag: self.del_flag,
            cloud_source: Some(0),
            skip_type: None,
        }
    }
}
impl From<ClipRecord> for ClipRecordParam {
    fn from(record: ClipRecord) -> Self {
        ClipRecordParam {
            id: record.id.into(),
            r#type: Some(record.r#type),
            content: record.content,
            md5_str: Some(record.md5_str),
            created: Some(record.created),
            os_type: Some(record.os_type),
            sort: Some(record.sort),
            pinned_flag: Some(record.pinned_flag),
            sync_flag: record.sync_flag.into(),
            sync_time: record.sync_time,
            device_id: record.device_id,
            version: record.version.into(),
            del_flag: record.del_flag.into(),
            local_file_path: record.local_file_path,
        }
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CloudSyncRequest {
    pub clips: Vec<ClipRecordParam>,
    pub timestamp: u64,
    pub last_sync_time: u64,
    pub device_id: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SingleCloudSyncResponse {
    pub timestamp: u64,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SingleCloudSyncParam {
    pub r#type: i32,
    pub clip: ClipRecordParam,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FileCloudSyncParam {
    pub md5_str: String,
    pub r#type: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DownloadCloudFileResponse {
    pub url: String,
    pub md5_str: String,
    pub r#type: String,
    pub file_name: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DownloadCloudFileParam {
    pub md5_str: String,
    pub r#type: String,
}

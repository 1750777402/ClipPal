use crate::{
    api::{api_get, api_post},
    biz::clip_record::ClipRecord,
    utils::http_client::HttpError,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;

// ----------------------------------------- 云同步api ------------------------------------------------------

// 云同步响应结构体
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

// 云同步请求结构体
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CloudSyncRequest {
    pub clips: Vec<ClipRecordParam>,
    pub timestamp: u64,
    pub last_sync_time: u64,
    pub device_id: String,
}

// 云同步api
pub async fn sync_clipboard(
    request: &CloudSyncRequest,
) -> Result<Option<CloudSyncResponse>, HttpError> {
    api_post("cliPal-sync/sync/complete", Some(request)).await
}

// -------------------------------------------获取服务器时间--------------------------------------------------------------

pub async fn sync_server_time() -> Result<Option<u64>, HttpError> {
    api_get("cliPal-sync/public/now").await
}

// -------------------------------------------处理单个记录的新增或者删除--------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SingleCloudSyncResponse {
    pub timestamp: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SingleCloudSyncParam {
    pub r#type: i32,
    pub clip: ClipRecordParam,
}

pub async fn sync_single_clip_record(
    record: &SingleCloudSyncParam,
) -> Result<Option<SingleCloudSyncResponse>, HttpError> {
    api_post("cliPal-sync/sync/single", Some(record)).await
}

// ------------------------------------------获取上传链接--------------------------------------------------------
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FileCloudSyncParam {
    pub md5_str: String,
    pub r#type: String,
}

pub async fn get_upload_file_url(
    record: &FileCloudSyncParam,
) -> Result<Option<DownloadCloudFileResponse>, HttpError> {
    // 准备form-data参数
    let mut form_data = HashMap::new();
    form_data.insert("md5Str".to_string(), record.md5_str.clone());
    form_data.insert("type".to_string(), record.r#type.clone());

    api_post("cliPal-sync/sync/getUploadUrl", Some(record)).await
}

// ------------------------------------------通知服务端上传完成--------------------------------------------------------
pub async fn sync_upload_success(record: &FileCloudSyncParam) -> Result<Option<bool>, HttpError> {
    // 准备form-data参数
    let mut form_data = HashMap::new();
    form_data.insert("md5Str".to_string(), record.md5_str.clone());
    form_data.insert("type".to_string(), record.r#type.clone());

    api_post("cliPal-sync/sync/uploadSuccess", Some(record)).await
}

// ----------------------------------------------获取下载链接------------------------------------------------------------------------

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

pub async fn get_dowload_url(
    record: &DownloadCloudFileParam,
) -> Result<Option<DownloadCloudFileResponse>, HttpError> {
    // 准备form-data参数
    let mut form_data = HashMap::new();
    form_data.insert("md5Str".to_string(), record.md5_str.clone());
    form_data.insert("type".to_string(), record.r#type.clone());

    api_post("cliPal-sync/sync/getDownloadUrl", Some(record)).await
}

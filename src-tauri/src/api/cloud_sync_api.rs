use crate::{
    api::{api_get, api_post},
    biz::clip_record::ClipRecord,
    utils::http_client::HttpError,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;

// ----------------------------------------- 云同步api ------------------------------------------------------

// 云同步响应结构体
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CloudSyncResponse {
    pub timestamp: u64,
    pub clips: Option<Vec<ClipRecordParam>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClipRecordParam {
    pub id: Option<String>,
    // 类型
    pub r#type: Option<String>,
    // 内容
    pub content: Value,
    // 内容md5值
    pub md5_str: Option<String>,
    // 时间戳
    pub created: Option<u64>,
    // 用户id
    pub user_id: Option<i32>,
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
}

impl ClipRecordParam {
    pub fn to_clip_record(&self) -> ClipRecord {
        ClipRecord {
            id: self.id.clone().unwrap_or_default(),
            r#type: self.r#type.clone().unwrap_or_default(),
            content: self.content.clone(),
            md5_str: self.md5_str.clone().unwrap_or_default(),
            created: self.created.unwrap_or(0),
            user_id: self.user_id.unwrap_or(0),
            os_type: self.os_type.clone().unwrap_or_default(),
            sort: self.sort.unwrap_or(0),
            pinned_flag: self.pinned_flag.unwrap_or(0),
            sync_flag: self.sync_flag,
            sync_time: self.sync_time,
            device_id: self.device_id.clone(),
            version: self.version,
            del_flag: self.del_flag,
        }
    }
}

// 云同步请求结构体
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CloudSyncRequest {
    pub clips: Vec<ClipRecord>,
    pub timestamp: u64,
    pub last_sync_time: u64,
}

// 云同步api
pub async fn sync_clipboard(
    request: &CloudSyncRequest,
) -> Result<Option<CloudSyncResponse>, HttpError> {
    api_post("POST", "cliPal-sync/sync/complete", Some(request)).await
}

// -------------------------------------------获取服务器时间--------------------------------------------------------------

pub async fn sync_server_time() -> Result<Option<u64>, HttpError> {
    api_get("cliPal-sync/public/now").await
}

// -------------------------------------------处理单个记录的新增或者删除--------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SingleCloudSyncResponse {
    pub timestamp: u64,
    pub clips: Option<Vec<ClipRecordParam>>,
}

pub async fn sync_single_clip_record(
    record: &ClipRecord,
) -> Result<Option<SingleCloudSyncResponse>, HttpError> {
    api_post("POST", "cliPal-sync/sync/single", Some(record)).await
}

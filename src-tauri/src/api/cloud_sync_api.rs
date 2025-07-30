use crate::{
    api::{api_get, api_post},
    biz::clip_record::ClipRecord,
    utils::http_client::HttpError,
};
use serde::{Deserialize, Serialize};

// ----------------------------------------- 云同步api ------------------------------------------------------

// 云同步响应结构体
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CloudSyncResponse {
    pub timestamp: u64,
    pub clips: Option<Vec<ClipRecord>>,
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

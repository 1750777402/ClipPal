use crate::utils::config::get_cloud_sync_domain;
use crate::utils::http_client::{ApiResponse, HttpClient, HttpError};
use crate::biz::clip_record::ClipRecord;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;


/// 获取 API 域名
fn get_api_domain() -> Result<String, HttpError> {
    get_cloud_sync_domain()
        .map(|s| s.to_string())
        .map_err(|e| HttpError::RequestFailed(format!("获取云同步请求域名失败: {}", e)))
}

/// 获取通用请求头
fn get_common_headers(token: &str) -> HashMap<String, String> {
    let mut headers = HashMap::new();
    headers.insert("Authorization".to_string(), format!("Bearer {}", token));
    headers.insert("Content-Type".to_string(), "application/json".to_string());
    headers
}

/// 通用API请求方法，返回 ApiResponse<T> 的 data 字段
pub async fn api_post<P, T>(
    mothod: &str,
    path: &str,
    payload: &P,
) -> Result<Option<T>, HttpError>
where
    P: serde::Serialize + Sized,
    T: for<'de> serde::Deserialize<'de>,
{
    let api_domain = get_api_domain()?;
    let url = format!("{}/{}", api_domain, path.trim_start_matches('/'));
    let token = "";
    let headers = get_common_headers(token);
    let client = HttpClient::new();
    let resp: ApiResponse<T> = client
        .request_with_headers(mothod, &url, Some(payload), Some(headers))
        .await?;
    if resp.code == 200 {
        Ok(resp.data)
    } else {
        Err(HttpError::RequestFailed(format!("API请求失败: {}", resp.message)))
    }
}

// ----------------------------------------- 云同步api ------------------------------------------------------

// 云同步响应结构体
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CloudSyncResponse {
    pub success: bool,
    pub message: String,
    pub data: Option<Vec<ClipRecord>>,
}

// 云同步请求结构体
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CloudSyncRequest {
    pub clips: Vec<ClipRecord>,
    pub timestamp: u64,
}

// 云同步api
pub async fn sync_clipboard(
    request: &CloudSyncRequest,
) -> Result<Option<CloudSyncResponse>, HttpError> {
    api_post("POST", "cliPal-sync/sync", request).await
}

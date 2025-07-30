use crate::utils::config::get_cloud_sync_domain;
use crate::utils::http_client::{ApiResponse, HttpClient, HttpError};
use crate::utils::secure_store::SECURE_STORE;
use std::collections::HashMap;

pub mod cloud_sync_api;

/// 获取 API 域名
fn get_api_domain() -> Result<String, HttpError> {
    get_cloud_sync_domain()
        .map(|s| s.to_string())
        .map_err(|e| HttpError::RequestFailed(format!("获取云同步请求域名失败: {}", e)))
}

/// 安全获取JWT token，获取不到时返回空字符串
fn get_jwt_token() -> String {
    SECURE_STORE
        .read()
        .ok()
        .and_then(|store| store.data().jwt_token.clone())
        .unwrap_or_default()
}

/// 获取通用请求头
fn get_common_headers(token: &str) -> HashMap<String, String> {
    let mut headers = HashMap::new();
    headers.insert("Authorization".to_string(), format!("Bearer {}", token));
    headers.insert("Content-Type".to_string(), "application/json".to_string());
    headers
}

/// 通用POST API请求方法，返回 ApiResponse<T> 的 data 字段
pub async fn api_post<P, T>(
    method: &str,
    path: &str,
    payload: Option<&P>,
) -> Result<Option<T>, HttpError>
where
    P: serde::Serialize + Sized,
    T: for<'de> serde::Deserialize<'de>,
{
    let api_domain = get_api_domain()?;
    let url = format!("{}/{}", api_domain, path.trim_start_matches('/'));
    // let token = get_jwt_token();
    let token = "eyJhbGciOiJIUzUxMiJ9.eyJyb2xlIjoiVVNFUiIsInR5cGUiOiJhY2Nlc3MiLCJ1c2VySWQiOjEsInN1YiI6ImFkbWluIiwiaXNzIjoiY2xpcC1wYWwtY2xvdWQiLCJpYXQiOjE3NTM4Njc5NzIsImV4cCI6MTc1Mzk1NDM3Mn0.Qm3yZ20CVhfTr-52fz550V8uoOvnFbLhkdwblQTsH5SSBAIZU_4VYL-VYWjBh-oLeIyydC70f_kr2mi43PyXxA";
    let headers = get_common_headers(&token);
    let client = HttpClient::new();
    let resp: ApiResponse<T> = client
        .request_with_headers(method, &url, payload, Some(headers))
        .await?;
    if resp.code == 200 {
        Ok(resp.data)
    } else {
        Err(HttpError::RequestFailed(format!(
            "API请求失败: {}",
            resp.message
        )))
    }
}

/// 获取通用请求头（不包含Content-Type，用于GET请求）
fn get_common_headers_without_content_type(token: &str) -> HashMap<String, String> {
    let mut headers = HashMap::new();
    headers.insert("Authorization".to_string(), format!("Bearer {}", token));
    headers
}

/// 通用GET API请求方法，返回 ApiResponse<T> 的 data 字段
pub async fn api_get<T>(path: &str) -> Result<Option<T>, HttpError>
where
    T: for<'de> serde::Deserialize<'de>,
{
    let api_domain = get_api_domain()?;
    let url = format!("{}/{}", api_domain, path.trim_start_matches('/'));
    let token = get_jwt_token();
    let headers = get_common_headers_without_content_type(&token);
    let client = HttpClient::new();
    let resp: ApiResponse<T> = client
        .request_with_headers("GET", &url, None::<&()>, Some(headers))
        .await?;
    if resp.code == 200 {
        Ok(resp.data)
    } else {
        Err(HttpError::RequestFailed(format!(
            "API请求失败: {}",
            resp.message
        )))
    }
}

use crate::utils::config::get_cloud_sync_domain;
use crate::utils::http_client::{ApiResponse, HttpClient, HttpError};
use crate::utils::secure_store::SECURE_STORE;
use std::collections::HashMap;
use std::path::Path;

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
pub async fn api_post<P, T>(path: &str, payload: Option<&P>) -> Result<Option<T>, HttpError>
where
    P: serde::Serialize + Sized,
    T: for<'de> serde::Deserialize<'de>,
{
    let api_domain = get_api_domain()?;
    let url = format!("{}/{}", api_domain, path.trim_start_matches('/'));
    // let token = get_jwt_token();
    let token = "eyJhbGciOiJIUzUxMiJ9.eyJyb2xlIjoiVVNFUiIsInR5cGUiOiJhY2Nlc3MiLCJ1c2VySWQiOjEsInN1YiI6ImFkbWluIiwiaXNzIjoiY2xpcC1wYWwtY2xvdWQiLCJpYXQiOjE3NTU2NTYwNTYsImV4cCI6MTc1NTc0MjQ1Nn0.Kmi6HIm6e-adBKcAvdjJU3pf25D1TmXVU6ct0f4Lq9FFQgZFURt5iWU-nMNlROOeXhFelxJQznDh5jpWibIIvA";
    let headers = get_common_headers(&token);
    let client = HttpClient::new();
    let resp: ApiResponse<T> = client
        .request_with_headers("POST", &url, payload, Some(headers))
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

/// 通用文件上传API请求方法，返回 ApiResponse<T> 的 data 字段
pub async fn api_post_file<T>(
    path: &str,
    file_path: &Path,
    form_data: &HashMap<String, String>,
) -> Result<Option<T>, HttpError>
where
    T: for<'de> serde::Deserialize<'de>,
{
    let api_domain = get_api_domain()?;
    let url = format!("{}/{}", api_domain, path.trim_start_matches('/'));
    // let token = get_jwt_token();
    let token = "eyJhbGciOiJIUzUxMiJ9.eyJyb2xlIjoiVVNFUiIsInR5cGUiOiJhY2Nlc3MiLCJ1c2VySWQiOjEsInN1YiI6ImFkbWluIiwiaXNzIjoiY2xpcC1wYWwtY2xvdWQiLCJpYXQiOjE3NTU2NTYwNTYsImV4cCI6MTc1NTc0MjQ1Nn0.Kmi6HIm6e-adBKcAvdjJU3pf25D1TmXVU6ct0f4Lq9FFQgZFURt5iWU-nMNlROOeXhFelxJQznDh5jpWibIIvA";

    // 为文件上传准备请求头（不包含Content-Type，让reqwest自动处理multipart）
    let mut headers = HashMap::new();
    headers.insert("Authorization".to_string(), format!("Bearer {}", token));

    let client = HttpClient::new().headers(headers);
    let resp: ApiResponse<T> = client.post_multipart(&url, file_path, form_data).await?;
    if resp.code == 200 {
        Ok(resp.data)
    } else {
        Err(HttpError::RequestFailed(format!(
            "API文件上传请求失败: {}",
            resp.message
        )))
    }
}

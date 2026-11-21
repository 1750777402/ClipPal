//! 桌面 HTTP/认证适配器；同步端点实现来自共享客户端。
use async_trait::async_trait;
use serde::{de::DeserializeOwned, Serialize};
use clip_sync_client::transport::{JsonTransport, SyncClient};
use {
    crate::app_context::app_context,
    crate::network::http_client::ApiResponse,
    crate::network::http_client::HttpError,
};
use std::collections::HashMap;
use clip_sync_protocol::{CloudSyncRequest, CloudSyncResponse, SingleCloudSyncParam, SingleCloudSyncResponse, FileCloudSyncParam, DownloadCloudFileResponse, DownloadCloudFileParam};

struct DesktopTransport;
#[async_trait]
impl JsonTransport for DesktopTransport {
    type Error = HttpError;
    async fn get<T: DeserializeOwned + Send>(&self, path: &str) -> Result<Option<T>, HttpError> {
        execute_request::<(), T>("GET", path, None, 30).await
    }
    async fn post<P: Serialize + Sync, T: DeserializeOwned + Send>(&self, path: &str, payload: &P, timeout_secs: u64) -> Result<Option<T>, HttpError> {
        execute_request("POST", path, Some(payload), timeout_secs).await
    }
}

/// 认证只来自注入的 provider；自建部署 401 不触发账号刷新或退出登录。
async fn execute_request<P: Serialize + Sync, T: DeserializeOwned + Send>(
    method: &str, path: &str, payload: Option<&P>, timeout_secs: u64,
) -> Result<Option<T>, HttpError> {
    let context = app_context().map_err(|error| HttpError::RequestFailed(error.to_string()))?;
    use clip_sync_client::auth::{authenticated_request, SyncResponse, SyncRequestError};
    authenticated_request(context.sync().auth(), |connection| async move {
        let url = connection.request_url(path).map_err(HttpError::InvalidUrl)?;
        let mut headers: HashMap<String, String> = HashMap::new();
        headers.insert("Authorization".into(), connection.authorization());
        if method == "POST" { headers.insert("Content-Type".into(), "application/json".into()); }
        let client = tauri_plugin_http::reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(timeout_secs))
            .user_agent("ClipPal/1.0")
            .build().map_err(|error| HttpError::RequestFailed(error.to_string()))?;
        let mut request = if method == "GET" { client.get(&url) } else { client.post(&url) };
        for (name, value) in headers { request = request.header(name, value); }
        if let Some(payload) = payload { request = request.json(payload); }
        let response = request.send().await.map_err(|error| HttpError::NetworkError(error.to_string()))?;
        let status = response.status();
        // 同时支持真实 HTTP 401 和既有响应信封中的业务 401。
        if !status.is_success() {
            return Ok::<_, HttpError>(SyncResponse::<T> {
                code: status.as_u16() as i32,
                message: format!("同步服务请求失败（HTTP {}）", status.as_u16()),
                data: None,
            });
        }
        let response = response.json::<ApiResponse<T>>().await
            .map_err(|error| HttpError::DeserializationFailed(error.to_string()))?;
        Ok(SyncResponse { code: response.code, message: response.message, data: response.data })
    }).await.map_err(|error| match error {
        SyncRequestError::Auth(message) => HttpError::RequestFailed(message),
        SyncRequestError::Transport(error) => error,
        SyncRequestError::Rejected { code, message } => {
            log::warn!("同步请求失败 [{}] code={}", path, code);
            HttpError::RequestFailed(if code == 401 { "同步认证失败，请检查认证配置".into() } else { message })
        }
    })
}

// 兼容现有同步任务的函数调用方式，地址和认证仍由桌面 HTTP 适配器处理。
pub async fn sync_clipboard(request: &CloudSyncRequest) -> Result<Option<CloudSyncResponse>, HttpError> {
    SyncClient::new(DesktopTransport).sync_clipboard(request).await
}
pub async fn sync_server_time() -> Result<Option<u64>, HttpError> {
    SyncClient::new(DesktopTransport).server_time().await
}
pub async fn sync_single_clip_record(request: &SingleCloudSyncParam) -> Result<Option<SingleCloudSyncResponse>, HttpError> {
    SyncClient::new(DesktopTransport).sync_record(request).await
}
pub async fn get_upload_file_url(request: &FileCloudSyncParam) -> Result<Option<DownloadCloudFileResponse>, HttpError> {
    SyncClient::new(DesktopTransport).upload_url(request).await
}
pub async fn sync_upload_success(request: &FileCloudSyncParam) -> Result<Option<bool>, HttpError> {
    SyncClient::new(DesktopTransport).upload_success(request).await
}
pub async fn get_dowload_url(request: &DownloadCloudFileParam) -> Result<Option<DownloadCloudFileResponse>, HttpError> {
    SyncClient::new(DesktopTransport).download_url(request).await
}

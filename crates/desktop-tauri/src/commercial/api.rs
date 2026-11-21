use crate::desktop::config::get_cloud_sync_domain;
use {
    crate::network::http_client::ApiResponse,
    crate::network::http_client::HttpClient,
    crate::network::http_client::HttpError,
};
use {
    crate::commercial::token_manager::get_valid_access_token,
    crate::commercial::token_manager::refresh_access_token,
};
use std::collections::HashMap;

/// 获取 API 域名
fn get_api_domain() -> Result<String, HttpError> {
    get_cloud_sync_domain()
        .map(|s| s.to_string())
        .map_err(|e| HttpError::RequestFailed(format!("获取云同步请求域名失败: {}", e)))
}

/// 执行API请求的内部实现
async fn execute_api_request<P, T>(
    method: &str,
    path: &str,
    payload: Option<&P>,
    retry_on_401: bool,
) -> Result<Option<T>, HttpError>
where
    P: serde::Serialize + Sized,
    T: for<'de> serde::Deserialize<'de>,
{
    execute_api_request_with_timeout(method, path, payload, retry_on_401, 30).await
}

/// 带超时的执行API请求的内部实现
async fn execute_api_request_with_timeout<P, T>(
    method: &str,
    path: &str,
    payload: Option<&P>,
    retry_on_401: bool,
    timeout_secs: u64,
) -> Result<Option<T>, HttpError>
where
    P: serde::Serialize + Sized,
    T: for<'de> serde::Deserialize<'de>,
{
    let api_domain = get_api_domain()?;
    let url = format!("{}/{}", api_domain, path.trim_start_matches('/'));

    // 获取访问令牌
    let token = match get_valid_access_token().await {
        Ok(Some(token)) => token,
        Ok(None) => {
            return Err(HttpError::RequestFailed(
                "用户未登录或令牌已过期".to_string(),
            ));
        }
        Err(e) => {
            return Err(HttpError::RequestFailed(format!("获取访问令牌失败: {}", e)));
        }
    };

    let headers = get_common_headers(&token);
    let client = HttpClient::new().timeout(timeout_secs);

    let resp: ApiResponse<T> = match method {
        "GET" => {
            let headers = get_common_headers_without_content_type(&token);
            client
                .request_with_headers("GET", &url, None::<&()>, Some(headers))
                .await?
        }
        "POST" => {
            client
                .request_with_headers("POST", &url, payload, Some(headers))
                .await?
        }
        _ => {
            return Err(HttpError::RequestFailed("不支持的HTTP方法".to_string()));
        }
    };

    match resp.code {
        200 => Ok(resp.data),
        401 if retry_on_401 => {
            // 令牌可能过期，尝试刷新
            log::info!("API返回401，尝试刷新令牌后重试");
            match refresh_access_token().await {
                Ok(Some(_new_token)) => {
                    // 使用新令牌重试请求（不再重试401）
                    Box::pin(execute_api_request_with_timeout(
                        method,
                        path,
                        payload,
                        false,
                        timeout_secs,
                    ))
                    .await
                }
                Ok(None) | Err(_) => Err(HttpError::RequestFailed(
                    "用户认证已过期，需要重新登录".to_string(),
                )),
            }
        }
        _ => {
            // 对于标准的ApiResponse，直接使用服务器返回的message，不再添加额外包装
            let error_msg = resp.message.trim().to_string();
            log::warn!(
                "API请求失败 [{}] 状态码:{} -> {}",
                path,
                resp.code,
                error_msg
            );
            Err(HttpError::RequestFailed(error_msg))
        }
    }
}

/// 获取通用请求头
fn get_common_headers(token: &str) -> HashMap<String, String> {
    let mut headers = HashMap::new();
    headers.insert("Authorization".to_string(), format!("Bearer {}", token));
    headers.insert("Content-Type".to_string(), "application/json".to_string());
    headers
}

/// 通用POST API请求方法（需要认证）
pub async fn api_post<P, T>(path: &str, payload: Option<&P>) -> Result<Option<T>, HttpError>
where
    P: serde::Serialize + Sized,
    T: for<'de> serde::Deserialize<'de>,
{
    execute_api_request("POST", path, payload, true).await
}

/// 公共API POST请求方法（不需要认证，如登录、注册等）
pub async fn api_post_public<P, T>(path: &str, payload: Option<&P>) -> Result<Option<T>, HttpError>
where
    P: serde::Serialize + Sized,
    T: for<'de> serde::Deserialize<'de>,
{
    let api_domain = get_api_domain()?;
    let url = format!("{}/{}", api_domain, path.trim_start_matches('/'));
    let headers = get_public_headers();
    let client = HttpClient::new();
    let resp: ApiResponse<T> = client
        .request_with_headers("POST", &url, payload, Some(headers))
        .await?;
    if resp.code == 200 {
        Ok(resp.data)
    } else {
        // 对于公共API的标准ApiResponse，也直接使用服务器返回的message
        let error_msg = resp.message.trim().to_string();
        log::warn!(
            "公共API请求失败 [{}] 状态码:{} -> {}",
            path,
            resp.code,
            error_msg
        );
        Err(HttpError::RequestFailed(error_msg))
    }
}

/// 获取通用请求头（不包含Content-Type，用于GET请求）
fn get_common_headers_without_content_type(token: &str) -> HashMap<String, String> {
    let mut headers = HashMap::new();
    headers.insert("Authorization".to_string(), format!("Bearer {}", token));
    headers
}

/// 公共API GET请求方法（不需要认证）
pub async fn api_get_public<T>(path: &str) -> Result<Option<T>, HttpError>
where
    T: for<'de> serde::Deserialize<'de>,
{
    let api_domain = get_api_domain()?;
    let url = format!("{}/{}", api_domain, path.trim_start_matches('/'));
    let client = HttpClient::new();
    let resp: ApiResponse<T> = client
        .request_with_headers("GET", &url, None::<&()>, None)
        .await?;
    if resp.code == 200 {
        Ok(resp.data)
    } else {
        // 对于公共API的标准ApiResponse，也直接使用服务器返回的message
        let error_msg = resp.message.trim().to_string();
        log::warn!(
            "公共GET API请求失败 [{}] 状态码:{} -> {}",
            path,
            resp.code,
            error_msg
        );
        Err(HttpError::RequestFailed(error_msg))
    }
}

/// 获取公共API请求头（不需要认证）
fn get_public_headers() -> HashMap<String, String> {
    let mut headers = HashMap::new();
    headers.insert("Content-Type".to_string(), "application/json".to_string());
    headers
}


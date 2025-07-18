use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tauri_plugin_http::{
    reqwest,
    reqwest::header::{CONTENT_TYPE, HeaderMap, HeaderName, HeaderValue},
};

/// 统一API响应结构体
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiResponse<T> {
    pub code: i32,
    pub message: String,
    pub data: Option<T>,
    pub timestamp: i64,
}

/// HTTP请求配置
#[derive(Debug, Clone)]
pub struct HttpConfig {
    pub timeout: Option<u64>,
    pub headers: Option<HashMap<String, String>>,
    pub user_agent: Option<String>,
}

impl Default for HttpConfig {
    fn default() -> Self {
        Self {
            timeout: Some(30),
            headers: None,
            user_agent: Some("ClipPal/1.0".to_string()),
        }
    }
}

/// HTTP请求错误
#[derive(Debug, thiserror::Error)]
pub enum HttpError {
    #[error("请求失败: {0}")]
    RequestFailed(String),
    #[error("序列化失败: {0}")]
    SerializationFailed(String),
    #[error("反序列化失败: {0}")]
    DeserializationFailed(String),
    #[error("无效的URL: {0}")]
    InvalidUrl(String),
    #[error("超时: {0}")]
    Timeout(String),
    #[error("网络错误: {0}")]
    NetworkError(String),
    #[error("全局AppHandle获取失败")]
    AppHandleNotFound,
}

/// HTTP客户端
pub struct HttpClient {
    config: HttpConfig,
}

impl HttpClient {
    /// 创建新的HTTP客户端
    pub fn new() -> Self {
        Self {
            config: HttpConfig::default(),
        }
    }

    /// 使用自定义配置创建HTTP客户端
    pub fn with_config(config: HttpConfig) -> Self {
        Self { config }
    }

    /// 设置请求配置
    pub fn set_config(mut self, config: HttpConfig) -> Self {
        self.config = config;
        self
    }

    /// 设置超时时间
    pub fn timeout(mut self, timeout: u64) -> Self {
        self.config.timeout = Some(timeout);
        self
    }

    /// 设置请求头
    pub fn headers(mut self, headers: HashMap<String, String>) -> Self {
        self.config.headers = Some(headers);
        self
    }

    /// 设置User-Agent
    pub fn user_agent(mut self, user_agent: String) -> Self {
        self.config.user_agent = Some(user_agent);
        self
    }

    /// 发起GET请求
    pub async fn get<T>(&self, url: &str) -> Result<ApiResponse<T>, HttpError>
    where
        T: for<'de> Deserialize<'de>,
    {
        self.request::<(), T>("GET", url, None, None).await
    }

    /// 发起带查询参数的GET请求
    pub async fn get_with_params<T>(
        &self,
        url: &str,
        params: &HashMap<String, String>,
    ) -> Result<ApiResponse<T>, HttpError>
    where
        T: for<'de> Deserialize<'de>,
    {
        let url_with_params = self.build_url_with_params(url, params)?;
        self.request::<(), T>("GET", &url_with_params, None, None)
            .await
    }

    /// 发起POST请求
    pub async fn post<T, U>(&self, url: &str, data: Option<&T>) -> Result<ApiResponse<U>, HttpError>
    where
        T: Serialize,
        U: for<'de> Deserialize<'de>,
    {
        self.request("POST", url, data, None).await
    }

    /// 发起带JSON数据的POST请求
    pub async fn post_json<T, U>(&self, url: &str, data: &T) -> Result<ApiResponse<U>, HttpError>
    where
        T: Serialize,
        U: for<'de> Deserialize<'de>,
    {
        let mut headers = HashMap::new();
        headers.insert("Content-Type".to_string(), "application/json".to_string());
        self.request_with_headers("POST", url, Some(data), Some(headers))
            .await
    }

    /// 发起带表单数据的POST请求
    pub async fn post_form<T, U>(
        &self,
        url: &str,
        data: &HashMap<String, String>,
    ) -> Result<ApiResponse<U>, HttpError>
    where
        U: for<'de> Deserialize<'de>,
    {
        let mut headers = HashMap::new();
        headers.insert(
            "Content-Type".to_string(),
            "application/x-www-form-urlencoded".to_string(),
        );
        self.request_with_headers("POST", url, Some(data), Some(headers))
            .await
    }

    /// 发起带自定义请求头的请求
    pub async fn request_with_headers<T, U>(
        &self,
        method: &str,
        url: &str,
        data: Option<&T>,
        headers: Option<HashMap<String, String>>,
    ) -> Result<ApiResponse<U>, HttpError>
    where
        T: Serialize,
        U: for<'de> Deserialize<'de>,
    {
        let mut all_headers = self.config.headers.clone().unwrap_or_default();
        if let Some(custom_headers) = headers {
            all_headers.extend(custom_headers);
        }
        self.request(method, url, data, Some(all_headers)).await
    }

    /// 核心请求方法
    async fn request<T, U>(
        &self,
        method: &str,
        url: &str,
        data: Option<&T>,
        headers: Option<HashMap<String, String>>,
    ) -> Result<ApiResponse<U>, HttpError>
    where
        T: Serialize,
        U: for<'de> Deserialize<'de>,
    {
        // 验证URL
        let _parsed_url = reqwest::Url::parse(url)
            .map_err(|e| HttpError::InvalidUrl(format!("无效的URL: {}", e)))?;

        // 构建请求体
        let body = if let Some(data) = data {
            serde_json::to_string(data)
                .map_err(|e| HttpError::SerializationFailed(e.to_string()))?
        } else {
            String::new()
        };

        // 构建请求头
        let mut header_map = HeaderMap::new();

        // 设置默认User-Agent
        if let Some(user_agent) = &self.config.user_agent {
            header_map.insert(
                "User-Agent",
                HeaderValue::from_str(user_agent)
                    .map_err(|e| HttpError::RequestFailed(format!("无效的User-Agent: {}", e)))?,
            );
        }

        // 设置Content-Type
        if !body.is_empty() && method != "GET" {
            header_map.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
        }

        // 设置自定义请求头
        if let Some(custom_headers) = headers {
            for (key, value) in custom_headers {
                let header_name = HeaderName::from_lowercase(key.to_lowercase().as_bytes())
                    .map_err(|e| HttpError::RequestFailed(format!("无效的请求头名称: {}", e)))?;
                header_map.insert(
                    header_name,
                    HeaderValue::from_str(&value)
                        .map_err(|e| HttpError::RequestFailed(format!("无效的请求头值: {}", e)))?,
                );
            }
        }

        // 构建请求选项
        let mut options = tauri_plugin_http::reqwest::ClientBuilder::new();

        // 设置超时
        if let Some(timeout) = self.config.timeout {
            options = options.timeout(std::time::Duration::from_secs(timeout));
        }

        // 发起请求
        let client = options
            .build()
            .map_err(|e| HttpError::RequestFailed(format!("创建HTTP客户端失败: {}", e)))?;

        let request_builder = match method.to_uppercase().as_str() {
            "GET" => client.get(url),
            "POST" => client.post(url).body(body),
            "PUT" => client.put(url).body(body),
            "DELETE" => client.delete(url),
            "PATCH" => client.patch(url).body(body),
            _ => {
                return Err(HttpError::RequestFailed(format!(
                    "不支持的HTTP方法: {}",
                    method
                )));
            }
        };

        let request = request_builder
            .headers(header_map)
            .build()
            .map_err(|e| HttpError::RequestFailed(format!("构建请求失败: {}", e)))?;

        // 执行请求
        let response = client
            .execute(request)
            .await
            .map_err(|e| HttpError::NetworkError(format!("网络请求失败: {}", e)))?;

        let response_text = response
            .text()
            .await
            .map_err(|e| HttpError::NetworkError(format!("读取响应失败: {}", e)))?;

        // 统一反序列化为ApiResponse<U>
        let api_response: ApiResponse<U> = serde_json::from_str(&response_text)
            .map_err(|e| HttpError::DeserializationFailed(format!("反序列化响应失败: {}", e)))?;

        Ok(api_response)
    }

    /// 构建带查询参数的URL
    fn build_url_with_params(
        &self,
        base_url: &str,
        params: &HashMap<String, String>,
    ) -> Result<String, HttpError> {
        let mut url = reqwest::Url::parse(base_url)
            .map_err(|e| HttpError::InvalidUrl(format!("无效的URL: {}", e)))?;

        for (key, value) in params {
            url.query_pairs_mut().append_pair(key, value);
        }

        Ok(url.to_string())
    }
}

/// 便捷的HTTP请求函数
pub async fn get<T>(url: &str) -> Result<ApiResponse<T>, HttpError>
where
    T: for<'de> Deserialize<'de>,
{
    HttpClient::new().get(url).await
}

pub async fn post<T, U>(url: &str, data: &T) -> Result<ApiResponse<U>, HttpError>
where
    T: Serialize,
    U: for<'de> Deserialize<'de>,
{
    HttpClient::new().post_json(url, data).await
}

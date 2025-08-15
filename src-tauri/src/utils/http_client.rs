#![allow(dead_code)]

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;
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

/// 原始HTTP响应结构体（用于任意返回格式）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RawResponse<T> {
    pub status: u16,
    pub headers: HashMap<String, String>,
    pub data: T,
    pub url: String,
}

/// HTTP请求配置
#[derive(Debug, Clone)]
pub struct HttpConfig {
    pub timeout: Option<u64>,
    pub file_timeout: Option<u64>,
    pub headers: Option<HashMap<String, String>>,
    pub user_agent: Option<String>,
}

impl Default for HttpConfig {
    fn default() -> Self {
        Self {
            timeout: Some(30),
            file_timeout: Some(60),
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
    #[error("文件错误: {0}")]
    FileError(String),
    #[error("文件大小超过限制: {0}")]
    FileSizeExceeded(String),
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

    /// 发起GET请求（返回ApiResponse格式）
    pub async fn get<T>(&self, url: &str) -> Result<ApiResponse<T>, HttpError>
    where
        T: for<'de> Deserialize<'de>,
    {
        self.request::<(), T>("GET", url, None, None).await
    }

    /// 发起带查询参数的GET请求（返回ApiResponse格式）
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

    /// 发起POST请求（返回ApiResponse格式）
    pub async fn post<T, U>(&self, url: &str, data: Option<&T>) -> Result<ApiResponse<U>, HttpError>
    where
        T: Serialize,
        U: for<'de> Deserialize<'de>,
    {
        self.request("POST", url, data, None).await
    }

    /// 发起带JSON数据的POST请求（返回ApiResponse格式）
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

    /// 发起带表单数据的POST请求（返回ApiResponse格式）
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

    /// 发起文件上传请求（返回ApiResponse格式）
    pub async fn post_multipart<U>(
        &self,
        url: &str,
        file_path: &Path,
        form_data: &HashMap<String, String>,
    ) -> Result<ApiResponse<U>, HttpError>
    where
        U: for<'de> Deserialize<'de>,
    {
        self.request_multipart(url, file_path, form_data).await
    }

    /// 发起带自定义请求头的请求（返回ApiResponse格式）
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

    // ========== 通用HTTP请求方法（返回任意格式） ==========

    /// 发起GET请求（返回原始响应格式）
    pub async fn get_raw<T>(&self, url: &str) -> Result<RawResponse<T>, HttpError>
    where
        T: for<'de> Deserialize<'de>,
    {
        self.request_raw::<(), T>("GET", url, None, None).await
    }

    /// 发起带查询参数的GET请求（返回原始响应格式）
    pub async fn get_with_params_raw<T>(
        &self,
        url: &str,
        params: &HashMap<String, String>,
    ) -> Result<RawResponse<T>, HttpError>
    where
        T: for<'de> Deserialize<'de>,
    {
        let url_with_params = self.build_url_with_params(url, params)?;
        self.request_raw::<(), T>("GET", &url_with_params, None, None)
            .await
    }

    /// 发起POST请求（返回原始响应格式）
    pub async fn post_raw<T, U>(
        &self,
        url: &str,
        data: Option<&T>,
    ) -> Result<RawResponse<U>, HttpError>
    where
        T: Serialize,
        U: for<'de> Deserialize<'de>,
    {
        self.request_raw("POST", url, data, None).await
    }

    /// 发起带JSON数据的POST请求（返回原始响应格式）
    pub async fn post_json_raw<T, U>(
        &self,
        url: &str,
        data: &T,
    ) -> Result<RawResponse<U>, HttpError>
    where
        T: Serialize,
        U: for<'de> Deserialize<'de>,
    {
        let mut headers = HashMap::new();
        headers.insert("Content-Type".to_string(), "application/json".to_string());
        self.request_with_headers_raw("POST", url, Some(data), Some(headers))
            .await
    }

    /// 发起带表单数据的POST请求（返回原始响应格式）
    pub async fn post_form_raw<T, U>(
        &self,
        url: &str,
        data: &HashMap<String, String>,
    ) -> Result<RawResponse<U>, HttpError>
    where
        U: for<'de> Deserialize<'de>,
    {
        let mut headers = HashMap::new();
        headers.insert(
            "Content-Type".to_string(),
            "application/x-www-form-urlencoded".to_string(),
        );
        self.request_with_headers_raw("POST", url, Some(data), Some(headers))
            .await
    }

    /// 发起带自定义请求头的请求（返回原始响应格式）
    pub async fn request_with_headers_raw<T, U>(
        &self,
        method: &str,
        url: &str,
        data: Option<&T>,
        headers: Option<HashMap<String, String>>,
    ) -> Result<RawResponse<U>, HttpError>
    where
        T: Serialize,
        U: for<'de> Deserialize<'de>,
    {
        let mut all_headers = self.config.headers.clone().unwrap_or_default();
        if let Some(custom_headers) = headers {
            all_headers.extend(custom_headers);
        }
        self.request_raw(method, url, data, Some(all_headers)).await
    }

    /// 核心请求方法（返回ApiResponse格式）
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
        let response_text = self.execute_request(method, url, data, headers).await?;

        // 直接反序列化为ApiResponse<U>
        let api_response: ApiResponse<U> = serde_json::from_str(&response_text).map_err(|e| {
            HttpError::DeserializationFailed(format!(
                "反序列化ApiResponse失败，请求url：{}，返回结果：{}: {}",
                url, response_text, e
            ))
        })?;

        Ok(api_response)
    }

    /// 核心请求方法（返回原始响应格式）
    async fn request_raw<T, U>(
        &self,
        method: &str,
        url: &str,
        data: Option<&T>,
        headers: Option<HashMap<String, String>>,
    ) -> Result<RawResponse<U>, HttpError>
    where
        T: Serialize,
        U: for<'de> Deserialize<'de>,
    {
        let (status, response_url, response_headers, response_text) = self
            .execute_request_with_response(method, url, data, headers)
            .await?;

        // 读取响应头
        let mut headers_map = HashMap::new();
        for (key, value) in response_headers.iter() {
            if let Ok(value_str) = value.to_str() {
                headers_map.insert(key.as_str().to_string(), value_str.to_string());
            }
        }

        // 反序列化响应数据
        let response_data = if response_text.is_empty() {
            serde_json::from_str("null").map_err(|e| {
                HttpError::DeserializationFailed(format!("反序列化空响应失败: {}", e))
            })?
        } else {
            serde_json::from_str(&response_text)
                .map_err(|e| HttpError::DeserializationFailed(format!("反序列化响应失败: {}", e)))?
        };

        Ok(RawResponse {
            status,
            headers: headers_map,
            data: response_data,
            url: response_url,
        })
    }

    /// 执行HTTP请求并返回响应文本（公共方法）
    async fn execute_request<T>(
        &self,
        method: &str,
        url: &str,
        data: Option<&T>,
        headers: Option<HashMap<String, String>>,
    ) -> Result<String, HttpError>
    where
        T: Serialize,
    {
        let (_, _, _, response_text) = self
            .execute_request_with_response(method, url, data, headers)
            .await?;
        Ok(response_text)
    }

    /// 执行HTTP请求并返回响应信息（公共方法）
    async fn execute_request_with_response<T>(
        &self,
        method: &str,
        url: &str,
        data: Option<&T>,
        headers: Option<HashMap<String, String>>,
    ) -> Result<(u16, String, reqwest::header::HeaderMap, String), HttpError>
    where
        T: Serialize,
    {
        // 打印请求信息（debug级别）
        log::debug!("=== HTTP请求开始 ===");
        log::debug!("请求方法: {}", method);
        log::debug!("请求URL: {}", url);

        // 验证URL
        let _parsed_url = reqwest::Url::parse(url).map_err(|e| {
            let error_msg = format!("无效的URL: {}", e);
            log::error!("URL验证失败: {}", error_msg);
            HttpError::InvalidUrl(error_msg)
        })?;

        // 构建请求体
        let body = if let Some(data) = data {
            serde_json::to_string(data).map_err(|e| {
                let error_msg = format!("序列化请求数据失败: {}", e);
                log::error!("请求数据序列化失败: {}", error_msg);
                HttpError::SerializationFailed(error_msg)
            })?
        } else {
            String::new()
        };

        // 打印请求体信息（debug级别）
        if !body.is_empty() {
            log::debug!("请求体: {}", body);
        } else {
            log::debug!("请求体: 空");
        }

        // 构建请求头
        let mut header_map = HeaderMap::new();

        // 设置默认User-Agent
        if let Some(user_agent) = &self.config.user_agent {
            header_map.insert(
                "User-Agent",
                HeaderValue::from_str(user_agent).map_err(|e| {
                    let error_msg = format!("无效的User-Agent: {}", e);
                    log::error!("User-Agent设置失败: {}", error_msg);
                    HttpError::RequestFailed(error_msg)
                })?,
            );
            log::debug!("User-Agent: {}", user_agent);
        }

        // 设置Content-Type
        if !body.is_empty() && method != "GET" {
            header_map.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
            log::debug!("Content-Type: application/json");
        }

        // 设置自定义请求头
        if let Some(custom_headers) = headers {
            log::debug!("自定义请求头:");
            for (key, value) in &custom_headers {
                let header_name = HeaderName::from_lowercase(key.to_lowercase().as_bytes())
                    .map_err(|e| {
                        let error_msg = format!("无效的请求头名称: {}", e);
                        log::error!("请求头名称无效: {}", error_msg);
                        HttpError::RequestFailed(error_msg)
                    })?;
                header_map.insert(
                    header_name,
                    HeaderValue::from_str(value).map_err(|e| {
                        let error_msg = format!("无效的请求头值: {}", e);
                        log::error!("请求头值无效: {}", error_msg);
                        HttpError::RequestFailed(error_msg)
                    })?,
                );
                log::debug!("  {}: {}", key, value);
            }
        }

        // 构建请求选项
        let mut options = tauri_plugin_http::reqwest::ClientBuilder::new();

        // 设置超时
        if let Some(timeout) = self.config.timeout {
            options = options.timeout(std::time::Duration::from_secs(timeout));
            log::debug!("请求超时设置: {}秒", timeout);
        }

        // 发起请求
        let client = options.build().map_err(|e| {
            let error_msg = format!("创建HTTP客户端失败: {}", e);
            log::error!("HTTP客户端创建失败: {}", error_msg);
            HttpError::RequestFailed(error_msg)
        })?;

        let request_builder = match method.to_uppercase().as_str() {
            "GET" => client.get(url),
            "POST" => client.post(url).body(body),
            "PUT" => client.put(url).body(body),
            "DELETE" => client.delete(url),
            "PATCH" => client.patch(url).body(body),
            _ => {
                let error_msg = format!("不支持的HTTP方法: {}", method);
                log::error!("不支持的HTTP方法: {}", error_msg);
                return Err(HttpError::RequestFailed(error_msg));
            }
        };

        let request = request_builder.headers(header_map).build().map_err(|e| {
            let error_msg = format!("构建请求失败: {}", e);
            log::error!("请求构建失败: {}", error_msg);
            HttpError::RequestFailed(error_msg)
        })?;

        log::debug!("=== 开始执行HTTP请求 ===");

        // 执行请求
        let response = client.execute(request).await.map_err(|e| {
            let error_msg = format!("网络请求失败: {}", e);
            log::error!("网络请求执行失败: {}", error_msg);
            log::error!("错误详情: {:?}", e);
            HttpError::NetworkError(error_msg)
        })?;

        let status = response.status().as_u16();
        let response_url = response.url().to_string();
        let response_headers = response.headers().clone();

        log::debug!("=== HTTP响应信息 ===");
        log::debug!("响应状态码: {}", status);
        log::debug!("响应URL: {}", response_url);
        log::debug!("响应头数量: {}", response_headers.len());

        // 打印响应头信息（debug级别）
        for (name, value) in response_headers.iter() {
            if let Ok(value_str) = value.to_str() {
                log::debug!("响应头 {}: {}", name, value_str);
            }
        }

        let response_text = response.text().await.map_err(|e| {
            let error_msg = format!("读取响应失败: {}", e);
            log::error!("响应内容读取失败: {}", error_msg);
            log::error!("错误详情: {:?}", e);
            HttpError::NetworkError(error_msg)
        })?;

        // 根据状态码决定日志级别
        match status {
            200..=299 => {
                // 成功状态码 (2xx)
                log::debug!("=== HTTP请求成功 ===");
                log::debug!("状态码: {} (成功)", status);
                log::debug!("响应内容长度: {} 字符", response_text.len());
                // 如果响应内容太长，只打印前500个字符
                if response_text.len() > 500 {
                    log::debug!("响应内容预览: {}...", &response_text[..500]);
                } else {
                    log::debug!("响应内容: {}", response_text);
                }
            }
            300..=399 => {
                // 重定向状态码 (3xx)
                log::debug!("=== HTTP请求重定向 ===");
                log::debug!("状态码: {} (重定向)", status);
                log::debug!("响应内容: {}", response_text);
            }
            400..=499 => {
                // 客户端错误状态码 (4xx)
                log::error!("=== HTTP请求失败 (客户端错误) ===");
                log::error!("状态码: {} (客户端错误)", status);
                log::error!("响应内容: {}", response_text);
            }
            500..=599 => {
                // 服务器错误状态码 (5xx)
                log::error!("=== HTTP请求失败 (服务器错误) ===");
                log::error!("状态码: {} (服务器错误)", status);
                log::error!("响应内容: {}", response_text);
            }
            _ => {
                // 其他未知状态码
                log::warn!("=== HTTP请求未知状态 ===");
                log::warn!("状态码: {} (未知)", status);
                log::warn!("响应内容: {}", response_text);
            }
        }

        Ok((status, response_url, response_headers, response_text))
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

    /// 核心文件上传方法（multipart/form-data）
    async fn request_multipart<U>(
        &self,
        url: &str,
        file_path: &Path,
        form_data: &HashMap<String, String>,
    ) -> Result<ApiResponse<U>, HttpError>
    where
        U: for<'de> Deserialize<'de>,
    {
        // 打印请求信息（debug级别）
        log::debug!("=== HTTP文件上传请求开始 ===");
        log::debug!("请求URL: {}", url);
        log::debug!("文件路径: {:?}", file_path);

        // 验证URL
        let _parsed_url = reqwest::Url::parse(url).map_err(|e| {
            let error_msg = format!("无效的URL: {}", e);
            log::error!("URL验证失败: {}", error_msg);
            HttpError::InvalidUrl(error_msg)
        })?;

        // 检查文件是否存在
        if !file_path.exists() {
            let error_msg = format!("文件不存在: {:?}", file_path);
            log::error!("文件验证失败: {}", error_msg);
            return Err(HttpError::FileError(error_msg));
        }

        // 检查文件大小
        let file_metadata = std::fs::metadata(file_path).map_err(|e| {
            let error_msg = format!("读取文件元数据失败: {}", e);
            log::error!("文件元数据读取失败: {}", error_msg);
            HttpError::FileError(error_msg)
        })?;

        // 获取文件大小限制配置
        use crate::utils::config::get_max_file_size_bytes;
        let max_file_size = get_max_file_size_bytes().unwrap_or(5 * 1024 * 1024); // 默认5MB

        if file_metadata.len() > max_file_size {
            let error_msg = format!(
                "文件大小 {} 字节超过限制 {} 字节",
                file_metadata.len(),
                max_file_size
            );
            log::error!("文件大小超过限制: {}", error_msg);
            return Err(HttpError::FileSizeExceeded(error_msg));
        }

        log::debug!("文件大小: {} 字节", file_metadata.len());

        // 构建multipart表单
        let mut form = reqwest::multipart::Form::new();

        // 添加表单字段
        for (key, value) in form_data {
            form = form.text(key.clone(), value.clone());
            log::debug!("表单字段 {}: {}", key, value);
        }

        // 读取文件内容
        let file_content = std::fs::read(file_path).map_err(|e| {
            let error_msg = format!("读取文件失败: {}", e);
            log::error!("文件读取失败: {}", error_msg);
            HttpError::FileError(error_msg)
        })?;

        // 获取文件名
        let file_name = file_path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("file");

        // 根据文件扩展名推断MIME类型
        let mime_type = get_mime_type_from_extension(file_path);
        log::debug!("检测到文件MIME类型: {}", mime_type);

        // 添加文件到表单，设置Content-Type
        let file_part = reqwest::multipart::Part::bytes(file_content)
            .file_name(file_name.to_string())
            .mime_str(&mime_type)
            .map_err(|e| {
                let error_msg = format!("设置MIME类型失败: {}", e);
                log::error!("MIME类型设置失败: {}", error_msg);
                HttpError::RequestFailed(error_msg)
            })?;

        form = form.part("file", file_part);

        log::debug!("文件添加到表单: {}", file_name);

        // 构建请求头
        let mut header_map = HeaderMap::new();

        // 设置默认User-Agent
        if let Some(user_agent) = &self.config.user_agent {
            header_map.insert(
                "User-Agent",
                HeaderValue::from_str(user_agent).map_err(|e| {
                    let error_msg = format!("无效的User-Agent: {}", e);
                    log::error!("User-Agent设置失败: {}", error_msg);
                    HttpError::RequestFailed(error_msg)
                })?,
            );
            log::debug!("User-Agent: {}", user_agent);
        }

        // 设置自定义请求头（但不设置Content-Type，让reqwest自动处理multipart）
        if let Some(custom_headers) = &self.config.headers {
            log::debug!("自定义请求头:");
            for (key, value) in custom_headers {
                if key.to_lowercase() != "content-type" {
                    let header_name = HeaderName::from_lowercase(key.to_lowercase().as_bytes())
                        .map_err(|e| {
                            let error_msg = format!("无效的请求头名称: {}", e);
                            log::error!("请求头名称无效: {}", error_msg);
                            HttpError::RequestFailed(error_msg)
                        })?;
                    header_map.insert(
                        header_name,
                        HeaderValue::from_str(value).map_err(|e| {
                            let error_msg = format!("无效的请求头值: {}", e);
                            log::error!("请求头值无效: {}", error_msg);
                            HttpError::RequestFailed(error_msg)
                        })?,
                    );
                    log::debug!("  {}: {}", key, value);
                }
            }
        }

        // 构建请求选项
        let mut options = tauri_plugin_http::reqwest::ClientBuilder::new();

        // 设置超时
        if let Some(file_timeout) = self.config.file_timeout {
            options = options.timeout(std::time::Duration::from_secs(file_timeout));
            log::debug!("请求超时设置: {}秒", file_timeout);
        }

        // 发起请求
        let client = options.build().map_err(|e| {
            let error_msg = format!("创建HTTP客户端失败: {}", e);
            log::error!("HTTP客户端创建失败: {}", error_msg);
            HttpError::RequestFailed(error_msg)
        })?;

        let request_builder = client.post(url).multipart(form);

        let request = request_builder.headers(header_map).build().map_err(|e| {
            let error_msg = format!("构建请求失败: {}", e);
            log::error!("请求构建失败: {}", error_msg);
            HttpError::RequestFailed(error_msg)
        })?;

        log::debug!("=== 开始执行HTTP文件上传请求 ===");

        // 执行请求
        let response = client.execute(request).await.map_err(|e| {
            let error_msg = format!("网络请求失败: {}", e);
            log::error!("网络请求执行失败: {}", error_msg);
            log::error!("错误详情: {:?}", e);
            HttpError::NetworkError(error_msg)
        })?;

        let status = response.status().as_u16();
        let response_url = response.url().to_string();

        log::debug!("=== HTTP响应信息 ===");
        log::debug!("响应状态码: {}", status);
        log::debug!("响应URL: {}", response_url);

        let response_text = response.text().await.map_err(|e| {
            let error_msg = format!("读取响应失败: {}", e);
            log::error!("响应内容读取失败: {}", error_msg);
            log::error!("错误详情: {:?}", e);
            HttpError::NetworkError(error_msg)
        })?;

        // 根据状态码决定日志级别
        match status {
            200..=299 => {
                log::debug!("=== HTTP文件上传请求成功 ===");
                log::debug!("状态码: {} (成功)", status);
                log::debug!("响应内容长度: {} 字符", response_text.len());
                if response_text.len() > 500 {
                    log::debug!("响应内容预览: {}...", &response_text[..500]);
                } else {
                    log::debug!("响应内容: {}", response_text);
                }
            }
            _ => {
                log::error!("=== HTTP文件上传请求失败 ===");
                log::error!("状态码: {}", status);
                log::error!("响应内容: {}", response_text);
            }
        }

        // 直接反序列化为ApiResponse<U>
        let api_response: ApiResponse<U> = serde_json::from_str(&response_text).map_err(|e| {
            HttpError::DeserializationFailed(format!(
                "反序列化ApiResponse失败，请求url：{}，返回结果：{}: {}",
                url, response_text, e
            ))
        })?;

        Ok(api_response)
    }
}

/// 返回ApiResponse格式的HTTP请求函数
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

/// 便捷的HTTP请求函数（返回原始响应格式）
pub async fn get_raw<T>(url: &str) -> Result<RawResponse<T>, HttpError>
where
    T: for<'de> Deserialize<'de>,
{
    HttpClient::new().get_raw(url).await
}

pub async fn post_raw<T, U>(url: &str, data: &T) -> Result<RawResponse<U>, HttpError>
where
    T: Serialize,
    U: for<'de> Deserialize<'de>,
{
    HttpClient::new().post_json_raw(url, data).await
}

/// 根据文件扩展名推断MIME类型
fn get_mime_type_from_extension(file_path: &Path) -> String {
    let extension = file_path
        .extension()
        .and_then(|ext| ext.to_str())
        .unwrap_or("")
        .to_lowercase();

    match extension.as_str() {
        // 图片类型
        "jpg" | "jpeg" => "image/jpeg",
        "png" => "image/png",
        "gif" => "image/gif",
        "bmp" => "image/bmp",
        "webp" => "image/webp",
        "svg" => "image/svg+xml",
        "ico" => "image/x-icon",
        "tiff" | "tif" => "image/tiff",
        
        // 文档类型
        "pdf" => "application/pdf",
        "doc" => "application/msword",
        "docx" => "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
        "xls" => "application/vnd.ms-excel",
        "xlsx" => "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
        "ppt" => "application/vnd.ms-powerpoint",
        "pptx" => "application/vnd.openxmlformats-officedocument.presentationml.presentation",
        "txt" => "text/plain",
        "csv" => "text/csv",
        "rtf" => "application/rtf",
        
        // 压缩文件
        "zip" => "application/zip",
        "rar" => "application/vnd.rar",
        "7z" => "application/x-7z-compressed",
        "tar" => "application/x-tar",
        "gz" => "application/gzip",
        "bz2" => "application/x-bzip2",
        
        // 音频类型
        "mp3" => "audio/mpeg",
        "wav" => "audio/wav",
        "flac" => "audio/flac",
        "aac" => "audio/aac",
        "ogg" => "audio/ogg",
        "m4a" => "audio/mp4",
        
        // 视频类型
        "mp4" => "video/mp4",
        "avi" => "video/x-msvideo",
        "mov" => "video/quicktime",
        "wmv" => "video/x-ms-wmv",
        "flv" => "video/x-flv",
        "mkv" => "video/x-matroska",
        "webm" => "video/webm",
        
        // 代码文件
        "html" | "htm" => "text/html",
        "css" => "text/css",
        "js" => "application/javascript",
        "json" => "application/json",
        "xml" => "application/xml",
        "yaml" | "yml" => "application/x-yaml",
        "toml" => "application/toml",
        
        // 编程语言文件
        "java" => "text/x-java-source",
        "py" => "text/x-python",
        "rs" => "text/x-rust",
        "cpp" | "cxx" | "cc" => "text/x-c++src",
        "c" => "text/x-csrc",
        "h" => "text/x-chdr",
        "cs" => "text/x-csharp",
        "go" => "text/x-go",
        "php" => "text/x-php",
        "rb" => "text/x-ruby",
        "swift" => "text/x-swift",
        "kt" => "text/x-kotlin",
        "ts" => "application/typescript",
        "vue" => "text/x-vue",
        "jsx" => "text/jsx",
        "tsx" => "text/tsx",
        
        // 可执行文件
        "exe" => "application/x-msdownload",
        "msi" => "application/x-msi",
        "dmg" => "application/x-apple-diskimage",
        "deb" => "application/vnd.debian.binary-package",
        "rpm" => "application/x-rpm",
        "apk" => "application/vnd.android.package-archive",
        
        // 字体文件
        "ttf" => "font/ttf",
        "otf" => "font/otf",
        "woff" => "font/woff",
        "woff2" => "font/woff2",
        "eot" => "application/vnd.ms-fontobject",
        
        // 默认为二进制流
        _ => "application/octet-stream",
    }
    .to_string()
}

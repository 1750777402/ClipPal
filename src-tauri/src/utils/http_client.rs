#![allow(dead_code)]

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::fs::File;
use std::io::Write;
use tauri_plugin_http::{
    reqwest,
    reqwest::header::{HeaderMap, HeaderName, HeaderValue},
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
    #[error("文件错误: {0}")]
    FileError(String),
    #[error("文件大小超过限制: {0}")]
    FileSizeExceeded(String),
    #[error("文件下载失败: {0}")]
    DownloadFailed(String),
}

/// 请求数据类型枚举
enum RequestData {
    Json(String),
    Form(HashMap<String, String>),
    Multipart(reqwest::multipart::Form),
    None,
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

    // ========== ApiResponse格式的请求方法 ==========

    /// 发起GET请求（返回ApiResponse格式）
    pub async fn get<T>(&self, url: &str) -> Result<ApiResponse<T>, HttpError>
    where
        T: for<'de> Deserialize<'de>,
    {
        self.execute_api_request("GET", url, RequestData::None, None).await
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
        self.execute_api_request("GET", &url_with_params, RequestData::None, None).await
    }

    /// 发起POST请求（返回ApiResponse格式）
    pub async fn post<T, U>(&self, url: &str, data: Option<&T>) -> Result<ApiResponse<U>, HttpError>
    where
        T: Serialize,
        U: for<'de> Deserialize<'de>,
    {
        let request_data = if let Some(data) = data {
            let json_str = serde_json::to_string(data).map_err(|e| {
                HttpError::SerializationFailed(format!("序列化请求数据失败: {}", e))
            })?;
            RequestData::Json(json_str)
        } else {
            RequestData::None
        };
        self.execute_api_request("POST", url, request_data, None).await
    }

    /// 发起带JSON数据的POST请求（返回ApiResponse格式）
    pub async fn post_json<T, U>(&self, url: &str, data: &T) -> Result<ApiResponse<U>, HttpError>
    where
        T: Serialize,
        U: for<'de> Deserialize<'de>,
    {
        let json_str = serde_json::to_string(data).map_err(|e| {
            HttpError::SerializationFailed(format!("序列化请求数据失败: {}", e))
        })?;
        let mut headers = HashMap::new();
        headers.insert("Content-Type".to_string(), "application/json".to_string());
        self.execute_api_request("POST", url, RequestData::Json(json_str), Some(headers)).await
    }

    /// 发起带表单数据的POST请求（返回ApiResponse格式）
    pub async fn post_form<U>(
        &self,
        url: &str,
        data: &HashMap<String, String>,
    ) -> Result<ApiResponse<U>, HttpError>
    where
        U: for<'de> Deserialize<'de>,
    {
        let mut headers = HashMap::new();
        headers.insert("Content-Type".to_string(), "application/x-www-form-urlencoded".to_string());
        self.execute_api_request("POST", url, RequestData::Form(data.clone()), Some(headers)).await
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
        let form = self.build_multipart_form(file_path, form_data)?;
        self.execute_api_request("POST", url, RequestData::Multipart(form), None).await
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
        let request_data = if let Some(data) = data {
            let json_str = serde_json::to_string(data).map_err(|e| {
                HttpError::SerializationFailed(format!("序列化请求数据失败: {}", e))
            })?;
            RequestData::Json(json_str)
        } else {
            RequestData::None
        };
        self.execute_api_request(method, url, request_data, headers).await
    }

    // ========== 原始响应格式的请求方法 ==========

    /// 发起GET请求（返回原始响应格式）
    pub async fn get_raw<T>(&self, url: &str) -> Result<RawResponse<T>, HttpError>
    where
        T: for<'de> Deserialize<'de>,
    {
        self.execute_raw_response("GET", url, RequestData::None, None).await
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
        self.execute_raw_response("GET", &url_with_params, RequestData::None, None).await
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
        let request_data = if let Some(data) = data {
            let json_str = serde_json::to_string(data).map_err(|e| {
                HttpError::SerializationFailed(format!("序列化请求数据失败: {}", e))
            })?;
            RequestData::Json(json_str)
        } else {
            RequestData::None
        };
        self.execute_raw_response("POST", url, request_data, None).await
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
        let json_str = serde_json::to_string(data).map_err(|e| {
            HttpError::SerializationFailed(format!("序列化请求数据失败: {}", e))
        })?;
        let mut headers = HashMap::new();
        headers.insert("Content-Type".to_string(), "application/json".to_string());
        self.execute_raw_response("POST", url, RequestData::Json(json_str), Some(headers)).await
    }

    /// 发起带表单数据的POST请求（返回原始响应格式）
    pub async fn post_form_raw<U>(
        &self,
        url: &str,
        data: &HashMap<String, String>,
    ) -> Result<RawResponse<U>, HttpError>
    where
        U: for<'de> Deserialize<'de>,
    {
        let mut headers = HashMap::new();
        headers.insert("Content-Type".to_string(), "application/x-www-form-urlencoded".to_string());
        self.execute_raw_response("POST", url, RequestData::Form(data.clone()), Some(headers)).await
    }

    // ========== 文件下载方法 ==========

    /// 下载文件到指定路径
    pub async fn download_file(&self, url: &str, save_path: &Path) -> Result<PathBuf, HttpError> {
        self.download_file_internal(url, save_path).await
    }

    /// 下载文件并获取响应头信息
    pub async fn download_file_with_info(&self, url: &str, save_path: &Path) -> Result<(PathBuf, HashMap<String, String>), HttpError> {
        // 构建HTTP客户端
        let client = self.build_client()?;
        let headers = self.build_headers(None)?;
        
        // 发送请求获取响应头信息
        let response = client.get(url).headers(headers).send().await.map_err(|e| {
            HttpError::NetworkError(format!("网络请求失败: {}", e))
        })?;

        if !response.status().is_success() {
            return Err(HttpError::DownloadFailed(format!("下载失败: HTTP {}", response.status())));
        }

        // 提取响应头
        let response_headers = self.extract_headers(&response);

        // 下载文件内容
        let bytes = response.bytes().await.map_err(|e| {
            HttpError::NetworkError(format!("读取响应数据失败: {}", e))
        })?;

        // 确保目录存在并写入文件
        if let Some(parent_dir) = save_path.parent() {
            if !parent_dir.exists() {
                std::fs::create_dir_all(parent_dir).map_err(|e| {
                    HttpError::FileError(format!("创建目录失败: {}", e))
                })?;
            }
        }

        let mut file = File::create(save_path).map_err(|e| {
            HttpError::FileError(format!("创建文件失败: {}", e))
        })?;

        file.write_all(&bytes).map_err(|e| {
            HttpError::FileError(format!("写入文件失败: {}", e))
        })?;

        file.flush().map_err(|e| {
            HttpError::FileError(format!("文件刷新失败: {}", e))
        })?;

        Ok((save_path.to_path_buf(), response_headers))
    }

    // ========== 内部实现方法 ==========

    /// 统一的HTTP请求执行方法 - ApiResponse格式
    async fn execute_api_request<T>(
        &self,
        method: &str,
        url: &str,
        data: RequestData,
        custom_headers: Option<HashMap<String, String>>,
    ) -> Result<ApiResponse<T>, HttpError>
    where
        T: for<'de> Deserialize<'de>,
    {
        let response_text = self.execute_raw_request(method, url, data, custom_headers).await?;
        
        serde_json::from_str(&response_text).map_err(|e| {
            HttpError::DeserializationFailed(format!(
                "反序列化ApiResponse失败，请求url：{}，返回结果：{}: {}",
                url, response_text, e
            ))
        })
    }

    /// 统一的HTTP请求执行方法 - Raw格式  
    async fn execute_raw_response<T>(
        &self,
        method: &str,
        url: &str,
        data: RequestData,
        custom_headers: Option<HashMap<String, String>>,
    ) -> Result<RawResponse<T>, HttpError>
    where
        T: for<'de> Deserialize<'de>,
    {
        log::debug!("=== HTTP请求开始 ===");
        log::debug!("请求方法: {}, URL: {}", method, url);

        // 验证URL
        let _parsed_url = reqwest::Url::parse(url).map_err(|e| {
            HttpError::InvalidUrl(format!("无效的URL: {}", e))
        })?;

        // 构建HTTP客户端
        let client = self.build_client()?;

        // 构建请求
        let mut request_builder = match method.to_uppercase().as_str() {
            "GET" => client.get(url),
            "POST" => client.post(url),
            "PUT" => client.put(url),
            "DELETE" => client.delete(url),
            "PATCH" => client.patch(url),
            _ => return Err(HttpError::RequestFailed(format!("不支持的HTTP方法: {}", method))),
        };

        // 设置请求体
        request_builder = self.apply_request_data(request_builder, data)?;

        // 设置请求头
        let headers = self.build_headers(custom_headers.as_ref())?;
        request_builder = request_builder.headers(headers);

        // 发送请求
        let response = request_builder.send().await.map_err(|e| {
            HttpError::NetworkError(format!("网络请求失败: {}", e))
        })?;

        let status = response.status().as_u16();
        let response_url = response.url().to_string();
        let response_headers = self.extract_headers(&response);

        log::debug!("响应状态码: {}", status);

        // 读取响应体
        let response_text = response.text().await.map_err(|e| {
            HttpError::NetworkError(format!("读取响应失败: {}", e))
        })?;

        let response_data: T = if response_text.is_empty() {
            serde_json::from_str("null").map_err(|e| {
                HttpError::DeserializationFailed(format!("反序列化空响应失败: {}", e))
            })?
        } else {
            serde_json::from_str(&response_text).map_err(|e| {
                HttpError::DeserializationFailed(format!("反序列化响应失败: {}", e))
            })?
        };

        Ok(RawResponse {
            status,
            headers: response_headers,
            data: response_data,
            url: response_url,
        })
    }

    /// 执行原始HTTP请求并返回响应文本
    async fn execute_raw_request(
        &self,
        method: &str,
        url: &str,
        data: RequestData,
        custom_headers: Option<HashMap<String, String>>,
    ) -> Result<String, HttpError> {
        log::debug!("=== HTTP请求开始 ===");
        log::debug!("请求方法: {}, URL: {}", method, url);

        // 验证URL
        let _parsed_url = reqwest::Url::parse(url).map_err(|e| {
            HttpError::InvalidUrl(format!("无效的URL: {}", e))
        })?;

        // 构建HTTP客户端
        let client = self.build_client()?;

        // 构建请求
        let mut request_builder = match method.to_uppercase().as_str() {
            "GET" => client.get(url),
            "POST" => client.post(url),
            "PUT" => client.put(url),
            "DELETE" => client.delete(url),
            "PATCH" => client.patch(url),
            _ => return Err(HttpError::RequestFailed(format!("不支持的HTTP方法: {}", method))),
        };

        // 设置请求体
        request_builder = self.apply_request_data(request_builder, data)?;

        // 设置请求头
        let headers = self.build_headers(custom_headers.as_ref())?;
        request_builder = request_builder.headers(headers);

        // 发送请求
        let response = request_builder.send().await.map_err(|e| {
            HttpError::NetworkError(format!("网络请求失败: {}", e))
        })?;

        log::debug!("响应状态码: {}", response.status());

        // 读取响应体
        response.text().await.map_err(|e| {
            HttpError::NetworkError(format!("读取响应失败: {}", e))
        })
    }

    /// 实际的文件下载实现
    async fn download_file_internal(&self, url: &str, save_path: &Path) -> Result<PathBuf, HttpError> {
        log::debug!("=== HTTP文件下载开始 ===");
        log::debug!("下载URL: {}, 保存路径: {:?}", url, save_path);

        // 确保目录存在
        if let Some(parent_dir) = save_path.parent() {
            if !parent_dir.exists() {
                std::fs::create_dir_all(parent_dir).map_err(|e| {
                    HttpError::FileError(format!("创建目录失败: {}", e))
                })?;
            }
        }

        // 构建HTTP客户端和发送请求
        let client = self.build_client()?;
        let headers = self.build_headers(None)?;
        
        let response = client.get(url).headers(headers).send().await.map_err(|e| {
            HttpError::NetworkError(format!("网络请求失败: {}", e))
        })?;

        if !response.status().is_success() {
            return Err(HttpError::DownloadFailed(format!("下载失败: HTTP {}", response.status())));
        }

        // 读取响应体并写入文件
        let bytes = response.bytes().await.map_err(|e| {
            HttpError::NetworkError(format!("读取响应数据失败: {}", e))
        })?;

        let mut file = File::create(save_path).map_err(|e| {
            HttpError::FileError(format!("创建文件失败: {}", e))
        })?;

        file.write_all(&bytes).map_err(|e| {
            HttpError::FileError(format!("写入文件失败: {}", e))
        })?;

        file.flush().map_err(|e| {
            HttpError::FileError(format!("文件刷新失败: {}", e))
        })?;

        log::debug!("=== HTTP文件下载完成 ===");
        log::debug!("已下载 {} 字节到 {:?}", bytes.len(), save_path);

        Ok(save_path.to_path_buf())
    }

    /// 构建HTTP客户端
    fn build_client(&self) -> Result<reqwest::Client, HttpError> {
        let mut client_builder = reqwest::ClientBuilder::new();
        
        if let Some(timeout) = self.config.timeout {
            client_builder = client_builder.timeout(std::time::Duration::from_secs(timeout));
        }
        
        client_builder.build().map_err(|e| {
            HttpError::RequestFailed(format!("创建HTTP客户端失败: {}", e))
        })
    }

    /// 构建请求头
    fn build_headers(&self, additional_headers: Option<&HashMap<String, String>>) -> Result<HeaderMap, HttpError> {
        let mut header_map = HeaderMap::new();

        // 设置默认User-Agent
        if let Some(user_agent) = &self.config.user_agent {
            header_map.insert(
                "User-Agent",
                HeaderValue::from_str(user_agent).unwrap_or_else(|_| HeaderValue::from_static("ClipPal/1.0")),
            );
        }

        // 设置配置中的请求头
        if let Some(config_headers) = &self.config.headers {
            self.apply_headers_to_map(&mut header_map, config_headers)?;
        }

        // 设置额外的请求头
        if let Some(additional) = additional_headers {
            self.apply_headers_to_map(&mut header_map, additional)?;
        }

        Ok(header_map)
    }

    /// 将HashMap格式的请求头应用到HeaderMap
    fn apply_headers_to_map(&self, header_map: &mut HeaderMap, headers: &HashMap<String, String>) -> Result<(), HttpError> {
        for (key, value) in headers {
            let header_name = HeaderName::from_lowercase(key.to_lowercase().as_bytes())
                .map_err(|e| HttpError::RequestFailed(format!("无效的请求头名称: {}", e)))?;
            header_map.insert(
                header_name,
                HeaderValue::from_str(value).map_err(|e| {
                    HttpError::RequestFailed(format!("无效的请求头值: {}", e))
                })?,
            );
        }
        Ok(())
    }

    /// 应用请求数据到请求构建器
    fn apply_request_data(&self, mut builder: reqwest::RequestBuilder, data: RequestData) -> Result<reqwest::RequestBuilder, HttpError> {
        match data {
            RequestData::Json(json_str) => {
                builder = builder.body(json_str);
            }
            RequestData::Form(form_data) => {
                builder = builder.form(&form_data);
            }
            RequestData::Multipart(form) => {
                builder = builder.multipart(form);
            }
            RequestData::None => {}
        }
        Ok(builder)
    }

    /// 构建multipart表单
    fn build_multipart_form(&self, file_path: &Path, form_data: &HashMap<String, String>) -> Result<reqwest::multipart::Form, HttpError> {
        // 检查文件是否存在
        if !file_path.exists() {
            return Err(HttpError::FileError(format!("文件不存在: {:?}", file_path)));
        }

        // 检查文件大小
        let file_metadata = std::fs::metadata(file_path).map_err(|e| {
            HttpError::FileError(format!("读取文件元数据失败: {}", e))
        })?;

        // 获取文件大小限制配置
        use crate::utils::config::get_max_file_size_bytes;
        let max_file_size = get_max_file_size_bytes().unwrap_or(5 * 1024 * 1024);

        if file_metadata.len() > max_file_size {
            return Err(HttpError::FileSizeExceeded(format!(
                "文件大小 {} 字节超过限制 {} 字节",
                file_metadata.len(),
                max_file_size
            )));
        }

        // 构建multipart表单
        let mut form = reqwest::multipart::Form::new();

        // 添加表单字段
        for (key, value) in form_data {
            form = form.text(key.clone(), value.clone());
        }

        // 读取文件内容
        let file_content = std::fs::read(file_path).map_err(|e| {
            HttpError::FileError(format!("读取文件失败: {}", e))
        })?;

        // 获取文件名和MIME类型
        let file_name = file_path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("file");

        let mime_type = get_mime_type_from_extension(file_path);

        // 添加文件到表单
        let file_part = reqwest::multipart::Part::bytes(file_content)
            .file_name(file_name.to_string())
            .mime_str(&mime_type)
            .map_err(|e| HttpError::RequestFailed(format!("设置MIME类型失败: {}", e)))?;

        form = form.part("file", file_part);

        Ok(form)
    }

    /// 提取响应头
    fn extract_headers(&self, response: &reqwest::Response) -> HashMap<String, String> {
        let mut headers = HashMap::new();
        for (key, value) in response.headers().iter() {
            if let Ok(value_str) = value.to_str() {
                headers.insert(key.as_str().to_string(), value_str.to_string());
            }
        }
        headers
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

/// 便捷的文件下载函数
pub async fn download_file(url: &str, save_path: &Path) -> Result<PathBuf, HttpError> {
    HttpClient::new().download_file(url, save_path).await
}

/// 便捷的文件下载函数（带响应头信息）
pub async fn download_file_with_info(url: &str, save_path: &Path) -> Result<(PathBuf, HashMap<String, String>), HttpError> {
    HttpClient::new().download_file_with_info(url, save_path).await
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
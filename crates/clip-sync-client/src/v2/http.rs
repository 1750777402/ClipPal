//! HTTPS 和 SSE 共用部署令牌及连接池。连接绑定到一个已验证的部署地址。
use super::{protocol::*, StreamEvent, SyncError, SyncTransport};
use crate::auth::SyncConnection;
use async_trait::async_trait;
use reqwest::{header, Client, Method, Response};
use serde::{de::DeserializeOwned, Serialize};
use std::time::Duration;

const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);
const SSE_IDLE: Duration = Duration::from_secs(45);
const MAX_BODY: usize = 16 * 1024 * 1024;
const MAX_SSE_LINE: usize = 8 * 1024;

/// 自建版 HTTPS 传输。商业版可以另接账号和令牌更新的实现。
pub struct HttpsTransport {
    http: Client,
    connection: SyncConnection,
}

impl HttpsTransport {
    /// 禁止重定向，确保令牌和剪贴板密文始终发给已配置的部署。
    pub fn new(connection: SyncConnection) -> Result<Self, SyncError> {
        let http = Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .connect_timeout(Duration::from_secs(10))
            .build()
            .map_err(network_error)?;
        Ok(Self { http, connection })
    }

    /// 暴露规范化后的部署地址供本机绑定游标，不包含凭证。
    pub fn endpoint(&self) -> &str {
        self.connection.endpoint()
    }

    /// 为 JSON 和 SSE 请求设置统一认证头，并收敛HTTP状态分类。
    async fn send(
        &self, method: Method, path: &str, body: Option<&(impl Serialize + Sync)>,
        accept: &str, timeout: Duration,
    ) -> Result<Response, SyncError> {
        let url = self.connection.request_url(path).map_err(SyncError::Blocked)?;
        let use_query = method == Method::GET;
        let mut authorization = header::HeaderValue::from_str(&self.connection.authorization())
            .map_err(|_| SyncError::Blocked("同步认证头格式无效".into()))?;
        authorization.set_sensitive(true);
        let mut request = self.http.request(method, url)
            .header(header::AUTHORIZATION, authorization)
            .header(header::ACCEPT, accept)
            .timeout(timeout);
        if let Some(body) = body {
            request = if use_query { request.query(body) } else { request.json(body) };
        }
        let response = request.send().await.map_err(network_error)?;
        let status = response.status();
        if status.is_success() {
            Ok(response)
        } else if status.is_server_error() || matches!(status.as_u16(), 408 | 429) {
            Err(SyncError::Retry(format!("同步服务暂时不可用（HTTP {status}）")))
        } else {
            Err(SyncError::Blocked(format!("同步请求被拒绝（HTTP {status}）")))
        }
    }

    /// 边读边限制响应大小，避免错误或异常响应占据过多内存。
    async fn json<T: DeserializeOwned>(
        &self, method: Method, path: &str, body: Option<&(impl Serialize + Sync)>,
    ) -> Result<T, SyncError> {
        let mut response = self.send(method, path, body, "application/json", REQUEST_TIMEOUT).await?;
        let mut bytes = Vec::new();
        while let Some(chunk) = response.chunk().await.map_err(network_error)? {
            if chunk.len() > MAX_BODY - bytes.len() {
                return Err(SyncError::Blocked("同步响应超过大小限制".into()));
            }
            bytes.extend_from_slice(&chunk);
        }
        serde_json::from_slice(&bytes)
            .map_err(|_| SyncError::Blocked("同步响应格式无效".into()))
    }
}

/// 只记录错误类型，避免第三方错误体或 URL 被日志输出。
fn network_error(error: reqwest::Error) -> SyncError {
    let detail = if error.is_timeout() { "请求超时" } else { "连接或读取失败" };
    SyncError::Retry(format!("同步网络异常：{detail}"))
}

#[async_trait]
impl SyncTransport for HttpsTransport {
    async fn scope(&self) -> Result<Scope, SyncError> {
        let scope: Scope = self.json(Method::GET, "sync/v2/scope", None::<&()>).await?;
        if !scope.is_valid() {
            return Err(SyncError::Blocked("同步范围或数据集标识无效".into()));
        }
        Ok(scope)
    }

    async fn upload(&self, request: &UploadRequest) -> Result<UploadResponse, SyncError> {
        self.json(Method::POST, "sync/v2/operations", Some(request)).await
    }

    async fn pull(&self, request: &PullRequest) -> Result<PullPage, SyncError> {
        self.json(Method::GET, "sync/v2/changes", Some(request)).await
    }

    async fn listen(
        &self, scope: &Scope, notify: &(dyn Fn(StreamEvent) + Send + Sync),
    ) -> Result<(), SyncError> {
        // SSE 需要长连接。连接后用独立的空闲超时识别半断开和心跳缺失。
        let mut response = tokio::time::timeout(REQUEST_TIMEOUT, self.send(
            Method::GET, "sync/v2/events", Some(scope), "text/event-stream",
            Duration::from_secs(24 * 3600),
        )).await.map_err(|_| SyncError::Retry("等待同步通知连接超时".into()))??;
        let mime = response.headers().get(header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .and_then(|value| value.split(';').next());
        if !mime.is_some_and(|value| value.trim().eq_ignore_ascii_case("text/event-stream")) {
            return Err(SyncError::Blocked("同步通知响应不是 SSE".into()));
        }
        notify(StreamEvent::Connected);
        let mut frame = SseFrame::default();
        loop {
            let chunk = tokio::time::timeout(SSE_IDLE, response.chunk()).await
                .map_err(|_| SyncError::Retry("同步通知心跳超时".into()))?
                .map_err(network_error)?;
            let Some(chunk) = chunk else { return Ok(()) };
            frame.read(&chunk, notify)?;
        }
    }
}

/// 只读取通知帧的类型和完整边界。支持分块、注释心跳、LF、CRLF 和 CR。
#[derive(Default)]
struct SseFrame {
    line: Vec<u8>,
    skip_lf: bool,
    change: bool,
    has_data: bool,
}

impl SseFrame {
    fn read(&mut self, bytes: &[u8], notify: &dyn Fn(StreamEvent)) -> Result<(), SyncError> {
        for &byte in bytes {
            let skip_lf = std::mem::replace(&mut self.skip_lf, byte == b'\r');
            match byte {
                b'\n' if skip_lf => {}
                b'\n' | b'\r' => {
                    if self.line.is_empty() {
                        if self.change && self.has_data {
                            notify(StreamEvent::Changed);
                        }
                        self.change = false;
                        self.has_data = false;
                    } else {
                        let line = self.line.strip_prefix(b"\xef\xbb\xbf").unwrap_or(&self.line);
                        if let Some(value) = line.strip_prefix(b"event:") {
                            self.change = value.strip_prefix(b" ").unwrap_or(value) == b"changed";
                        } else if line == b"data" || line.starts_with(b"data:") {
                            self.has_data = true;
                        }
                    }
                    self.line.clear();
                }
                _ if self.line.len() == MAX_SSE_LINE => {
                    return Err(SyncError::Blocked("SSE 通知行过长".into()));
                }
                _ => self.line.push(byte),
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;

    /// 真实 SSE 字节可以在任意位置分块。心跳和不完整事件都不应触发补拉。
    #[test]
    fn fragmented_sse_dispatches_only_complete_change_events() {
        let count = Cell::new(0);
        let notify = |event| {
            assert!(matches!(event, StreamEvent::Changed));
            count.set(count.get() + 1);
        };
        let mut frame = SseFrame::default();
        for bytes in [
            b"\xef\xbb\xbfeve".as_slice(),
            b"nt: changed\r",
            b"\ndata: {\"latest\":102}\r",
            b"\n",
        ] {
            frame.read(bytes, &notify).unwrap();
        }
        assert_eq!(count.get(), 0);
        frame.read(b"\r\n: heartbeat\r\n\r\nevent: ignored\ndata: 42\n\n", &notify).unwrap();
        assert_eq!(count.get(), 1);
        frame.read(b"event: changed\rdata: 103\r\r", &notify).unwrap();
        assert_eq!(count.get(), 2);
    }

    /// 分页使用 GET，验证真正的 HTTP 请求构造器能序列化扁平范围和 64 位游标。
    #[test]
    fn pull_query_contains_dataset_and_exact_cursor() {
        let request = PullRequest {
            scope: Scope { scope_id: "private".into(), dataset_id: "dataset-1".into() },
            after: i64::MAX - 1,
            limit: BATCH_LIMIT,
        };
        let http = Client::new();
        let request = http.get("https://sync.test/sync/v2/changes").query(&request).build().unwrap();
        let params: std::collections::HashMap<_, _> = request.url().query_pairs().into_owned().collect();
        assert_eq!(params["scope_id"], "private");
        assert_eq!(params["dataset_id"], "dataset-1");
        assert_eq!(params["after"], (i64::MAX - 1).to_string());
        assert_eq!(params["limit"], "100");
    }
}

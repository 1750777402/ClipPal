//! 同步连接与凭据端口。凭据不能出现在 Debug 或 URL 中。
use async_trait::async_trait;
use url::Url;

#[derive(Clone)]
pub struct SyncConnection {
    endpoint: Url,
    bearer_token: String,
}

impl SyncConnection {
    pub fn new(endpoint: &str, bearer_token: &str) -> Result<Self, String> {
        let mut endpoint = Url::parse(endpoint).map_err(|_| "同步服务地址无效".to_owned())?;
        if endpoint.scheme() != "https" || endpoint.host_str().is_none()
            || !endpoint.username().is_empty() || endpoint.password().is_some()
            || endpoint.query().is_some() || endpoint.fragment().is_some()
        {
            return Err("同步服务地址必须为不含账号、查询参数和片段的 HTTPS 地址".into());
        }
        if bearer_token.is_empty() || !bearer_token.bytes().all(|b| b.is_ascii_graphic()) {
            return Err("同步认证令牌为空或格式无效".into());
        }
        if !endpoint.path().ends_with('/') {
            endpoint.set_path(&format!("{}/", endpoint.path()));
        }
        Ok(Self { endpoint, bearer_token: bearer_token.to_owned() })
    }

    pub fn request_url(&self, path: &str) -> Result<String, String> {
        // 只允许客户端内定义的相对端点，防止凭据被发送到其他服务。
        if path.contains(':') || path.starts_with("//")
            || path.contains('\\') || path.split('/').any(|part| part == "..")
            || path.contains('?') || path.contains('#')
        {
            return Err("同步端点路径无效".into());
        }
        let url = self.endpoint.join(path.trim_start_matches('/'))
            .map_err(|_| "同步端点路径无效".to_owned())?;
        if url.origin() != self.endpoint.origin() || !url.path().starts_with(self.endpoint.path()) {
            return Err("同步端点不能离开当前部署地址".into());
        }
        Ok(url.to_string())
    }

    pub fn authorization(&self) -> String { format!("Bearer {}", self.bearer_token) }
}

impl std::fmt::Debug for SyncConnection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SyncConnection").field("endpoint", &self.endpoint.as_str())
            .field("bearer_token", &"[REDACTED]").finish()
    }
}

#[async_trait]
pub trait SyncAuthProvider: Send + Sync {
    fn is_ready(&self) -> bool;
    async fn connection(&self) -> Result<SyncConnection, String>;
    /// 返回 true 表示凭据已刷新，可以重试一次。部署令牌没有账号刷新流程。
    async fn refresh(&self) -> Result<bool, String> { Ok(false) }
}

pub struct DisabledSyncAuth;
#[async_trait]
impl SyncAuthProvider for DisabledSyncAuth {
    fn is_ready(&self) -> bool { false }
    async fn connection(&self) -> Result<SyncConnection, String> {
        Err("尚未配置同步服务认证".into())
    }
}

/// 传输结果不绑定特定 HTTP 客户端或账号实现。
pub struct SyncResponse<T> { pub code: i32, pub message: String, pub data: Option<T> }
pub enum SyncRequestError<E> {
    Auth(String),
    Transport(E),
    Rejected { code: i32, message: String },
}

/// 统一认证重试边界：只对 401 尝试刷新，且最多重发一次。
/// 部署令牌 provider 默认不刷新；网络失败不修改认证状态。
pub async fn authenticated_request<T, E, F, Fut>(
    auth: &dyn SyncAuthProvider, request: F,
) -> Result<Option<T>, SyncRequestError<E>>
where
    F: Fn(SyncConnection) -> Fut,
    Fut: std::future::Future<Output = Result<SyncResponse<T>, E>>,
{
    for attempt in 0..2 {
        let connection = auth.connection().await.map_err(SyncRequestError::Auth)?;
        let response = request(connection).await.map_err(SyncRequestError::Transport)?;
        if response.code == 200 { return Ok(response.data); }
        if response.code == 401 && attempt == 0 && auth.refresh().await.map_err(SyncRequestError::Auth)? {
            continue;
        }
        return Err(SyncRequestError::Rejected { code: response.code, message: response.message });
    }
    unreachable!("最多两次请求均在循环内返回结果")
}

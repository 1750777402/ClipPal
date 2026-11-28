//! 同步连接与凭据端口。凭据不能出现在 Debug 或 URL 中。
use async_trait::async_trait;
use url::Url;

#[derive(Clone)]
pub struct SyncConnection {
    endpoint: Url,
    bearer_token: String,
}

impl SyncConnection {
    /// 正式连接始终要求 HTTPS；商业认证也继续使用这个入口。
    pub fn new(endpoint: &str, bearer_token: &str) -> Result<Self, String> {
        Self::parse(endpoint, bearer_token, false)
    }

    /// 仅调试构建允许回环 HTTP，供本机 Tomcat 联调；发布构建仍要求 HTTPS。
    /// 不允许局域网、公网地址，也不自动跟随重定向。
    pub fn for_development(endpoint: &str, bearer_token: &str) -> Result<Self, String> {
        Self::parse(endpoint, bearer_token, cfg!(debug_assertions))
    }

    /// 两种入口共用凭证、地址和路径校验，开发例外只改变回环地址的协议限制。
    fn parse(endpoint: &str, bearer_token: &str, allow_loopback_http: bool) -> Result<Self, String> {
        let mut endpoint = Url::parse(endpoint).map_err(|_| "同步服务地址无效".to_owned())?;
        let loopback = match endpoint.host() {
            Some(url::Host::Ipv4(address)) => address.is_loopback(),
            Some(url::Host::Ipv6(address)) => address.is_loopback(),
            Some(url::Host::Domain(name)) => name.eq_ignore_ascii_case("localhost"),
            None => false,
        };
        let allowed_scheme = endpoint.scheme() == "https"
            || (allow_loopback_http && endpoint.scheme() == "http" && loopback);
        if !allowed_scheme || endpoint.host_str().is_none()
            || !endpoint.username().is_empty() || endpoint.password().is_some()
            || endpoint.query().is_some() || endpoint.fragment().is_some()
        {
            return Err("同步服务地址必须为 HTTPS；调试构建仅额外允许回环 HTTP，地址不能包含账号、查询参数或片段".into());
        }
        if bearer_token.is_empty() || !bearer_token.bytes().all(|b| b.is_ascii_graphic()) {
            return Err("同步认证令牌为空或格式无效".into());
        }
        if !endpoint.path().ends_with('/') {
            endpoint.set_path(&format!("{}/", endpoint.path()));
        }
        Ok(Self { endpoint, bearer_token: bearer_token.to_owned() })
    }

    /// 返回规范化部署地址，供本机隔离同步范围，不含认证令牌。
    pub fn endpoint(&self) -> &str { self.endpoint.as_str() }

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

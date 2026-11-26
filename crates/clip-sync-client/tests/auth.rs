use async_trait::async_trait;
use std::sync::atomic::{AtomicUsize, Ordering};
use clip_sync_client::auth::*;

struct RefreshingAuth { refreshes: AtomicUsize }
#[async_trait]
impl SyncAuthProvider for RefreshingAuth {
    fn is_ready(&self) -> bool { true }
    async fn connection(&self) -> Result<SyncConnection, String> {
        SyncConnection::new("https://cloud.test", if self.refreshes.load(Ordering::SeqCst) == 0 { "old-token" } else { "new-token" })
    }
    async fn refresh(&self) -> Result<bool, String> { self.refreshes.fetch_add(1, Ordering::SeqCst); Ok(true) }
}

#[tokio::test]
async fn retries_once_with_new_credentials_and_never_refreshes_on_network_failure() {
    let auth = RefreshingAuth { refreshes: AtomicUsize::new(0) };
    let calls = AtomicUsize::new(0);
    let result = authenticated_request(&auth, |connection| {
        let attempt = calls.fetch_add(1, Ordering::SeqCst);
        async move {
            assert_eq!(connection.authorization(), if attempt == 0 { "Bearer old-token" } else { "Bearer new-token" });
            Ok::<_, ()>(SyncResponse { code: if attempt == 0 { 401 } else { 200 }, message: String::new(), data: Some(42) })
        }
    }).await;
    assert!(matches!(result, Ok(Some(42))));
    assert_eq!(calls.load(Ordering::SeqCst), 2);
    assert_eq!(auth.refreshes.load(Ordering::SeqCst), 1);
    let result = authenticated_request(&auth, |_| async { Err::<SyncResponse<()>, _>("offline") }).await;
    assert!(matches!(result, Err(SyncRequestError::Transport("offline"))));
    assert_eq!(auth.refreshes.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn repeated_401_has_a_fixed_retry_bound() {
    let auth = RefreshingAuth { refreshes: AtomicUsize::new(0) };
    let calls = AtomicUsize::new(0);
    let result = authenticated_request(&auth, |_| {
        calls.fetch_add(1, Ordering::SeqCst);
        async { Ok::<_, ()>(SyncResponse::<()> { code: 401, message: String::new(), data: None }) }
    }).await;
    assert!(matches!(result, Err(SyncRequestError::Rejected { code: 401, .. })));
    assert_eq!(calls.load(Ordering::SeqCst), 2);
    assert_eq!(auth.refreshes.load(Ordering::SeqCst), 1);
}

#[test]
fn keeps_authorization_within_the_configured_deployment() {
    let connection = SyncConnection::new("https://sync.test/base", "secret").unwrap();
    for path in ["https://elsewhere.test", "//elsewhere.test", "../escape", "%2e%2e/escape", "https:elsewhere.test", "x?token=a", "x#fragment"] {
        assert!(connection.request_url(path).is_err(), "{path}");
    }
}

/// 本地HTTP只允许开发入口和调试构建，不能通过普通连接或公网地址绕过HTTPS。
#[test]
fn development_http_is_limited_to_loopback_and_debug_builds() {
    for endpoint in ["http://127.0.0.1:8080", "http://localhost:8080", "http://[::1]:8080"] {
        assert!(SyncConnection::new(endpoint, "test-token").is_err());
        assert_eq!(SyncConnection::for_development(endpoint, "test-token").is_ok(), cfg!(debug_assertions));
    }
    for endpoint in ["http://192.168.1.2:8080", "http://example.com", "http://localhost.example.com", "http://0.0.0.0:8080",
        "http://user:password@127.0.0.1", "http://127.0.0.1?token=a", "http://127.0.0.1#fragment"] {
        assert!(SyncConnection::for_development(endpoint, "test-token").is_err(), "{endpoint}");
    }
    let connection = SyncConnection::for_development("https://example.test/base", "test-token").unwrap();
    assert_eq!(connection.request_url("sync/v2/scope").unwrap(), "https://example.test/base/sync/v2/scope");
}

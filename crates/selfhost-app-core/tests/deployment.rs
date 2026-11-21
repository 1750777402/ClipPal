use selfhost_app_core::{deployment::DeploymentConfig, clip_sync_client::auth::{SyncAuthProvider, authenticated_request, SyncRequestError, SyncResponse}};
use std::sync::atomic::{AtomicUsize, Ordering};

#[tokio::test]
async fn deployment_auth_works_without_account_and_does_not_refresh_on_401() {
    let config = DeploymentConfig::new("https://sync.example.test/base", "deployment-secret", 100).unwrap();
    assert!(config.is_ready());
    assert_eq!(config.max_file_size_bytes(), 102400);
    let connection = config.connection().await.unwrap();
    assert_eq!(connection.request_url("clipPal-sync/sync/single").unwrap(), "https://sync.example.test/base/clipPal-sync/sync/single");
    assert_eq!(connection.authorization(), "Bearer deployment-secret");
    assert!(!format!("{connection:?}").contains("deployment-secret"));
    let calls = AtomicUsize::new(0);
    let result = authenticated_request(&config, |_| {
        calls.fetch_add(1, Ordering::SeqCst);
        async { Ok::<_, ()>(SyncResponse::<()> { code: 401, message: "invalid token".into(), data: None }) }
    }).await;
    assert!(matches!(result, Err(SyncRequestError::Rejected { code: 401, .. })));
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    assert!(config.is_ready()); // 401 不触发账号登出，也不清空部署配置。
}

#[test]
fn rejects_invalid_deployment_configuration_and_credential_routing() {
    for endpoint in ["http://sync.test", "https://user:password@sync.test", "https://sync.test?token=secret", "https://sync.test#secret"] {
        assert!(DeploymentConfig::new(endpoint, "token", 1).is_err());
    }
    for token in ["", "with space", "injected\r\nheader"] {
        assert!(DeploymentConfig::new("https://sync.test", token, 1).is_err());
    }
    assert!(DeploymentConfig::new("https://sync.test", "token", u64::MAX).is_err());
}

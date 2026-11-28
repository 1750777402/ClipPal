use selfhost_app_core::{deployment::DeploymentConfig, clip_sync_client::auth::SyncAuthProvider};

#[tokio::test]
async fn deployment_auth_works_without_account_and_does_not_refresh_on_401() {
    let config = DeploymentConfig::new("https://sync.example.test/base", "deployment-secret").unwrap();
    assert!(config.is_ready());
    let connection = config.connection().await.unwrap();
    assert_eq!(connection.request_url("sync/operations").unwrap(), "https://sync.example.test/base/sync/operations");
    assert_eq!(connection.authorization(), "Bearer deployment-secret");
    assert!(!format!("{connection:?}").contains("deployment-secret"));
    assert!(!config.refresh().await.unwrap());
    assert!(config.is_ready()); // 401 不触发账号登出，也不清空部署配置。
}

#[test]
fn rejects_invalid_deployment_configuration_and_credential_routing() {
    for endpoint in ["http://sync.test", "https://user:password@sync.test", "https://sync.test?token=secret", "https://sync.test#secret"] {
        assert!(DeploymentConfig::new(endpoint, "token").is_err());
    }
    for token in ["", "with space", "injected\r\nheader"] {
        assert!(DeploymentConfig::new("https://sync.test", token).is_err());
    }
}

/// 直接验证桌面自建配置接入了受限的回环HTTP规则，发布构建仍然拒绝HTTP。
#[tokio::test]
async fn self_hosted_debug_configuration_supports_local_tomcat() {
    let config = DeploymentConfig::new("http://127.0.0.1:8080", "test-token");
    assert_eq!(config.is_ok(), cfg!(debug_assertions));
    if let Ok(config) = config {
        assert_eq!(config.connection().await.unwrap().request_url("sync/limits").unwrap(),
            "http://127.0.0.1:8080/sync/limits");
    }
}

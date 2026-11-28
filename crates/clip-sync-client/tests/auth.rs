use clip_sync_client::auth::SyncConnection;

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
    assert_eq!(connection.request_url("sync/limits").unwrap(), "https://example.test/base/sync/limits");
}

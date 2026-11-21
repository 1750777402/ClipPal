use std::sync::Mutex;
use async_trait::async_trait;
use serde::{Serialize, de::DeserializeOwned};
use clip_sync_client::transport::{JsonTransport, SyncClient};
use clip_sync_protocol::*;

struct CaptureTransport { calls: Mutex<Vec<(String, u64)>> }
#[async_trait]
impl JsonTransport for CaptureTransport {
    type Error = &'static str;
    async fn get<T: DeserializeOwned + Send>(&self, path: &str) -> Result<Option<T>, Self::Error> {
        self.calls.lock().unwrap().push((path.into(), 0));
        Err("offline")
    }
    async fn post<P: Serialize + Sync, T: DeserializeOwned + Send>(&self, path: &str, _payload: &P, timeout: u64) -> Result<Option<T>, Self::Error> {
        self.calls.lock().unwrap().push((path.into(), timeout));
        Err("offline")
    }
}

// 传输层错误必须原样返回，宿主才能决定认证失效或断网后的恢复策略。
#[tokio::test]
async fn transport_errors_reach_the_host_without_retrying_or_mutating_requests() {
    let client = SyncClient::new(CaptureTransport { calls: Mutex::new(Vec::new()) });
    let request = CloudSyncRequest { clips: vec![], timestamp: 100, last_sync_time: 80, device_id: "test".into() };
    assert_eq!(client.sync_clipboard(&request).await.unwrap_err(), "offline");
    assert_eq!(request.last_sync_time, 80);
    assert_eq!(client.server_time().await.unwrap_err(), "offline");
}

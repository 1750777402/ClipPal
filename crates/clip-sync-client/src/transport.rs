use async_trait::async_trait;
use serde::{de::DeserializeOwned, Serialize};
use clip_sync_protocol::*;

/// 请求执行端口。部署地址、HTTPS、认证与刷新策略由宿主应用提供。
/// 泛型返回类型保留 JSON 类型检查；同步客户端不持有账号或全局上下文。
#[async_trait]
pub trait JsonTransport: Send + Sync {
    type Error: Send;
    async fn get<T: DeserializeOwned + Send>(&self, path: &str) -> Result<Option<T>, Self::Error>;
    async fn post<P: Serialize + Sync, T: DeserializeOwned + Send>(&self, path: &str, payload: &P, timeout_secs: u64) -> Result<Option<T>, Self::Error>;
}

/// 现有协议的端点调用，实现与部署认证分离，供自建版和商业版共用。
pub struct SyncClient<T> { transport: T }
impl<T: JsonTransport> SyncClient<T> {
    pub fn new(transport: T) -> Self { Self { transport } }
    /// 云同步api（1分钟超时）
    pub async fn sync_clipboard(&self, request: &CloudSyncRequest) -> Result<Option<CloudSyncResponse>, T::Error> {
        self.transport.post("clipPal-sync/sync/complete", request, 60).await
    }
    /// 获取服务器时间。
    pub async fn server_time(&self) -> Result<Option<u64>, T::Error> {
        self.transport.get("clipPal-sync/public/now").await
    }
    /// 处理单个记录的新增或者删除。
    pub async fn sync_record(&self, request: &SingleCloudSyncParam) -> Result<Option<SingleCloudSyncResponse>, T::Error> {
        self.transport.post("clipPal-sync/sync/single", request, 30).await
    }
    /// 获取上传链接。
    pub async fn upload_url(&self, request: &FileCloudSyncParam) -> Result<Option<DownloadCloudFileResponse>, T::Error> {
        self.transport.post("clipPal-sync/sync/getUploadUrl", request, 30).await
    }
    /// 通知服务端上传完成。
    pub async fn upload_success(&self, request: &FileCloudSyncParam) -> Result<Option<bool>, T::Error> {
        self.transport.post("clipPal-sync/sync/uploadSuccess", request, 30).await
    }
    /// 获取下载链接。
    pub async fn download_url(&self, request: &DownloadCloudFileParam) -> Result<Option<DownloadCloudFileResponse>, T::Error> {
        self.transport.post("clipPal-sync/sync/getDownloadUrl", request, 30).await
    }
}

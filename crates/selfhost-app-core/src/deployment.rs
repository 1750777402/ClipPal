//! 打包时提供的自建部署配置；不依赖用户账号或商业服务。
use async_trait::async_trait;
use clip_sync_client::auth::{SyncAuthProvider, SyncConnection};

pub struct DeploymentConfig {
    connection: SyncConnection,
    max_file_size_bytes: u64,
}
impl DeploymentConfig {
    pub fn new(endpoint: &str, token: &str, max_file_size_kb: u64) -> Result<Self, String> {
        let max_file_size_bytes = max_file_size_kb.checked_mul(1024)
            .ok_or_else(|| "同步文件大小配置超出范围".to_owned())?;
        Ok(Self { connection: SyncConnection::for_development(endpoint, token)?, max_file_size_bytes })
    }
    pub fn max_file_size_bytes(&self) -> u64 { self.max_file_size_bytes }
}
#[async_trait]
impl SyncAuthProvider for DeploymentConfig {
    fn is_ready(&self) -> bool { true }
    async fn connection(&self) -> Result<SyncConnection, String> { Ok(self.connection.clone()) }
}

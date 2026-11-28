//! 打包时提供的自建部署配置；不依赖用户账号或商业服务。
use async_trait::async_trait;
use clip_sync_client::auth::{SyncAuthProvider, SyncConnection};

pub struct DeploymentConfig {
    connection: SyncConnection,
}

impl DeploymentConfig {
    /// 打包时注入认证配置。同步限额由自建版代码及服务端握手提供。
    pub fn new(endpoint: &str, token: &str) -> Result<Self, String> {
        Ok(Self { connection: SyncConnection::for_development(endpoint, token)? })
    }
}
#[async_trait]
impl SyncAuthProvider for DeploymentConfig {
    fn is_ready(&self) -> bool { true }
    async fn connection(&self) -> Result<SyncConnection, String> { Ok(self.connection.clone()) }
}

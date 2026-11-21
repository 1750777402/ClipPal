//! 商业认证和权益适配。通用同步任务只调用注入的策略与凭据端口。
use async_trait::async_trait;
use {
    crate::app_context::AppContext,
    crate::core::policy::ApplicationPolicy,
    crate::commercial::vip_service::VipService,
    crate::commercial::token_manager,
};
#[cfg(feature = "sync")]
use clip_sync_client::auth::{SyncAuthProvider, SyncConnection};

pub struct CommercialPolicy;
#[async_trait]
impl ApplicationPolicy for CommercialPolicy {
    fn max_records(&self, context: &AppContext) -> Result<u32, String> {
        VipService::from_context(context).get_cached_max_records_limit()
    }
    fn cached_sync_file_limit(&self, context: &AppContext) -> Result<u64, String> {
        VipService::from_context(context).get_cached_max_file_size()
    }
    async fn sync_file_limit(&self, context: &AppContext) -> Result<u64, String> {
        VipService::from_context(context).get_max_file_size().await
    }
    async fn file_copy_limit(&self, context: &AppContext) -> u64 {
        VipService::from_context(context).get_file_copy_size_limit().await
    }
    async fn authorize_sync(&self, context: &AppContext) -> Result<(), String> {
        if !token_manager::has_valid_auth() { return Err("请先登录账号才能开启云同步功能".into()); }
        match VipService::from_context(context).check_vip_permission().await? {
            (true, _) => Ok(()), (false, message) => Err(message),
        }
    }
    async fn prepare_sync(&self, context: &AppContext) -> Result<(), String> {
        self.authorize_sync(context).await?;
        let vip = VipService::from_context(context);
        if vip.should_refresh_vip_status().unwrap_or(false) {
            if let Err(error) = vip.refresh_vip_status().await { log::warn!("VIP 状态刷新失败: {}", error); }
            self.authorize_sync(context).await?;
        }
        Ok(())
    }
    async fn can_sync_file(&self, context: &AppContext, size: u64) -> Result<(bool, String), String> {
        VipService::from_context(context).can_sync_file(size).await
    }
}

#[cfg(feature = "sync")]
pub struct CommercialSyncAuth { pub endpoint: String }
#[cfg(feature = "sync")]
#[async_trait]
impl SyncAuthProvider for CommercialSyncAuth {
    fn is_ready(&self) -> bool { token_manager::has_valid_auth() }
    async fn connection(&self) -> Result<SyncConnection, String> {
        let token = token_manager::get_valid_access_token().await?
            .ok_or_else(|| "用户未登录或令牌已过期".to_owned())?;
        SyncConnection::new(&self.endpoint, &token)
    }
    async fn refresh(&self) -> Result<bool, String> {
        token_manager::refresh_access_token().await.map(|token| token.is_some())
    }
}

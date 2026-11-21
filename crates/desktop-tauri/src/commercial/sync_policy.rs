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

/// 将商业权益服务适配成通用能力策略，避免本地核心直接依赖套餐和账号。
pub struct CommercialPolicy;
#[async_trait]
impl ApplicationPolicy for CommercialPolicy {
    /// 返回当前版本策略允许的本地记录数上限。
    fn max_records(&self, context: &AppContext) -> Result<u32, String> {
        VipService::from_context(context).get_cached_max_records_limit()
    }
    /// 只读本地配置或权益缓存，返回同步附件的字节上限。
    fn cached_sync_file_limit(&self, context: &AppContext) -> Result<u64, String> {
        VipService::from_context(context).get_cached_max_file_size()
    }
    /// 取得当前同步文件大小上限，具体是否联网由策略实现决定。
    async fn sync_file_limit(&self, context: &AppContext) -> Result<u64, String> {
        VipService::from_context(context).get_max_file_size().await
    }
    /// 检查当前凭据或部署配置是否允许同步；不直接读写记录。
    async fn authorize_sync(&self, context: &AppContext) -> Result<(), String> {
        if !token_manager::has_valid_auth() { return Err("请先登录账号才能开启云同步功能".into()); }
        match VipService::from_context(context).check_vip_permission().await? {
            (true, _) => Ok(()), (false, message) => Err(message),
        }
    }
    /// 同步前执行授权准备，商业策略可刷新已过期的权益缓存。
    async fn prepare_sync(&self, context: &AppContext) -> Result<(), String> {
        self.authorize_sync(context).await?;
        let vip = VipService::from_context(context);
        if vip.should_refresh_vip_status().unwrap_or(false) {
            if let Err(error) = vip.refresh_vip_status().await { log::warn!("VIP 状态刷新失败: {}", error); }
            self.authorize_sync(context).await?;
        }
        Ok(())
    }
    /// 根据当前策略判断单个附件大小是否允许同步，并返回原因。
    async fn can_sync_file(&self, context: &AppContext, size: u64) -> Result<(bool, String), String> {
        VipService::from_context(context).can_sync_file(size).await
    }
}

#[cfg(feature = "sync")]
/// 商业同步凭据提供者；服务地址与访问令牌在后端组合，不暴露给页面。
pub struct CommercialSyncAuth { pub endpoint: String }
#[cfg(feature = "sync")]
#[async_trait]
impl SyncAuthProvider for CommercialSyncAuth {
    /// 只检查本地登录凭据是否存在，不在就绪查询中触发网络请求。
    fn is_ready(&self) -> bool { token_manager::has_valid_auth() }
    /// 获取有效访问令牌并生成同步连接配置，不把令牌返回页面。
    async fn connection(&self) -> Result<SyncConnection, String> {
        let token = token_manager::get_valid_access_token().await?
            .ok_or_else(|| "用户未登录或令牌已过期".to_owned())?;
        SyncConnection::new(&self.endpoint, &token)
    }
    /// 尝试刷新访问令牌，返回是否取得新的有效令牌。
    async fn refresh(&self) -> Result<bool, String> {
        token_manager::refresh_access_token().await.map(|token| token.is_some())
    }
}

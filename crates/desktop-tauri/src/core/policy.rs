//! 桌面应用扩展策略端口；实现由 bootstrap 注入，本地与同步业务不认识 VIP。
use async_trait::async_trait;
use crate::app_context::AppContext;

#[async_trait]
/// 各版本的能力策略接口。本地流程调用此接口，不自行访问账号或权益存储。
pub trait ApplicationPolicy: Send + Sync {
    /// 返回当前版本策略允许的本地记录数上限。
    fn max_records(&self, context: &AppContext) -> Result<u32, String>;
    /// 只读本地配置或权益缓存，返回同步附件的字节上限。
    #[cfg(feature = "sync")]
    fn cached_sync_file_limit(&self, context: &AppContext) -> Result<u64, String>;
    /// 取得当前同步文件大小上限，具体是否联网由策略实现决定。
    #[cfg(feature = "sync")]
    async fn sync_file_limit(&self, context: &AppContext) -> Result<u64, String>;
    /// 检查当前凭据或部署配置是否允许同步；不直接读写记录。
    #[cfg(feature = "sync")]
    async fn authorize_sync(&self, context: &AppContext) -> Result<(), String>;
    /// 同步前执行授权准备，商业策略可刷新已过期的权益缓存。
    #[cfg(feature = "sync")]
    async fn prepare_sync(&self, context: &AppContext) -> Result<(), String> {
        self.authorize_sync(context).await
    }
    /// 根据当前策略判断单个附件大小是否允许同步，并返回原因。
    #[cfg(feature = "sync")]
    async fn can_sync_file(&self, context: &AppContext, size: u64) -> Result<(bool, String), String>;
}

/// 单机/自建的本地能力不受商业套餐限制；最大值只受现有 SQL 整数范围约束。
pub struct LocalPolicy;
#[async_trait]
impl ApplicationPolicy for LocalPolicy {
    /// 返回当前版本策略允许的本地记录数上限。
    fn max_records(&self, _context: &AppContext) -> Result<u32, String> { Ok(i32::MAX as u32) }
    /// 只读本地配置或权益缓存，返回同步附件的字节上限。
    #[cfg(feature = "sync")]
    fn cached_sync_file_limit(&self, _context: &AppContext) -> Result<u64, String> { Ok(0) }
    /// 取得当前同步文件大小上限，具体是否联网由策略实现决定。
    #[cfg(feature = "sync")]
    async fn sync_file_limit(&self, context: &AppContext) -> Result<u64, String> {
        self.cached_sync_file_limit(context)
    }
    /// 检查当前凭据或部署配置是否允许同步；不直接读写记录。
    #[cfg(feature = "sync")]
    async fn authorize_sync(&self, _context: &AppContext) -> Result<(), String> {
        Err("当前版本未包含云同步功能".into())
    }
    /// 根据当前策略判断单个附件大小是否允许同步，并返回原因。
    #[cfg(feature = "sync")]
    async fn can_sync_file(&self, _context: &AppContext, _size: u64) -> Result<(bool, String), String> {
        Ok((false, "当前版本未包含云同步功能".into()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    #[cfg(feature = "commercial-extension")]
    use {
    crate::commercial::context::CommercialContext,
    crate::commercial::http::HttpAuthClient,
    crate::commercial::http::HttpVipClient,
    crate::commercial::storage::AuthStore,
    crate::commercial::storage::VipStore,
    crate::commercial::domain::user::AuthSession,
    crate::commercial::domain::user::UserInfo,
    crate::commercial::domain::vip::VipInfo,
};

    #[cfg(feature = "commercial-extension")]
    /// 测试禁用的商业存储；任何错误的跨层调用都应立即失败。
    struct ForbiddenCommercialStore;
    #[cfg(feature = "commercial-extension")]
    impl AuthStore for ForbiddenCommercialStore {
        /// 测试禁用存储：若本地策略触碰商业会话，立即让测试失败。
        fn save_session(&self, _: &AuthSession) -> Result<(), String> { panic!("本地策略不应访问商业会话") }
        /// 测试禁用存储：禁止本地策略读取商业访问令牌。
        fn access_token(&self) -> Option<String> { panic!("本地策略不应读取商业令牌") }
        /// 测试禁用存储：禁止本地策略读取商业刷新令牌。
        fn refresh_token(&self) -> Option<String> { panic!("本地策略不应读取商业令牌") }
        /// 测试禁用存储：禁止本地策略读取商业用户资料。
        fn user_info(&self) -> Option<UserInfo> { panic!("本地策略不应读取商业用户") }
        /// 测试禁用存储：禁止本地策略清除商业会话。
        fn clear(&self) -> Result<(), String> { panic!("本地策略不应清理商业会话") }
        /// 测试禁用存储：禁止本地策略修改商业用户资料。
        fn update_user_nickname(&self, _: &str) -> Result<(), String> { panic!("本地策略不应修改商业用户") }
    }
    #[cfg(feature = "commercial-extension")]
    impl VipStore for ForbiddenCommercialStore {
        /// 测试禁用存储：禁止本地策略读取 VIP 缓存。
        fn get_info(&self) -> Result<Option<VipInfo>, String> { panic!("本地策略不应读取 VIP 缓存") }
        /// 测试禁用存储：禁止本地策略写入 VIP 缓存。
        fn save_checked(&self, _: &VipInfo) -> Result<(), String> { panic!("本地策略不应写入 VIP 缓存") }
        /// 测试禁用存储：禁止本地策略清空 VIP 缓存。
        fn clear_info(&self) -> Result<(), String> { panic!("本地策略不应清理 VIP 缓存") }
        /// 测试禁用存储：禁止本地策略决定商业权益刷新时机。
        fn should_refresh(&self) -> Result<bool, String> { panic!("本地策略不应刷新 VIP") }
    }

    /// 组装测试专用应用上下文；商业存储替身用于检测错误的跨层访问。
    async fn context(policy: Arc<dyn ApplicationPolicy>) -> (AppContext, std::path::PathBuf) {
        let dir = std::env::temp_dir().join(format!("clip-pal-policy-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let decode = |value: &str| -> crate::errors::AppResult<String> { Ok(value.into()) };
        let local = desktop_app_core::LocalContext::open(&dir.join("test.db"), Some(dir.join("settings.json")), Arc::new(decode)).await.unwrap();
        #[cfg(feature = "sync")]
        let application = selfhost_app_core::SelfhostContext::with_auth(local, Arc::new(
            selfhost_app_core::deployment::DeploymentConfig::new("https://sync.test", "test-deployment-token", 1).unwrap(),
        ));
        #[cfg(not(feature = "sync"))]
        let application = local;
        #[cfg(feature = "commercial-extension")]
        let commercial = CommercialContext::new(Arc::new(HttpAuthClient), Arc::new(HttpVipClient), Arc::new(ForbiddenCommercialStore), Arc::new(ForbiddenCommercialStore));
        (AppContext::new(application, #[cfg(feature = "commercial-extension")] commercial, policy), dir)
    }

    /// 验证本地功能不受账号和 VIP 存储影响。
    #[tokio::test]
    async fn local_policy_keeps_local_capabilities_independent_of_account_and_vip() {
        let (context, dir) = context(Arc::new(LocalPolicy)).await;
        assert_eq!(context.policy().max_records(&context).unwrap(), i32::MAX as u32);
        #[cfg(feature = "sync")]
        assert!(context.policy().authorize_sync(&context).await.is_err());
        assert_eq!(context.with_settings(|settings| settings.max_records).unwrap(), 200);
        drop(context);
        let _ = std::fs::remove_dir_all(dir);
    }

    /// 验证自建策略只使用部署认证及大小限制，不读取商业会话。
    #[cfg(feature = "sync")]
    #[tokio::test]
    async fn selfhost_policy_uses_deployment_auth_and_limits_without_commercial_stores() {
        let (context, dir) = context(Arc::new(crate::sync::policy::SelfhostPolicy { max_file_size_bytes: 1024 })).await;
        assert!(context.policy().prepare_sync(&context).await.is_ok());
        assert_eq!(context.policy().max_records(&context).unwrap(), i32::MAX as u32);
        assert!(context.policy().can_sync_file(&context, 1024).await.unwrap().0);
        assert!(!context.policy().can_sync_file(&context, 1025).await.unwrap().0);
        assert!(context.sync().auth().is_ready());
        drop(context);
        let _ = std::fs::remove_dir_all(dir);
    }
}

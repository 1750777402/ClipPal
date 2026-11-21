use std::sync::Arc;
use {
    crate::commercial::storage::AuthStore,
    crate::commercial::storage::VipStore,
    crate::commercial::ports::AuthGateway,
    crate::commercial::ports::VipGateway,
};

/// 商业扩展资源。
///
/// 账号、VIP 和支付服务属于商业边界。它们仍然由当前桌面应用注入，
/// 但不再和单机核心数据库、搜索、设置资源放在同一个上下文分组中。
pub struct CommercialContext {
    /// 远程认证服务端口。
    auth_gateway: Arc<dyn AuthGateway>,

    /// 远程 VIP 和支付服务端口。
    vip_gateway: Arc<dyn VipGateway>,

    /// 本地认证会话安全存储。
    auth_store: Arc<dyn AuthStore>,

    /// 本地 VIP 权益快照安全存储。
    vip_store: Arc<dyn VipStore>,
}

impl CommercialContext {
    /// 创建商业扩展资源组。
    pub fn new(
        auth_gateway: Arc<dyn AuthGateway>,
        vip_gateway: Arc<dyn VipGateway>,
        auth_store: Arc<dyn AuthStore>,
        vip_store: Arc<dyn VipStore>,
    ) -> Self {
        Self {
            auth_gateway,
            vip_gateway,
            auth_store,
            vip_store,
        }
    }

    pub fn auth_gateway(&self) -> &dyn AuthGateway {
        self.auth_gateway.as_ref()
    }

    pub fn vip_gateway(&self) -> &dyn VipGateway {
        self.vip_gateway.as_ref()
    }

    pub fn auth_store(&self) -> &dyn AuthStore {
        self.auth_store.as_ref()
    }

    pub fn vip_store(&self) -> &dyn VipStore {
        self.vip_store.as_ref()
    }
}

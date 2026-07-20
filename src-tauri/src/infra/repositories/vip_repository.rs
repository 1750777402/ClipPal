use std::collections::HashMap;

use async_trait::async_trait;

use crate::domain::vip::{
    PayCodrUrlResponse, PayParam, QueryPayParam, QueryPayResponse, ServerConfigResponse, VipInfo,
    VipLimits, VipType,
};

#[async_trait]
/// VIP 仓储接口，统一封装权益校验、缓存、服务端配置和支付数据访问。
pub trait VipRepository: Send + Sync {
    /// 登录成功后初始化 VIP 状态并执行本地记录数限制。
    async fn initialize_and_enforce_limits(&self) -> Result<(), String>;

    /// 读取本地加密缓存中的 VIP 信息，不访问网络。
    fn get_local_info(&self) -> Result<Option<VipInfo>, String>;

    /// 检查当前账号的云同步权限并返回业务说明。
    async fn check_cloud_sync_permission(&self) -> Result<(bool, String), String>;

    /// 汇总当前用户可用的记录数、文件大小和同步能力。
    async fn get_limits(&self) -> Result<VipLimits, String>;

    /// 从服务端刷新 VIP 状态并更新本地缓存和限制。
    async fn refresh(&self) -> Result<bool, String>;

    /// 获取服务端配置的全部 VIP 套餐信息。
    async fn get_server_config(
        &self,
    ) -> Result<Option<HashMap<VipType, ServerConfigResponse>>, String>;

    /// 创建支付订单并返回支付二维码。
    async fn get_pay_url(&self, param: &PayParam) -> Result<Option<PayCodrUrlResponse>, String>;

    /// 查询指定订单的支付状态。
    async fn get_pay_result(
        &self,
        param: &QueryPayParam,
    ) -> Result<Option<QueryPayResponse>, String>;
}

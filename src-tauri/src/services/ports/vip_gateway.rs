use std::collections::HashMap;

use async_trait::async_trait;

use crate::domain::vip::{
    PayCodrUrlResponse, PayParam, QueryPayParam, QueryPayResponse, ServerConfigResponse,
    UserVipInfoResponse, VipType,
};

#[async_trait]
/// VIP 远程端口，定义权益、套餐和支付流程需要的服务端能力。
pub trait VipGateway: Send + Sync {
    /// 查询当前登录用户的最新 VIP 权益快照。
    async fn fetch_current_info(&self) -> Result<Option<UserVipInfoResponse>, String>;

    /// 获取服务端配置的全部 VIP 套餐。
    async fn get_server_config(
        &self,
    ) -> Result<Option<HashMap<VipType, ServerConfigResponse>>, String>;

    /// 创建支付订单并返回二维码信息。
    async fn get_pay_url(&self, param: &PayParam) -> Result<Option<PayCodrUrlResponse>, String>;

    /// 查询指定支付订单的最新状态。
    async fn get_pay_result(
        &self,
        param: &QueryPayParam,
    ) -> Result<Option<QueryPayResponse>, String>;
}

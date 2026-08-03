use std::collections::HashMap;

use async_trait::async_trait;

use crate::{
    api::{api_get_public, api_post},
    domain::vip::{
        PayCodrUrlResponse, PayParam, QueryPayParam, QueryPayResponse, ServerConfigResponse,
        UserVipInfoResponse, VipType,
    },
    services::ports::VipGateway,
};

#[derive(Default)]
/// 通过项目统一 HTTP 传输能力访问 VIP 和支付服务。
pub struct HttpVipClient;

#[async_trait]
impl VipGateway for HttpVipClient {
    /// 查询当前登录用户的 VIP 权益快照。
    async fn fetch_current_info(&self) -> Result<Option<UserVipInfoResponse>, String> {
        api_post("clipPal-sync/vip/check", Some(&serde_json::json!({})))
            .await
            .map_err(|error| error.to_string())
    }

    /// 调用公开 VIP 配置接口，并将传输错误收敛为应用层错误文本。
    async fn get_server_config(
        &self,
    ) -> Result<Option<HashMap<VipType, ServerConfigResponse>>, String> {
        api_get_public("clipPal-sync/public/syncConfig")
            .await
            .map_err(|error| error.to_string())
    }

    /// 调用支付二维码接口。
    async fn get_pay_url(&self, param: &PayParam) -> Result<Option<PayCodrUrlResponse>, String> {
        api_post("clipPal-sync/pay/getPayCodeUrl", Some(param))
            .await
            .map_err(|error| error.to_string())
    }

    /// 调用支付结果查询接口。
    async fn get_pay_result(
        &self,
        param: &QueryPayParam,
    ) -> Result<Option<QueryPayResponse>, String> {
        api_post("clipPal-sync/pay/queryPayResult", Some(param))
            .await
            .map_err(|error| error.to_string())
    }
}

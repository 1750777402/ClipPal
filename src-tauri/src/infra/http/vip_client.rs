use std::collections::HashMap;

use async_trait::async_trait;

use crate::{
    api::vip_api,
    domain::vip::{
        PayCodrUrlResponse, PayParam, QueryPayParam, QueryPayResponse, ServerConfigResponse,
        VipType,
    },
};

#[async_trait]
/// VIP HTTP Client 接口，隔离仓储与具体接口路径和 HTTP 错误类型。
pub trait VipClient: Send + Sync {
    /// 获取所有 VIP 套餐的服务端配置。
    async fn get_server_config(
        &self,
    ) -> Result<Option<HashMap<VipType, ServerConfigResponse>>, String>;

    /// 创建支付订单并返回二维码信息。
    async fn get_pay_url(&self, param: &PayParam) -> Result<Option<PayCodrUrlResponse>, String>;

    /// 查询支付订单状态。
    async fn get_pay_result(
        &self,
        param: &QueryPayParam,
    ) -> Result<Option<QueryPayResponse>, String>;
}

#[derive(Default)]
/// 使用项目统一 HTTP API 封装的 VIP Client 实现。
pub struct HttpVipClient;

#[async_trait]
impl VipClient for HttpVipClient {
    /// 调用公开 VIP 配置接口，并将 HTTP 错误收敛为仓储可处理的字符串错误。
    async fn get_server_config(
        &self,
    ) -> Result<Option<HashMap<VipType, ServerConfigResponse>>, String> {
        vip_api::get_server_config()
            .await
            .map_err(|error| error.to_string())
    }

    /// 调用支付二维码接口。
    async fn get_pay_url(&self, param: &PayParam) -> Result<Option<PayCodrUrlResponse>, String> {
        vip_api::get_pay_url(param)
            .await
            .map_err(|error| error.to_string())
    }

    /// 调用支付结果查询接口。
    async fn get_pay_result(
        &self,
        param: &QueryPayParam,
    ) -> Result<Option<QueryPayResponse>, String> {
        vip_api::get_pay_result(param)
            .await
            .map_err(|error| error.to_string())
    }
}

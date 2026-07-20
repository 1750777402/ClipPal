use std::collections::HashMap;

pub use crate::domain::vip::{
    PayCodrUrlResponse, PayParam, QueryPayParam, QueryPayResponse, ServerConfigResponse,
    UserVipInfoResponse,
};
use crate::{
    api::{api_get_public, api_post},
    domain::vip::VipType,
    utils::http_client::HttpError,
};

/// -------------------------------------------Vip信息检测--------------------------------------------------------------

/// 用户VIP信息检查获取
pub async fn user_vip_check() -> Result<Option<UserVipInfoResponse>, HttpError> {
    api_post("clipPal-sync/vip/check", Some(&serde_json::json!({}))).await
}

/// -------------------------------------------获取服务端的vip配置信息--------------------------------------------------------------
/// 获取服务端配置信息
pub async fn get_server_config() -> Result<Option<HashMap<VipType, ServerConfigResponse>>, HttpError>
{
    api_get_public("clipPal-sync/public/syncConfig").await
}

/// -------------------------------------------获取支付二维码--------------------------------------------------------------
/// 获取支付二维码
pub async fn get_pay_url(request: &PayParam) -> Result<Option<PayCodrUrlResponse>, HttpError> {
    api_post("clipPal-sync/pay/getPayCodeUrl", Some(request)).await
}

/// -------------------------------------------查询支付结果--------------------------------------------------------------
/// 查询支付结果
pub async fn get_pay_result(
    request: &QueryPayParam,
) -> Result<Option<QueryPayResponse>, HttpError> {
    api_post("clipPal-sync/pay/queryPayResult", Some(request)).await
}

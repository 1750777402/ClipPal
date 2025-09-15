use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::{
    api::{api_get_public, api_post},
    utils::{http_client::HttpError, secure_store::VipType},
};

/// -------------------------------------------Vip信息检测--------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UserVipInfoResponse {
    pub user_id: u32,
    pub vip_flag: bool,
    pub vip_type: Option<VipType>,
    pub expire_time: Option<u64>,      // VIP过期时间戳
    pub max_records: u32,              // 最大记录条数限制
    pub max_file_size: u64,            // 最大文件大小限制(KB)
    pub features: Option<Vec<String>>, // VIP功能列表
}

/// 用户VIP信息检查获取
pub async fn user_vip_check() -> Result<Option<UserVipInfoResponse>, HttpError> {
    api_post("clipPal-sync/vip/check", Some(&serde_json::json!({}))).await
}

/// -------------------------------------------获取服务端的vip配置信息--------------------------------------------------------------
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServerConfigResponse {
    pub price: i32,               // 价格 元
    pub period: i32,              // 时效 天
    pub max_file_size: u64,       // 用户文件大小限制
    pub record_limit: u32,        // 用户本地记录条数限制
    pub sync_check_interval: u32, // 同步检查间隔(秒)
}

/// 获取服务端配置信息
pub async fn get_server_config() -> Result<Option<HashMap<VipType, ServerConfigResponse>>, HttpError>
{
    api_get_public("clipPal-sync/public/syncConfig").await
}

/// -------------------------------------------获取支付二维码--------------------------------------------------------------
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PayParam {
    pub vip_type: String, // 选择的vip类型
    pub pay_type: String, // 支付方式
}

/// 获取支付二维码
pub async fn get_pay_url(request: &PayParam) -> Result<Option<String>, HttpError> {
    api_post("clipPal-sync/pay/getUrl", Some(request)).await
}

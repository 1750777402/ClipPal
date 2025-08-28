use serde::{Deserialize, Serialize};

use crate::{api::api_post, utils::http_client::HttpError};

/// -------------------------------------------Vip信息检测--------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UserVipInfoResponse {
    pub user_id: u32,
    pub username: String,
    pub is_vip: bool,
    pub vip_type: Option<String>,          // "monthly", "quarterly", "yearly"
    pub expire_time: Option<u64>,          // VIP过期时间戳
    pub max_records: u32,                  // 最大记录条数限制
    pub max_sync_records: u32,             // 可云同步的最大记录数
    pub max_file_size: u64,                // 最大文件大小限制(字节)
    pub features: Vec<String>,             // VIP功能列表
    pub current_sync_count: Option<u32>,   // 当前已同步记录数
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServerConfigResponse {
    pub max_file_size_free: u64,          // 免费用户文件大小限制
    pub max_file_size_vip: u64,           // VIP用户文件大小限制
    pub free_sync_limit: u32,             // 免费用户云同步限制
    pub vip_sync_limit: u32,              // VIP用户云同步限制
    pub sync_check_interval: u32,         // 同步检查间隔(秒)
}

/// 用户VIP信息检查获取
pub async fn user_vip_check() -> Result<Option<UserVipInfoResponse>, HttpError> {
    api_post("cliPal-sync/vip/check", Some(&serde_json::json!({}))).await
}

/// 获取服务端配置信息
pub async fn get_server_config() -> Result<Option<ServerConfigResponse>, HttpError> {
    api_post("cliPal-sync/vip/config", Some(&serde_json::json!({}))).await
}

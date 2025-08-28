use serde::{Deserialize, Serialize};

use crate::{api::api_post, utils::http_client::HttpError};

/// -------------------------------------------Vip信息检测--------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UserVipInfoResponse {
    pub username: String,
}

/// 用户vip信息检查获取
pub async fn user_vip_check() -> Result<Option<UserVipInfoResponse>, HttpError> {
    api_post("cliPal-sync/auth/logout", Some(&String::new())).await
}

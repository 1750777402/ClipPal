use serde::{Deserialize, Serialize};

use crate::{api::api_post, utils::http_client::HttpError};

/// -------------------------------------用户登录api---------------------------------------------------------
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LoginRequestParam {
    pub username: String,
    pub password: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthResponse {
    pub access_token: String,
    pub refresh_token: String,
    pub token_type: String,
    pub expires_in: i32,
    pub user_info: UserInfo,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UserInfo {
    pub id: u64,
    pub username: String,
    pub nick_name: Option<String>,
    pub email: Option<String>,
    pub phone: Option<String>,
}

/// 用户登录接口
pub async fn user_login(request: &LoginRequestParam) -> Result<Option<AuthResponse>, HttpError> {
    api_post("cliPal-sync/auth/login", Some(request)).await
}

/// -----------------------------------------------注册api--------------------------------------------------------------------
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RegisterRequestParam {
    pub username: String,
    pub password: String,
    pub confirm_password: String,
    pub nick_name: String,
    pub email: String,
    pub captcha: String,
    pub phone: Option<String>,
}

/// 用户注册接口
pub async fn user_register(request: &RegisterRequestParam) -> Result<Option<UserInfo>, HttpError> {
    api_post("cliPal-sync/auth/register", Some(request)).await
}

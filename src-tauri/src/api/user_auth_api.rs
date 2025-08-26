use serde::{Deserialize, Serialize};

use crate::{
    api::{api_post, api_post_public},
    utils::http_client::HttpError,
};

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

/// 用户登录接口（公共接口，不需要认证）
pub async fn user_login(request: &LoginRequestParam) -> Result<Option<AuthResponse>, HttpError> {
    api_post_public("cliPal-sync/auth/login", Some(request)).await
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

/// 用户注册接口（公共接口，不需要认证）
pub async fn user_register(request: &RegisterRequestParam) -> Result<Option<UserInfo>, HttpError> {
    api_post_public("cliPal-sync/auth/register", Some(request)).await
}

/// -----------------------------------------------发送验证码--------------------------------------------------------------------
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EmailCodeRequestParam {
    pub email: String,
}

/// 发送验证码（公共接口，不需要认证）
pub async fn send_email_code(request: &EmailCodeRequestParam) -> Result<Option<bool>, HttpError> {
    api_post_public("cliPal-sync/auth/sendEmailCode", Some(request)).await
}

/// ---------------------------------------------刷新身份令牌-------------------------------------------------------------------
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RefreshTokenRequestParam {
    pub refresh_token: String,
}

/// 刷新身份令牌（公共接口，使用refresh token，不需要access token）
pub async fn refresh_token(
    request: &RefreshTokenRequestParam,
) -> Result<Option<AuthResponse>, HttpError> {
    api_post_public("cliPal-sync/auth/refresh", Some(request)).await
}

/// ---------------------------------------------退出登录-------------------------------------------------------------------

/// 用户退出登录（需要认证）
pub async fn user_logout() -> Result<Option<String>, HttpError> {
    api_post("cliPal-sync/auth/logout", Some(&String::new())).await
}

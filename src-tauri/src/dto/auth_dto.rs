use serde::{Deserialize, Serialize};

use crate::api::user_auth_api::{
    CheckUsernameRequestParam, EmailCodeRequestParam, LoginRequestParam, RegisterRequestParam,
    UserInfo as ApiUserInfo,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserInfo {
    pub id: u64,
    pub account: String,
    pub nickname: Option<String>,
    pub email: Option<String>,
    pub phone: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoginResponse {
    pub user_info: UserInfo,
    pub token: String,
    pub expires_in: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FrontendLoginRequest {
    pub account: String,
    pub password: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FrontendRegisterRequest {
    pub nickname: String,
    pub account: String,
    pub password: String,
    pub confirm_password: String,
    pub email: String,
    pub captcha: String,
    pub phone: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FrontendSendEmailCodeRequest {
    pub email: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FrontendCheckUsernameRequest {
    pub username: String,
}

impl From<FrontendLoginRequest> for LoginRequestParam {
    fn from(request: FrontendLoginRequest) -> Self {
        Self {
            username: request.account,
            password: request.password,
        }
    }
}

impl From<FrontendRegisterRequest> for RegisterRequestParam {
    fn from(request: FrontendRegisterRequest) -> Self {
        Self {
            username: request.account,
            password: request.password,
            confirm_password: request.confirm_password,
            nick_name: request.nickname,
            email: request.email,
            captcha: request.captcha,
            phone: request.phone,
        }
    }
}

impl From<FrontendSendEmailCodeRequest> for EmailCodeRequestParam {
    fn from(request: FrontendSendEmailCodeRequest) -> Self {
        Self {
            email: request.email,
        }
    }
}

impl From<FrontendCheckUsernameRequest> for CheckUsernameRequestParam {
    fn from(request: FrontendCheckUsernameRequest) -> Self {
        Self {
            username: request.username,
        }
    }
}

impl From<ApiUserInfo> for UserInfo {
    fn from(user: ApiUserInfo) -> Self {
        Self {
            id: user.id,
            account: user.username,
            nickname: user.nick_name,
            email: user.email,
            phone: user.phone,
        }
    }
}

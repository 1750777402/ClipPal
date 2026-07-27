use async_trait::async_trait;

use crate::{
    api::user_auth_api::{
        check_username as api_check_username, refresh_token as api_refresh_token,
        send_email_code as api_send_email_code, update_user_info as api_update_user_info,
        user_login, user_logout, user_register as api_user_register, CheckUsernameRequestParam,
        EmailCodeRequestParam, LoginRequestParam, RefreshTokenRequestParam, RegisterRequestParam,
        UpdateUserInfoParam, UserInfo as ApiUserInfo,
    },
    domain::user::{
        AuthSession, FrontendCheckUsernameRequest, FrontendLoginRequest, FrontendRegisterRequest,
        FrontendSendEmailCodeRequest, UserInfo,
    },
};

#[async_trait]
/// 认证服务端客户端，只负责 HTTP 请求、协议字段转换和传输错误收敛。
pub trait AuthClient: Send + Sync {
    async fn login(&self, request: FrontendLoginRequest) -> Result<Option<AuthSession>, String>;
    async fn register(&self, request: FrontendRegisterRequest) -> Result<Option<UserInfo>, String>;
    async fn send_email_code(
        &self,
        request: FrontendSendEmailCodeRequest,
    ) -> Result<Option<bool>, String>;
    async fn logout(&self) -> Result<(), String>;
    async fn check_username(
        &self,
        request: FrontendCheckUsernameRequest,
    ) -> Result<Option<bool>, String>;
    async fn update_nickname(&self, nickname: &str) -> Result<Option<bool>, String>;
    async fn refresh_session(&self, refresh_token: &str) -> Result<Option<AuthSession>, String>;
}

#[derive(Default)]
pub struct HttpAuthClient;

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

#[async_trait]
impl AuthClient for HttpAuthClient {
    async fn login(&self, request: FrontendLoginRequest) -> Result<Option<AuthSession>, String> {
        let request = LoginRequestParam {
            username: request.account,
            password: request.password,
        };
        user_login(&request)
            .await
            .map(|response| {
                response.map(|response| AuthSession {
                    access_token: response.access_token,
                    refresh_token: response.refresh_token,
                    expires_in: response.expires_in,
                    user_info: response.user_info.into(),
                })
            })
            .map_err(|error| error.to_string())
    }

    async fn register(&self, request: FrontendRegisterRequest) -> Result<Option<UserInfo>, String> {
        let request = RegisterRequestParam {
            username: request.account,
            password: request.password,
            confirm_password: request.confirm_password,
            nick_name: request.nickname,
            email: request.email,
            captcha: request.captcha,
            phone: request.phone,
        };
        api_user_register(&request)
            .await
            .map(|response| response.map(Into::into))
            .map_err(|error| error.to_string())
    }

    async fn send_email_code(
        &self,
        request: FrontendSendEmailCodeRequest,
    ) -> Result<Option<bool>, String> {
        api_send_email_code(&EmailCodeRequestParam {
            email: request.email,
        })
        .await
        .map_err(|error| error.to_string())
    }

    async fn logout(&self) -> Result<(), String> {
        user_logout()
            .await
            .map(|_| ())
            .map_err(|error| error.to_string())
    }

    async fn check_username(
        &self,
        request: FrontendCheckUsernameRequest,
    ) -> Result<Option<bool>, String> {
        api_check_username(&CheckUsernameRequestParam {
            username: request.username,
        })
        .await
        .map_err(|error| error.to_string())
    }

    async fn update_nickname(&self, nickname: &str) -> Result<Option<bool>, String> {
        api_update_user_info(&UpdateUserInfoParam {
            nick_name: nickname.to_string(),
        })
        .await
        .map_err(|error| error.to_string())
    }

    async fn refresh_session(&self, refresh_token: &str) -> Result<Option<AuthSession>, String> {
        api_refresh_token(&RefreshTokenRequestParam {
            refresh_token: refresh_token.to_string(),
        })
        .await
        .map(|response| {
            response.map(|response| AuthSession {
                access_token: response.access_token,
                refresh_token: response.refresh_token,
                expires_in: response.expires_in,
                user_info: response.user_info.into(),
            })
        })
        .map_err(|error| error.to_string())
    }
}

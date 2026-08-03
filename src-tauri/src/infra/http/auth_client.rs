use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use crate::{
    api::{api_get_public, api_post, api_post_public},
    domain::user::{
        AuthSession, FrontendCheckUsernameRequest, FrontendLoginRequest, FrontendRegisterRequest,
        FrontendSendEmailCodeRequest, UserInfo,
    },
    services::ports::AuthGateway,
};

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct LoginRequestParam {
    username: String,
    password: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct AuthResponse {
    access_token: String,
    refresh_token: String,
    #[allow(dead_code)]
    token_type: String,
    expires_in: i32,
    user_info: ApiUserInfo,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ApiUserInfo {
    id: u64,
    username: String,
    nick_name: Option<String>,
    email: Option<String>,
    phone: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct RegisterRequestParam {
    username: String,
    password: String,
    confirm_password: String,
    nick_name: String,
    email: String,
    captcha: String,
    phone: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct EmailCodeRequestParam {
    email: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct RefreshTokenRequestParam {
    refresh_token: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct UpdateUserInfoParam {
    nick_name: String,
}

#[derive(Default)]
/// 通过项目统一 HTTP 传输能力访问认证服务。
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

impl From<AuthResponse> for AuthSession {
    fn from(response: AuthResponse) -> Self {
        Self {
            access_token: response.access_token,
            refresh_token: response.refresh_token,
            expires_in: response.expires_in,
            user_info: response.user_info.into(),
        }
    }
}

#[async_trait]
impl AuthGateway for HttpAuthClient {
    /// 将登录参数转换为服务端协议，返回可供本地持久化的完整认证会话。
    async fn login(&self, request: FrontendLoginRequest) -> Result<Option<AuthSession>, String> {
        let request = LoginRequestParam {
            username: request.account,
            password: request.password,
        };
        api_post_public::<_, AuthResponse>("clipPal-sync/auth/login", Some(&request))
            .await
            .map(|response| response.map(Into::into))
            .map_err(|error| error.to_string())
    }

    /// 将注册参数转换为服务端协议并返回新建用户资料。
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
        api_post_public::<_, ApiUserInfo>("clipPal-sync/auth/register", Some(&request))
            .await
            .map(|response| response.map(Into::into))
            .map_err(|error| error.to_string())
    }

    /// 请求服务端向注册邮箱发送验证码。
    async fn send_email_code(
        &self,
        request: FrontendSendEmailCodeRequest,
    ) -> Result<Option<bool>, String> {
        let request = EmailCodeRequestParam {
            email: request.email,
        };
        api_post_public("clipPal-sync/auth/sendEmailCode", Some(&request))
            .await
            .map_err(|error| error.to_string())
    }

    /// 通知服务端结束当前访问令牌对应的会话。
    async fn logout(&self) -> Result<(), String> {
        let payload = String::new();
        api_post::<_, String>("clipPal-sync/auth/logout", Some(&payload))
            .await
            .map(|_| ())
            .map_err(|error| error.to_string())
    }

    /// 通过公开接口检查指定用户名是否可用于注册。
    async fn check_username(
        &self,
        request: FrontendCheckUsernameRequest,
    ) -> Result<Option<bool>, String> {
        let path = format!(
            "clipPal-sync/auth/checkUsername?username={}",
            request.username
        );
        api_get_public(&path)
            .await
            .map_err(|error| error.to_string())
    }

    /// 将昵称转换为服务端用户资料字段并提交更新。
    async fn update_nickname(&self, nickname: &str) -> Result<Option<bool>, String> {
        let request = UpdateUserInfoParam {
            nick_name: nickname.to_string(),
        };
        api_post("clipPal-sync/user/updateInfo", Some(&request))
            .await
            .map_err(|error| error.to_string())
    }

    /// 使用 refresh token 请求新会话，并把协议响应转换为领域会话对象。
    async fn refresh_session(&self, refresh_token: &str) -> Result<Option<AuthSession>, String> {
        let request = RefreshTokenRequestParam {
            refresh_token: refresh_token.to_string(),
        };
        api_post_public::<_, AuthResponse>("clipPal-sync/auth/refresh", Some(&request))
            .await
            .map(|response| response.map(Into::into))
            .map_err(|error| error.to_string())
    }
}

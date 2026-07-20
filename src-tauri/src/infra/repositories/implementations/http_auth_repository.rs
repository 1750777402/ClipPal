use async_trait::async_trait;

use crate::{
    api::user_auth_api::{
        check_username as api_check_username, send_email_code as api_send_email_code,
        update_user_info as api_update_user_info, user_login, user_logout,
        user_register as api_user_register, CheckUsernameRequestParam, EmailCodeRequestParam,
        LoginRequestParam, RegisterRequestParam, UpdateUserInfoParam, UserInfo as ApiUserInfo,
    },
    domain::user::{
        FrontendCheckUsernameRequest, FrontendLoginRequest, FrontendRegisterRequest,
        FrontendSendEmailCodeRequest, LoginResponse, UserInfo,
    },
    infra::{
        repositories::AuthRepository,
        security::{AuthStore, SecureAuthStore},
    },
};

impl From<FrontendLoginRequest> for LoginRequestParam {
    /// 将前端登录字段转换为服务端认证接口要求的字段名称。
    fn from(request: FrontendLoginRequest) -> Self {
        Self {
            username: request.account,
            password: request.password,
        }
    }
}

impl From<FrontendRegisterRequest> for RegisterRequestParam {
    /// 将前端注册请求完整转换为服务端注册参数。
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
    /// 将领域请求转换为发送验证码接口参数。
    fn from(request: FrontendSendEmailCodeRequest) -> Self {
        Self {
            email: request.email,
        }
    }
}

impl From<FrontendCheckUsernameRequest> for CheckUsernameRequestParam {
    /// 将领域请求转换为用户名检查接口参数。
    fn from(request: FrontendCheckUsernameRequest) -> Self {
        Self {
            username: request.username,
        }
    }
}

impl From<ApiUserInfo> for UserInfo {
    /// 将服务端用户模型转换为前端和领域层使用的用户资料。
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

#[derive(Default)]
/// 基于认证 HTTP API 和加密安全存储的认证仓储实现。
pub struct HttpAuthRepository {
    store: SecureAuthStore,
}

#[async_trait]
impl AuthRepository for HttpAuthRepository {
    /// 调用登录接口，并在成功后原子地保存令牌、用户资料和过期时间。
    async fn login(&self, request: FrontendLoginRequest) -> Result<LoginResponse, String> {
        // 参数转换集中在基础设施层，避免 service 依赖 HTTP API 类型。
        let request: LoginRequestParam = request.into();
        match user_login(&request).await {
            Ok(Some(response)) => {
                // 只有安全存储成功后才向上层返回登录成功，防止产生不可恢复会话。
                self.store.store_auth_data(&response)?;
                Ok(LoginResponse {
                    user_info: response.user_info.into(),
                    token: response.access_token,
                    expires_in: response.expires_in,
                })
            }
            Ok(None) => Err("登录响应为空".to_string()),
            Err(error) => Err(error.to_string()),
        }
    }

    /// 调用注册接口，并将服务端用户模型转换为领域模型。
    async fn register(&self, request: FrontendRegisterRequest) -> Result<UserInfo, String> {
        let request: RegisterRequestParam = request.into();
        match api_user_register(&request).await {
            Ok(Some(user_info)) => Ok(user_info.into()),
            Ok(None) => Err("注册响应为空".to_string()),
            Err(error) => Err(error.to_string()),
        }
    }

    /// 调用邮箱验证码接口，并把空值或 `false` 统一视为业务失败。
    async fn send_email_code(
        &self,
        request: FrontendSendEmailCodeRequest,
    ) -> Result<String, String> {
        let request: EmailCodeRequestParam = request.into();
        match api_send_email_code(&request).await {
            Ok(Some(true)) => Ok("验证码已发送".to_string()),
            Ok(Some(false) | None) => Err("验证码发送失败".to_string()),
            Err(error) => Err(error.to_string()),
        }
    }

    /// 通知服务端退出；本地状态清理由 service 单独保证。
    async fn logout_remote(&self) {
        let _ = user_logout().await;
    }

    /// 检查安全存储中是否存在访问令牌。
    fn has_access_token(&self) -> bool {
        self.store.access_token().is_some()
    }

    /// 从安全存储反序列化用户资料并转换为领域模型。
    fn user_info(&self) -> Option<UserInfo> {
        self.store.user_info().map(Into::into)
    }

    /// 清除本地全部认证字段。
    fn clear_auth_data(&self) -> Result<(), String> {
        self.store.clear_auth_data()
    }

    /// 调用服务端用户名检查接口，空响应按不可用处理。
    async fn check_username(&self, request: FrontendCheckUsernameRequest) -> Result<bool, String> {
        let request: CheckUsernameRequestParam = request.into();
        match api_check_username(&request).await {
            Ok(response) => response.ok_or_else(|| "用户名不可用".to_string()),
            Err(error) => Err(error.to_string()),
        }
    }

    /// 更新服务端昵称，并在服务端成功后刷新本地安全存储。
    async fn update_nickname(&self, nickname: &str) -> Result<bool, String> {
        let request = UpdateUserInfoParam {
            nick_name: nickname.to_string(),
        };

        match api_update_user_info(&request).await {
            Ok(Some(true)) => {
                if let Err(error) = self.store.update_user_nickname(nickname) {
                    log::warn!("更新本地用户信息失败: {}", error);
                }
                Ok(true)
            }
            Ok(_) => Err("昵称更新失败".to_string()),
            Err(error) => Err(error.to_string()),
        }
    }
}

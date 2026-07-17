use tauri::Emitter;

use crate::{
    api::user_auth_api::{
        check_username as api_check_username, send_email_code as api_send_email_code,
        update_user_info as api_update_user_info, user_login, user_logout as api_user_logout,
        user_register as api_user_register, CheckUsernameRequestParam, EmailCodeRequestParam,
        LoginRequestParam, RegisterRequestParam, UpdateUserInfoParam,
    },
    app_context::AppContext,
    biz::vip_checker::VipChecker,
    dto::auth_dto::{
        FrontendCheckUsernameRequest, FrontendLoginRequest, FrontendRegisterRequest,
        FrontendSendEmailCodeRequest, LoginResponse, UserInfo,
    },
    infra::security::{AuthStore, SecureAuthStore},
    utils::token_manager::has_valid_auth,
};

pub struct AuthService<'a, R> {
    context: &'a AppContext,
    store: R,
}

impl<'a> AuthService<'a, SecureAuthStore> {
    pub fn from_context(context: &'a AppContext) -> Self {
        Self {
            context,
            store: SecureAuthStore,
        }
    }
}

impl<R> AuthService<'_, R>
where
    R: AuthStore,
{
    pub async fn login(&self, param: FrontendLoginRequest) -> Result<LoginResponse, String> {
        log::info!("用户登录请求: {}", param.account);
        let api_param: LoginRequestParam = param.into();

        match user_login(&api_param).await {
            Ok(Some(response)) => {
                log::info!("用户登录成功: {}", response.user_info.username);
                self.store.store_auth_data(&response)?;

                tokio::spawn(async {
                    if let Err(error) = VipChecker::initialize_vip_and_enforce_limits().await {
                        log::error!("登录后 VIP 状态初始化失败: {}", error);
                    }
                });

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

    pub async fn register(&self, param: FrontendRegisterRequest) -> Result<UserInfo, String> {
        log::info!("用户注册请求: {}", param.account);
        let api_param: RegisterRequestParam = param.into();

        match api_user_register(&api_param).await {
            Ok(Some(user_info)) => Ok(user_info.into()),
            Ok(None) => Err("注册响应为空".to_string()),
            Err(error) => Err(error.to_string()),
        }
    }

    pub async fn send_email_code(
        &self,
        param: FrontendSendEmailCodeRequest,
    ) -> Result<String, String> {
        let api_param: EmailCodeRequestParam = param.into();
        match api_send_email_code(&api_param).await {
            Ok(Some(true)) => Ok("验证码已发送".to_string()),
            Ok(Some(false) | None) => Err("验证码发送失败".to_string()),
            Err(error) => Err(error.to_string()),
        }
    }

    pub async fn logout(&self) -> Result<String, String> {
        if has_valid_auth() {
            let _ = api_user_logout().await;
        }

        self.store.clear_auth_data()?;
        self.emit("auth-cleared");

        if let Err(error) = crate::biz::system_setting::disable_cloud_sync().await {
            log::error!("禁用云同步设置失败: {}", error);
        }
        self.emit("cloud-sync-disabled");

        Ok("退出成功".to_string())
    }

    pub fn validate_token(&self) -> Result<bool, String> {
        Ok(self.store.access_token().is_some())
    }

    pub fn get_user_info(&self) -> Result<UserInfo, String> {
        self.store
            .user_info()
            .map(Into::into)
            .ok_or_else(|| "用户未登录".to_string())
    }

    pub fn check_login_status(&self) -> Result<Option<UserInfo>, String> {
        match self.store.access_token() {
            Some(_) => match self.store.user_info() {
                Some(user_info) => Ok(Some(user_info.into())),
                None => {
                    let _ = self.store.clear_auth_data();
                    Ok(None)
                }
            },
            None => Ok(None),
        }
    }

    pub async fn check_username(
        &self,
        param: FrontendCheckUsernameRequest,
    ) -> Result<bool, String> {
        let api_param: CheckUsernameRequestParam = param.into();
        match api_check_username(&api_param).await {
            Ok(response) => response.ok_or_else(|| "用户名不可用".to_string()),
            Err(error) => Err(error.to_string()),
        }
    }

    pub async fn update_user_info(&self, nickname: String) -> Result<bool, String> {
        let nickname = nickname.trim();
        if nickname.is_empty() {
            return Err("昵称不能为空".to_string());
        }
        if nickname.len() > 20 {
            return Err("昵称长度不能超过20个字符".to_string());
        }

        let param = UpdateUserInfoParam {
            nick_name: nickname.to_string(),
        };

        match api_update_user_info(&param).await {
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

    fn emit(&self, event: &str) {
        if let Some(app_handle) = self.context.try_app_handle() {
            if let Err(error) = app_handle.emit(event, ()) {
                log::error!("发送 {} 事件失败: {}", event, error);
            }
        }
    }
}

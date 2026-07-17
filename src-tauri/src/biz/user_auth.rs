use serde::{Deserialize, Serialize};
use tauri::Emitter;

use crate::{
    api::user_auth_api::{
        check_username as api_check_username, send_email_code as api_send_email_code,
        update_user_info as api_update_user_info, user_login, user_logout as api_user_logout,
        user_register as api_user_register, AuthResponse, CheckUsernameRequestParam,
        EmailCodeRequestParam, LoginRequestParam, RegisterRequestParam, UpdateUserInfoParam,
        UserInfo as ApiUserInfo,
    },
    app_context::try_app_context,
    response::{string_result, CommandResponse},
    utils::secure_store::SECURE_STORE,
    utils::token_manager::has_valid_auth,
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
        LoginRequestParam {
            username: request.account,
            password: request.password,
        }
    }
}

impl From<FrontendRegisterRequest> for RegisterRequestParam {
    fn from(request: FrontendRegisterRequest) -> Self {
        RegisterRequestParam {
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
        EmailCodeRequestParam {
            email: request.email,
        }
    }
}

impl From<FrontendCheckUsernameRequest> for CheckUsernameRequestParam {
    fn from(request: FrontendCheckUsernameRequest) -> Self {
        CheckUsernameRequestParam {
            username: request.username,
        }
    }
}

impl From<ApiUserInfo> for UserInfo {
    fn from(api_user: ApiUserInfo) -> Self {
        UserInfo {
            id: api_user.id,
            account: api_user.username,
            nickname: api_user.nick_name,
            email: api_user.email,
            phone: api_user.phone,
        }
    }
}

#[allow(dead_code)]
pub async fn login(param: FrontendLoginRequest) -> Result<CommandResponse<LoginResponse>, String> {
    Ok(string_result(
        async {
            log::info!("用户登录请求: {}", param.account);

            let api_param: LoginRequestParam = param.into();
            match user_login(&api_param).await {
                Ok(response) => {
                    if let Some(auth_response) = response {
                        log::info!("用户登录成功: {}", auth_response.user_info.username);

                        if let Err(e) = store_auth_data(&auth_response).await {
                            log::error!("存储认证数据失败: {}", e);
                            return Err(format!("存储认证数据失败: {}", e));
                        }

                        tokio::spawn(async {
                            if let Err(e) =
                            crate::biz::vip_checker::VipChecker::initialize_vip_and_enforce_limits()
                                .await
                        {
                            log::error!("登录后 VIP 状态初始化失败: {}", e);
                        }
                        });

                        Ok(LoginResponse {
                            user_info: auth_response.user_info.into(),
                            token: auth_response.access_token,
                            expires_in: auth_response.expires_in,
                        })
                    } else {
                        Err("登录响应为空".to_string())
                    }
                }
                Err(e) => Err(e.to_string()),
            }
        }
        .await,
    ))
}

#[allow(dead_code)]
pub async fn user_register(
    param: FrontendRegisterRequest,
) -> Result<CommandResponse<UserInfo>, String> {
    Ok(string_result(
        async {
            log::info!("用户注册请求: {}", param.account);

            let api_param: RegisterRequestParam = param.into();
            match api_user_register(&api_param).await {
                Ok(response) => {
                    if let Some(user_info) = response {
                        Ok(user_info.into())
                    } else {
                        Err("注册响应为空".to_string())
                    }
                }
                Err(e) => Err(e.to_string()),
            }
        }
        .await,
    ))
}

#[allow(dead_code)]
pub async fn send_email_code(
    param: FrontendSendEmailCodeRequest,
) -> Result<CommandResponse<String>, String> {
    Ok(string_result(
        async {
            let api_param: EmailCodeRequestParam = param.into();
            match api_send_email_code(&api_param).await {
                Ok(response) => match response {
                    Some(true) => Ok("验证码已发送".to_string()),
                    Some(false) | None => Err("验证码发送失败".to_string()),
                },
                Err(e) => Err(e.to_string()),
            }
        }
        .await,
    ))
}

async fn store_auth_data(auth_response: &AuthResponse) -> Result<(), String> {
    let mut store = SECURE_STORE
        .write()
        .map_err(|e| format!("获取存储写锁失败: {}", e))?;

    store
        .set_jwt_token(auth_response.access_token.clone())
        .map_err(|e| format!("存储访问令牌失败: {}", e))?;
    store
        .set_refresh_token(auth_response.refresh_token.clone())
        .map_err(|e| format!("存储刷新令牌失败: {}", e))?;

    let user_info_json = serde_json::to_string(&auth_response.user_info)
        .map_err(|e| format!("序列化用户信息失败: {}", e))?;
    store
        .set_user_info(user_info_json)
        .map_err(|e| format!("存储用户信息失败: {}", e))?;
    store
        .set_token_expires(auth_response.expires_in.clone())
        .map_err(|e| format!("存储过期时间失败: {}", e))?;

    Ok(())
}

pub fn get_stored_access_token() -> Option<String> {
    match SECURE_STORE.write() {
        Ok(mut store) => store.get_jwt_token().ok().flatten(),
        Err(e) => {
            log::error!("获取访问令牌存储锁失败: {}", e);
            None
        }
    }
}

pub fn get_stored_user_info() -> Option<UserInfo> {
    match SECURE_STORE.write() {
        Ok(mut store) => match store.get_user_info() {
            Ok(Some(user_info_json)) => serde_json::from_str::<ApiUserInfo>(&user_info_json)
                .ok()
                .map(Into::into),
            _ => None,
        },
        Err(e) => {
            log::error!("获取用户信息存储锁失败: {}", e);
            None
        }
    }
}

pub fn clear_stored_auth_data() -> Result<(), String> {
    let mut store = SECURE_STORE
        .write()
        .map_err(|e| format!("获取存储写锁失败: {}", e))?;
    store
        .clear_auth_data()
        .map_err(|e| format!("清除认证数据失败: {}", e))?;
    Ok(())
}

#[allow(dead_code)]
pub async fn logout() -> Result<CommandResponse<String>, String> {
    Ok(string_result(
        async {
            if has_valid_auth() {
                let _ = api_user_logout().await;
            }

            clear_stored_auth_data()?;
            notify_auth_cleared().await;

            if let Err(e) = crate::biz::system_setting::disable_cloud_sync().await {
                log::error!("禁用云同步设置失败: {}", e);
            }
            notify_cloud_sync_disabled().await;

            Ok("退出成功".to_string())
        }
        .await,
    ))
}

#[allow(dead_code)]
pub async fn validate_token() -> Result<CommandResponse<bool>, String> {
    Ok(string_result(
        async { Ok(get_stored_access_token().is_some()) }.await,
    ))
}

#[allow(dead_code)]
pub async fn get_user_info() -> Result<CommandResponse<UserInfo>, String> {
    Ok(string_result(
        async { get_stored_user_info().ok_or("用户未登录".to_string()) }.await,
    ))
}

#[allow(dead_code)]
pub async fn check_login_status() -> Result<CommandResponse<Option<UserInfo>>, String> {
    Ok(string_result(
        async {
            match get_stored_access_token() {
                Some(_) => match get_stored_user_info() {
                    Some(user_info) => Ok(Some(user_info)),
                    None => {
                        let _ = clear_stored_auth_data();
                        Ok(None)
                    }
                },
                None => Ok(None),
            }
        }
        .await,
    ))
}

#[allow(dead_code)]
pub async fn check_username(
    param: FrontendCheckUsernameRequest,
) -> Result<CommandResponse<bool>, String> {
    Ok(string_result(
        async {
            let api_param: CheckUsernameRequestParam = param.into();
            match api_check_username(&api_param).await {
                Ok(response) => response.ok_or("用户名不可用".to_string()),
                Err(e) => Err(e.to_string()),
            }
        }
        .await,
    ))
}

async fn notify_auth_cleared() {
    if let Some(app_handle) = try_app_context().and_then(|context| context.try_app_handle()) {
        if let Err(e) = app_handle.emit("auth-cleared", ()) {
            log::error!("发送认证清除事件失败: {}", e);
        }
    }
}

async fn notify_cloud_sync_disabled() {
    if let Some(app_handle) = try_app_context().and_then(|context| context.try_app_handle()) {
        if let Err(e) = app_handle.emit("cloud-sync-disabled", ()) {
            log::error!("发送云同步禁用事件失败: {}", e);
        }
    }
}

#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateNicknameRequest {
    pub nick_name: String,
}

#[allow(dead_code)]
pub async fn update_user_info(nick_name: String) -> Result<CommandResponse<bool>, String> {
    Ok(string_result(
        async {
            let trimmed_nickname = nick_name.trim();
            if trimmed_nickname.is_empty() {
                return Err("昵称不能为空".to_string());
            }
            if trimmed_nickname.len() > 20 {
                return Err("昵称长度不能超过20个字符".to_string());
            }

            let update_param = UpdateUserInfoParam {
                nick_name: trimmed_nickname.to_string(),
            };

            match api_update_user_info(&update_param).await {
                Ok(response) => {
                    if let Some(true) = response {
                        if let Err(e) = update_local_user_nickname(trimmed_nickname).await {
                            log::warn!("更新本地用户信息失败: {}", e);
                        }
                        Ok(true)
                    } else {
                        Err("昵称更新失败".to_string())
                    }
                }
                Err(e) => Err(e.to_string()),
            }
        }
        .await,
    ))
}

async fn update_local_user_nickname(new_nickname: &str) -> Result<(), String> {
    let mut store = SECURE_STORE
        .write()
        .map_err(|e| format!("获取存储写锁失败: {}", e))?;

    let user_info_json = store
        .get_user_info()
        .map_err(|e| format!("获取用户信息失败: {}", e))?;

    if let Some(json_str) = user_info_json {
        let mut api_user_info: ApiUserInfo =
            serde_json::from_str(&json_str).map_err(|e| format!("反序列化用户信息失败: {}", e))?;

        api_user_info.nick_name = Some(new_nickname.to_string());
        let updated_json = serde_json::to_string(&api_user_info)
            .map_err(|e| format!("序列化更新后的用户信息失败: {}", e))?;

        store
            .set_user_info(updated_json)
            .map_err(|e| format!("存储更新后的用户信息失败: {}", e))?;
    }

    Ok(())
}

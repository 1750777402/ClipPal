use serde::{Deserialize, Serialize};

use crate::{
    api::user_auth_api::{
        check_username as api_check_username, send_email_code as api_send_email_code,
        update_user_info as api_update_user_info, user_login, user_logout as api_user_logout,
        user_register as api_user_register, AuthResponse, CheckUsernameRequestParam,
        EmailCodeRequestParam, LoginRequestParam, RegisterRequestParam, UpdateUserInfoParam,
        UserInfo as ApiUserInfo,
    },
    utils::secure_store::SECURE_STORE,
    utils::token_manager::has_valid_auth,
    CONTEXT,
};
use tauri::Emitter;

// 前端需要的用户信息结构
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserInfo {
    pub id: u64,
    pub account: String,
    pub nickname: Option<String>,
    pub email: Option<String>,
    pub phone: Option<String>,
}

// 前端登录响应结构
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoginResponse {
    pub user_info: UserInfo,
    pub token: String,
    pub expires_in: i32,
}

// 前端登录请求结构（适配API参数）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FrontendLoginRequest {
    pub account: String,
    pub password: String,
}

// 前端注册请求结构（适配API参数）
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

// 前端发送验证码请求结构
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FrontendSendEmailCodeRequest {
    pub email: String,
}

// 前端检查用户名请求结构
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

#[tauri::command]
pub async fn login(param: FrontendLoginRequest) -> Result<LoginResponse, String> {
    log::info!("用户登录请求: {}", param.account);

    // 转换为API请求参数
    let api_param: LoginRequestParam = param.into();

    let login_res = user_login(&api_param).await;
    match login_res {
        Ok(response) => {
            if let Some(auth_response) = response {
                log::info!("用户登录成功: {}", auth_response.user_info.username);

                // 存储token到加密文件
                if let Err(e) = store_auth_data(&auth_response).await {
                    log::error!("存储认证数据失败: {}", e);
                    return Err(format!("存储认证数据失败: {}", e));
                }

                // 登录成功后，触发VIP状态检查
                tokio::spawn(async {
                    log::info!("用户登录成功，触发VIP状态检查");
                    if let Err(e) =
                        crate::biz::vip_checker::VipChecker::initialize_vip_and_enforce_limits()
                            .await
                    {
                        log::error!("登录后VIP状态初始化失败: {}", e);
                    }
                });

                // 构造前端响应
                let login_response = LoginResponse {
                    user_info: auth_response.user_info.into(),
                    token: auth_response.access_token,
                    expires_in: auth_response.expires_in,
                };

                log::info!("用户登录完成: {}", login_response.user_info.account);
                Ok(login_response)
            } else {
                log::warn!("登录响应为空");
                Err("登录响应为空".to_string())
            }
        }
        Err(e) => {
            log::error!("用户登录失败: {}", e);
            // 直接返回服务器的错误信息，不再添加额外包装
            Err(e.to_string())
        }
    }
}

#[tauri::command]
pub async fn user_register(param: FrontendRegisterRequest) -> Result<UserInfo, String> {
    log::info!("用户注册请求: {}", param.account);

    // 转换为API请求参数
    let api_param: RegisterRequestParam = param.into();

    let register_res = api_user_register(&api_param).await;
    match register_res {
        Ok(response) => {
            if let Some(user_info) = response {
                log::info!("用户注册成功: {}", user_info.username);

                // 转换为前端用户信息结构
                let frontend_user_info: UserInfo = user_info.into();

                log::info!("用户注册完成: {}", frontend_user_info.account);
                Ok(frontend_user_info)
            } else {
                log::warn!("注册响应为空");
                Err("注册响应为空".to_string())
            }
        }
        Err(e) => {
            log::error!("用户注册失败: {}", e);
            // 直接返回服务器的错误信息，不再添加额外包装
            Err(e.to_string())
        }
    }
}

#[tauri::command]
pub async fn send_email_code(param: FrontendSendEmailCodeRequest) -> Result<String, String> {
    // 转换为API请求参数
    let api_param: EmailCodeRequestParam = param.into();

    let send_res = api_send_email_code(&api_param).await;
    match send_res {
        Ok(response) => {
            if let Some(success_flag) = response {
                if success_flag {
                    log::info!("验证码发送成功");
                    Ok("验证码已发送".to_string())
                } else {
                    log::warn!("验证码发送失败: 服务器返回 false");
                    Err("验证码发送失败".to_string())
                }
            } else {
                log::warn!("验证码发送失败: 服务器返回空数据");
                Err("验证码发送失败".to_string())
            }
        }
        Err(e) => {
            log::error!("发送验证码失败: {}", e);
            // 直接返回服务器的错误信息，不再添加额外包装
            Err(e.to_string())
        }
    }
}

/// 存储认证数据到加密文件
async fn store_auth_data(auth_response: &AuthResponse) -> Result<(), String> {
    // 获取写锁并存储所有认证数据
    let mut store = SECURE_STORE
        .write()
        .map_err(|e| format!("获取存储写锁失败: {}", e))?;

    // 存储访问令牌
    store
        .set_jwt_token(auth_response.access_token.clone())
        .map_err(|e| format!("存储访问令牌失败: {}", e))?;

    // 存储刷新令牌
    store
        .set_refresh_token(auth_response.refresh_token.clone())
        .map_err(|e| format!("存储刷新令牌失败: {}", e))?;

    // 存储用户信息
    let user_info_json = serde_json::to_string(&auth_response.user_info)
        .map_err(|e| format!("序列化用户信息失败: {}", e))?;
    store
        .set_user_info(user_info_json)
        .map_err(|e| format!("存储用户信息失败: {}", e))?;

    // 存储过期时间
    store
        .set_token_expires(auth_response.expires_in.clone())
        .map_err(|e| format!("存储过期时间失败: {}", e))?;

    log::info!("认证数据已安全存储");
    Ok(())
}

/// 获取存储的访问令牌
pub fn get_stored_access_token() -> Option<String> {
    match SECURE_STORE.write() {
        Ok(mut store) => match store.get_jwt_token() {
            Ok(token) => token,
            Err(e) => {
                log::debug!("获取访问令牌失败: {}", e);
                None
            }
        },
        Err(e) => {
            log::error!("获取访问令牌存储写锁失败: {}", e);
            None
        }
    }
}

/// 获取存储的用户信息
pub fn get_stored_user_info() -> Option<UserInfo> {
    match SECURE_STORE.write() {
        Ok(mut store) => match store.get_user_info() {
            Ok(Some(user_info_json)) => {
                match serde_json::from_str::<ApiUserInfo>(&user_info_json) {
                    Ok(api_user_info) => Some(api_user_info.into()),
                    Err(e) => {
                        log::error!("用户信息反序列化失败: {}", e);
                        None
                    }
                }
            }
            Ok(None) => None,
            Err(e) => {
                log::error!("获取用户信息失败: {}", e);
                None
            }
        },
        Err(e) => {
            log::error!("获取用户信息存储写锁失败: {}", e);
            None
        }
    }
}

/// 清除所有存储的认证数据
pub fn clear_stored_auth_data() -> Result<(), String> {
    let mut store = SECURE_STORE
        .write()
        .map_err(|e| format!("获取存储写锁失败: {}", e))?;

    store
        .clear_auth_data()
        .map_err(|e| format!("清除认证数据失败: {}", e))?;

    log::info!("认证数据已清除");
    Ok(())
}

/// 用户登出
#[tauri::command]
pub async fn logout() -> Result<String, String> {
    log::info!("用户登出请求");

    // 如果有有效的认证状态，先调用后端退出登录接口
    if has_valid_auth() {
        match api_user_logout().await {
            Ok(_) => {
                log::info!("后端退出登录成功");
            }
            Err(e) => {
                log::warn!("后端退出登录失败，继续清除本地数据: {}", e);
                // 不返回错误，继续清除本地数据
            }
        }
    }

    // 清除本地存储的认证数据
    if let Err(e) = clear_stored_auth_data() {
        log::error!("清除本地认证数据失败: {}", e);
        return Err(e);
    }

    // 通知前端登录状态已清除
    notify_auth_cleared().await;

    // 实际禁用云同步设置
    if let Err(e) = crate::biz::system_setting::disable_cloud_sync().await {
        log::error!("禁用云同步设置失败: {}", e);
    }

    // 通知前端云同步功能状态已更新
    notify_cloud_sync_disabled().await;

    log::info!("用户登出完成");
    Ok("登出成功".to_string())
}

/// 验证当前Token是否有效
#[tauri::command]
pub async fn validate_token() -> Result<bool, String> {
    match get_stored_access_token() {
        Some(_token) => {
            log::debug!("找到存储的token，验证有效性");
            // 这里可以添加token有效性验证逻辑
            // 比如检查过期时间或者向服务器验证
            Ok(true)
        }
        None => {
            log::debug!("未找到存储的token");
            Ok(false)
        }
    }
}

/// 获取当前用户信息
#[tauri::command]
pub async fn get_user_info() -> Result<UserInfo, String> {
    match get_stored_user_info() {
        Some(user_info) => {
            log::debug!("获取用户信息成功: {}", user_info.account);
            Ok(user_info)
        }
        None => {
            log::debug!("未找到用户信息");
            Err("用户未登录".to_string())
        }
    }
}

/// 检查是否有有效的登录状态（应用启动时调用）
#[tauri::command]
pub async fn check_login_status() -> Result<Option<UserInfo>, String> {
    match get_stored_access_token() {
        Some(_token) => {
            // 有token，尝试获取用户信息
            match get_stored_user_info() {
                Some(user_info) => {
                    log::info!("应用启动时检测到用户登录状态: {}", user_info.account);
                    Ok(Some(user_info))
                }
                None => {
                    log::warn!("发现无效认证数据，清除本地缓存");
                    let _ = clear_stored_auth_data();
                    Ok(None)
                }
            }
        }
        None => {
            log::debug!("应用启动时未检测到登录状态");
            Ok(None)
        }
    }
}

#[tauri::command]
pub async fn check_username(param: FrontendCheckUsernameRequest) -> Result<bool, String> {
    // 转换为API请求参数
    let api_param: CheckUsernameRequestParam = param.into();

    let check_res = api_check_username(&api_param).await;
    match check_res {
        Ok(response) => {
            if let Some(is_available) = response {
                Ok(is_available)
            } else {
                log::warn!("用户名检查响应为空");
                Err("用户名不可用".to_string())
            }
        }
        Err(e) => {
            log::error!("检查用户名失败: {}", e);
            // 直接返回服务器的错误信息，不再添加额外包装
            Err(e.to_string())
        }
    }
}

/// 通知前端认证状态已清除
async fn notify_auth_cleared() {
    log::info!("通知前端认证状态已清除");

    // 通过Tauri事件系统通知前端
    if let Some(app_handle) = CONTEXT.try_get::<tauri::AppHandle>() {
        if let Err(e) = app_handle.emit("auth-cleared", ()) {
            log::error!("发送认证清除事件失败: {}", e);
        }
    }
}

/// 通知前端云同步已被禁用
async fn notify_cloud_sync_disabled() {
    log::info!("通知前端云同步已被禁用");

    // 通过Tauri事件系统通知前端
    if let Some(app_handle) = CONTEXT.try_get::<tauri::AppHandle>() {
        if let Err(e) = app_handle.emit("cloud-sync-disabled", ()) {
            log::error!("发送云同步禁用事件失败: {}", e);
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateNicknameRequest {
    pub nick_name: String,
}

/// 更新用户昵称
#[tauri::command]
pub async fn update_user_info(nick_name: String) -> Result<bool, String> {
    log::info!("更新用户昵称请求: {}", nick_name);

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
            if let Some(success) = response {
                if success {
                    log::info!("昵称更新成功");

                    // 更新本地存储的用户信息
                    if let Err(e) = update_local_user_nickname(trimmed_nickname).await {
                        log::warn!("更新本地用户信息失败: {}", e);
                    }

                    Ok(true)
                } else {
                    log::warn!("昵称更新失败: 服务器返回false");
                    Err("昵称更新失败".to_string())
                }
            } else {
                log::warn!("昵称更新失败: 服务器返回空响应");
                Err("昵称更新失败".to_string())
            }
        }
        Err(e) => {
            log::error!("昵称更新失败: {}", e);
            Err(e.to_string())
        }
    }
}

/// 更新本地存储的用户昵称
async fn update_local_user_nickname(new_nickname: &str) -> Result<(), String> {
    let mut store = SECURE_STORE
        .write()
        .map_err(|e| format!("获取存储写锁失败: {}", e))?;

    // 获取现有用户信息
    let user_info_json = store
        .get_user_info()
        .map_err(|e| format!("获取用户信息失败: {}", e))?;

    if let Some(json_str) = user_info_json {
        let mut api_user_info: ApiUserInfo =
            serde_json::from_str(&json_str).map_err(|e| format!("用户信息反序列化失败: {}", e))?;

        // 更新昵称
        api_user_info.nick_name = Some(new_nickname.to_string());

        // 重新序列化并存储
        let updated_json = serde_json::to_string(&api_user_info)
            .map_err(|e| format!("序列化更新后的用户信息失败: {}", e))?;

        store
            .set_user_info(updated_json)
            .map_err(|e| format!("存储更新后的用户信息失败: {}", e))?;

        log::info!("本地用户信息昵称已更新");
    }

    Ok(())
}

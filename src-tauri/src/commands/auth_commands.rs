use std::sync::Arc;

use crate::{
    app_context::AppContext,
    domain::user::{
        FrontendCheckUsernameRequest, FrontendLoginRequest, FrontendRegisterRequest,
        FrontendSendEmailCodeRequest, LoginResponse, UserInfo,
    },
    response::{string_result, CommandResponse},
    services::auth_service::AuthService,
};

#[tauri::command]
/// 使用账号和密码登录，返回用户信息、访问令牌及过期时间。
pub async fn login(
    state: tauri::State<'_, Arc<AppContext>>,
    param: FrontendLoginRequest,
) -> Result<CommandResponse<LoginResponse>, String> {
    let service = AuthService::from_context(state.inner().as_ref());
    Ok(string_result(service.login(param).await))
}

#[tauri::command]
/// 注册新用户，将前端注册参数交给认证服务完成服务端请求。
pub async fn user_register(
    state: tauri::State<'_, Arc<AppContext>>,
    param: FrontendRegisterRequest,
) -> Result<CommandResponse<UserInfo>, String> {
    let service = AuthService::from_context(state.inner().as_ref());
    Ok(string_result(service.register(param).await))
}

#[tauri::command]
/// 向指定邮箱发送注册验证码，并返回可展示的业务结果。
pub async fn send_email_code(
    state: tauri::State<'_, Arc<AppContext>>,
    param: FrontendSendEmailCodeRequest,
) -> Result<CommandResponse<String>, String> {
    let service = AuthService::from_context(state.inner().as_ref());
    Ok(string_result(service.send_email_code(param).await))
}

#[tauri::command]
/// 退出当前账号，清理本地认证状态并关闭云同步。
pub async fn logout(
    state: tauri::State<'_, Arc<AppContext>>,
) -> Result<CommandResponse<String>, String> {
    let service = AuthService::from_context(state.inner().as_ref());
    Ok(string_result(service.logout().await))
}

#[tauri::command]
/// 检查本地是否存在有效认证令牌，供前端恢复登录状态。
pub async fn validate_token(
    state: tauri::State<'_, Arc<AppContext>>,
) -> Result<CommandResponse<bool>, String> {
    let service = AuthService::from_context(state.inner().as_ref());
    Ok(string_result(service.validate_token()))
}

#[tauri::command]
/// 读取当前登录用户信息；未登录时返回统一业务错误。
pub async fn get_user_info(
    state: tauri::State<'_, Arc<AppContext>>,
) -> Result<CommandResponse<UserInfo>, String> {
    let service = AuthService::from_context(state.inner().as_ref());
    Ok(string_result(service.get_user_info()))
}

#[tauri::command]
/// 检查完整登录状态，允许以 `None` 表达未登录而不是抛出错误。
pub async fn check_login_status(
    state: tauri::State<'_, Arc<AppContext>>,
) -> Result<CommandResponse<Option<UserInfo>>, String> {
    let service = AuthService::from_context(state.inner().as_ref());
    Ok(string_result(service.check_login_status()))
}

#[tauri::command]
/// 请求服务端检查用户名是否可用。
pub async fn check_username(
    state: tauri::State<'_, Arc<AppContext>>,
    param: FrontendCheckUsernameRequest,
) -> Result<CommandResponse<bool>, String> {
    let service = AuthService::from_context(state.inner().as_ref());
    Ok(string_result(service.check_username(param).await))
}

#[tauri::command]
/// 更新当前用户昵称，并同步更新本地安全存储中的用户信息。
pub async fn update_user_info(
    state: tauri::State<'_, Arc<AppContext>>,
    nick_name: String,
) -> Result<CommandResponse<bool>, String> {
    let service = AuthService::from_context(state.inner().as_ref());
    Ok(string_result(service.update_user_info(nick_name).await))
}

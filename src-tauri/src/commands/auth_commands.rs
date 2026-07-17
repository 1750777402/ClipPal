use std::sync::Arc;

use crate::{
    app_context::AppContext,
    dto::auth_dto::{
        FrontendCheckUsernameRequest, FrontendLoginRequest, FrontendRegisterRequest,
        FrontendSendEmailCodeRequest, LoginResponse, UserInfo,
    },
    response::{string_result, CommandResponse},
    services::auth_service::AuthService,
};

#[tauri::command]
pub async fn login(
    state: tauri::State<'_, Arc<AppContext>>,
    param: FrontendLoginRequest,
) -> Result<CommandResponse<LoginResponse>, String> {
    let service = AuthService::from_context(state.inner().as_ref());
    Ok(string_result(service.login(param).await))
}

#[tauri::command]
pub async fn user_register(
    state: tauri::State<'_, Arc<AppContext>>,
    param: FrontendRegisterRequest,
) -> Result<CommandResponse<UserInfo>, String> {
    let service = AuthService::from_context(state.inner().as_ref());
    Ok(string_result(service.register(param).await))
}

#[tauri::command]
pub async fn send_email_code(
    state: tauri::State<'_, Arc<AppContext>>,
    param: FrontendSendEmailCodeRequest,
) -> Result<CommandResponse<String>, String> {
    let service = AuthService::from_context(state.inner().as_ref());
    Ok(string_result(service.send_email_code(param).await))
}

#[tauri::command]
pub async fn logout(
    state: tauri::State<'_, Arc<AppContext>>,
) -> Result<CommandResponse<String>, String> {
    let service = AuthService::from_context(state.inner().as_ref());
    Ok(string_result(service.logout().await))
}

#[tauri::command]
pub async fn validate_token(
    state: tauri::State<'_, Arc<AppContext>>,
) -> Result<CommandResponse<bool>, String> {
    let service = AuthService::from_context(state.inner().as_ref());
    Ok(string_result(service.validate_token()))
}

#[tauri::command]
pub async fn get_user_info(
    state: tauri::State<'_, Arc<AppContext>>,
) -> Result<CommandResponse<UserInfo>, String> {
    let service = AuthService::from_context(state.inner().as_ref());
    Ok(string_result(service.get_user_info()))
}

#[tauri::command]
pub async fn check_login_status(
    state: tauri::State<'_, Arc<AppContext>>,
) -> Result<CommandResponse<Option<UserInfo>>, String> {
    let service = AuthService::from_context(state.inner().as_ref());
    Ok(string_result(service.check_login_status()))
}

#[tauri::command]
pub async fn check_username(
    state: tauri::State<'_, Arc<AppContext>>,
    param: FrontendCheckUsernameRequest,
) -> Result<CommandResponse<bool>, String> {
    let service = AuthService::from_context(state.inner().as_ref());
    Ok(string_result(service.check_username(param).await))
}

#[tauri::command]
pub async fn update_user_info(
    state: tauri::State<'_, Arc<AppContext>>,
    nick_name: String,
) -> Result<CommandResponse<bool>, String> {
    let service = AuthService::from_context(state.inner().as_ref());
    Ok(string_result(service.update_user_info(nick_name).await))
}

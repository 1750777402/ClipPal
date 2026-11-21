use std::sync::Arc;

use {
    crate::app_context::AppContext,
    clip_core::domain::settings::Settings,
    crate::response::CommandResponse,
    crate::core::application::SettingsService,
};

#[tauri::command]
/// 从设置仓储加载当前配置并返回给前端。
pub async fn load_settings(state: tauri::State<'_, Arc<AppContext>>) -> Result<CommandResponse<crate::core::application::settings_service::SettingsView>, String> {
    let service = SettingsService::from_context(state.inner().as_ref());
    Ok(crate::response::command_result(service.load_settings().await))
}

#[tauri::command]
/// 校验、应用并持久化前端提交的完整设置。
pub async fn save_settings(
    state: tauri::State<'_, Arc<AppContext>>,
    settings: Settings,
) -> Result<CommandResponse<()>, String> {
    let service = SettingsService::from_context(state.inner().as_ref());
    Ok(crate::response::command_result(service.save_settings(settings).await))
}

#[tauri::command]
/// 校验快捷键格式和系统占用，临时注册后释放，不修改当前设置。
pub async fn validate_shortcut(
    state: tauri::State<'_, Arc<AppContext>>,
    shortcut: String,
) -> Result<CommandResponse<bool>, String> {
    let service = SettingsService::from_context(state.inner().as_ref());
    Ok(crate::response::command_result(service.validate_shortcut(shortcut).await))
}

/// 辅助功能权限为系统状态；不能由客户端配置或前端缓存声明已授权。
#[tauri::command]
pub fn get_auto_paste_permission() -> CommandResponse<crate::desktop::system::accessibility::AutoPastePermission> {
    crate::response::ok(crate::desktop::system::accessibility::status())
}

#[tauri::command]
pub async fn request_auto_paste_permission(
    state: tauri::State<'_, Arc<AppContext>>,
) -> Result<CommandResponse<crate::desktop::system::accessibility::AutoPastePermission>, String> {
    Ok(crate::response::command_result(crate::desktop::system::accessibility::request(state.inner()).await))
}

#[tauri::command]
pub fn open_accessibility_settings(state: tauri::State<'_, Arc<AppContext>>) -> CommandResponse<()> {
    crate::response::command_result(crate::desktop::system::accessibility::open_settings(state.inner()))
}

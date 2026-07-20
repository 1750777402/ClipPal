use std::sync::Arc;

use crate::{
    app_context::AppContext,
    domain::settings::Settings,
    response::{ok, string_result, CommandResponse},
    services::settings_service::SettingsService,
};

#[tauri::command]
/// 从设置仓储加载当前配置并返回给前端。
pub fn load_settings(state: tauri::State<'_, Arc<AppContext>>) -> CommandResponse<Settings> {
    let service = SettingsService::from_context(state.inner().as_ref());
    ok(service.load_settings())
}

#[tauri::command]
/// 校验、应用并持久化前端提交的完整设置。
pub async fn save_settings(
    state: tauri::State<'_, Arc<AppContext>>,
    settings: Settings,
) -> Result<CommandResponse<()>, String> {
    let service = SettingsService::from_context(state.inner().as_ref());
    Ok(string_result(service.save_settings(settings).await))
}

#[tauri::command]
/// 校验快捷键格式以及系统快捷键解析结果，不修改当前设置。
pub async fn validate_shortcut(
    state: tauri::State<'_, Arc<AppContext>>,
    shortcut: String,
) -> Result<CommandResponse<bool>, String> {
    let service = SettingsService::from_context(state.inner().as_ref());
    Ok(string_result(service.validate_shortcut(shortcut)))
}

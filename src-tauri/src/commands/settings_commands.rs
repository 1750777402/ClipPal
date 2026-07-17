use std::sync::Arc;

use crate::{
    app_context::AppContext,
    biz::system_setting::Settings,
    response::{ok, string_result, CommandResponse},
    services::settings_service::SettingsService,
};

#[tauri::command]
pub fn load_settings(state: tauri::State<'_, Arc<AppContext>>) -> CommandResponse<Settings> {
    let service = SettingsService::from_context(state.inner().as_ref());
    ok(service.load_settings())
}

#[tauri::command]
pub async fn save_settings(
    state: tauri::State<'_, Arc<AppContext>>,
    settings: Settings,
) -> Result<CommandResponse<()>, String> {
    let service = SettingsService::from_context(state.inner().as_ref());
    Ok(string_result(service.save_settings(settings).await))
}

#[tauri::command]
pub async fn validate_shortcut(
    state: tauri::State<'_, Arc<AppContext>>,
    shortcut: String,
) -> Result<CommandResponse<bool>, String> {
    let service = SettingsService::from_context(state.inner().as_ref());
    Ok(string_result(service.validate_shortcut(shortcut)))
}

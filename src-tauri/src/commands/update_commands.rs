use std::sync::Arc;

use crate::{
    app_context::AppContext,
    response::{string_result, CommandResponse},
    services::update_service::UpdateService,
    updater::UpdateInfo,
};

#[tauri::command]
pub async fn check_soft_version(
    state: tauri::State<'_, Arc<AppContext>>,
) -> Result<CommandResponse<UpdateInfo>, String> {
    let service = UpdateService::from_context(state.inner().as_ref());
    Ok(string_result(service.check_soft_version().await))
}

#[tauri::command]
pub async fn download_and_install_update(
    state: tauri::State<'_, Arc<AppContext>>,
) -> Result<CommandResponse<bool>, String> {
    let service = UpdateService::from_context(state.inner().as_ref());
    Ok(string_result(service.download_and_install_update().await))
}

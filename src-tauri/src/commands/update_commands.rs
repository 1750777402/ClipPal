use std::sync::Arc;

use crate::{
    app_context::AppContext,
    domain::update::UpdateInfo,
    response::{string_result, CommandResponse},
    services::update_service::UpdateService,
};

#[tauri::command]
/// 检查软件更新，返回当前版本、最新版本和更新说明。
pub async fn check_soft_version(
    state: tauri::State<'_, Arc<AppContext>>,
) -> Result<CommandResponse<UpdateInfo>, String> {
    let service = UpdateService::from_context(state.inner().as_ref());
    Ok(string_result(service.check_soft_version().await))
}

#[tauri::command]
/// 下载并安装当前可用更新，返回安装流程是否成功完成。
pub async fn download_and_install_update(
    state: tauri::State<'_, Arc<AppContext>>,
) -> Result<CommandResponse<bool>, String> {
    let service = UpdateService::from_context(state.inner().as_ref());
    Ok(string_result(service.download_and_install_update().await))
}

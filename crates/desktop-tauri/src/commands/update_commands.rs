use std::sync::Arc;

use {
    crate::app_context::AppContext,
    desktop_app_core::update::{UpdateInfo, UpdateProgress},
    crate::response::string_result,
    crate::response::{CommandResponse, ErrorSeverity},
    crate::core::application::update_service::UpdateService,
};

#[tauri::command]
/// 检查软件更新，返回当前版本、最新版本和更新说明。
pub async fn check_soft_version(
    state: tauri::State<'_, Arc<AppContext>>,
) -> Result<CommandResponse<UpdateInfo>, String> {
    let service = UpdateService::from_context(state.inner().as_ref());
    Ok(update_response(service.check_soft_version().await))
}

#[tauri::command]
/// 安装用户确认的版本，进度只发送给当前调用的窗口。
pub async fn download_and_install_update(
    state: tauri::State<'_, Arc<AppContext>>,
    version: String,
    on_progress: tauri::ipc::Channel<UpdateProgress>,
) -> Result<CommandResponse<()>, String> {
    let service = UpdateService::from_context(state.inner().as_ref());
    Ok(update_response(service.download_and_install_update(&version, |progress| {
        // 窗口关闭时丢弃后续进度，安装流程仍以签名和系统结果为准。
        let _ = on_progress.send(progress);
    }).await))
}

/// 更新弹窗负责展示失败原因，启动静默检查无需另外弹出全局消息。
fn update_response<T: serde::Serialize>(result: Result<T, String>) -> CommandResponse<T> {
    let mut response = string_result(result);
    if let Some(error) = &mut response.error {
        error.severity = ErrorSeverity::Silent;
    }
    response
}

//! 自建同步的桌面接口。输入输出复用应用统一响应结构。
use crate::{app_context::AppContext, response::{command_result, CommandResponse}, sync::runtime::{self, SyncSettingsStatus}};
use selfhost_app_core::SyncCardStates;
use std::sync::Arc;

/// 读取连接及队列处理状态，不向页面暴露地址、令牌或内部错误。
#[tauri::command]
pub async fn get_sync_status(state: tauri::State<'_, Arc<AppContext>>) -> Result<CommandResponse<SyncSettingsStatus>, String> {
    Ok(command_result(runtime::settings_status(state.inner()).await))
}

/// 保存本机云同步开关，并等待本次启停操作完成。
#[tauri::command]
pub async fn set_sync_enabled(
    state: tauri::State<'_, Arc<AppContext>>, enabled: bool,
) -> Result<CommandResponse<SyncSettingsStatus>, String> {
    Ok(command_result(runtime::set_enabled(state.inner(), enabled).await))
}

/// 主列表批量读取可见记录的状态和正在下载的附件。
#[tauri::command]
pub async fn get_sync_card_states(
    state: tauri::State<'_, Arc<AppContext>>, content_keys: Vec<String>,
) -> Result<CommandResponse<SyncCardStates>, String> {
    Ok(command_result(state.sync().card_states(content_keys).await))
}

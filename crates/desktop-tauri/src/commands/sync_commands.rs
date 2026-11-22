//! 自建同步的桌面接口。输入输出复用应用统一响应结构。
use crate::{app_context::AppContext, response::{command_result, ok, CommandResponse}};
use desktop_app_core::dto::ClipRecordIdParam;
use selfhost_app_core::SyncStatus;
use std::sync::Arc;

/// 读取连接及队列处理状态，不向页面暴露地址、令牌或内部错误。
#[tauri::command]
pub async fn get_sync_status(state: tauri::State<'_, Arc<AppContext>>) -> Result<CommandResponse<SyncStatus>, String> {
    Ok(ok(state.sync().status().await))
}

/// 显式在本地删除并登记跨端删除。离线时提交本机事务即返回成功。
#[tauri::command]
pub async fn delete_synced_record(
    state: tauri::State<'_, Arc<AppContext>>,
    param: ClipRecordIdParam,
) -> Result<CommandResponse<String>, String> {
    Ok(command_result(state.sync().delete_everywhere(param.record_id).await))
}

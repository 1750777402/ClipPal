use std::sync::Arc;

use crate::{
    app_context::AppContext,
    dto::clip_dto::{ClipRecordIdParam, CopySingleFileParam},
    response::{string_result, CommandResponse},
    services::clipboard_service::ClipboardService,
};

#[tauri::command]
/// 将指定记录写入系统剪贴板，并根据设置决定是否执行自动粘贴。
pub async fn copy_clip_record(
    state: tauri::State<'_, Arc<AppContext>>,
    param: ClipRecordIdParam,
) -> Result<CommandResponse<String>, String> {
    let service = ClipboardService::from_context(state.inner().as_ref());
    Ok(string_result(
        service.copy_record(param.record_id, true).await,
    ))
}

#[tauri::command]
/// 将指定记录写入系统剪贴板，但明确禁止触发自动粘贴。
pub async fn copy_clip_record_no_paste(
    state: tauri::State<'_, Arc<AppContext>>,
    param: ClipRecordIdParam,
) -> Result<CommandResponse<String>, String> {
    let service = ClipboardService::from_context(state.inner().as_ref());
    Ok(string_result(
        service.copy_record(param.record_id, false).await,
    ))
}

#[tauri::command]
/// 从文件类型记录中选择一个文件写入系统剪贴板。
pub async fn copy_single_file(
    state: tauri::State<'_, Arc<AppContext>>,
    param: CopySingleFileParam,
) -> Result<CommandResponse<String>, String> {
    let service = ClipboardService::from_context(state.inner().as_ref());
    Ok(string_result(service.copy_single_file(param).await))
}

#[tauri::command]
/// 打开系统保存对话框，将指定图片记录另存到用户选择的位置。
pub async fn image_save_as(
    state: tauri::State<'_, Arc<AppContext>>,
    param: ClipRecordIdParam,
) -> Result<CommandResponse<String>, String> {
    let service = ClipboardService::from_context(state.inner().as_ref());
    Ok(string_result(service.image_save_as(param.record_id).await))
}

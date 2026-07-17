use std::sync::Arc;

use crate::{
    app_context::AppContext,
    dto::clip_dto::{ClipRecordIdParam, CopySingleFileParam},
    response::{string_result, CommandResponse},
    services::clipboard_service::ClipboardService,
};

#[tauri::command]
pub async fn copy_clip_record(
    state: tauri::State<'_, Arc<AppContext>>,
    param: ClipRecordIdParam,
) -> Result<CommandResponse<String>, String> {
    let service = ClipboardService::new(state.inner().as_ref());
    Ok(string_result(
        service.copy_record(param.record_id, true).await,
    ))
}

#[tauri::command]
pub async fn copy_clip_record_no_paste(
    state: tauri::State<'_, Arc<AppContext>>,
    param: ClipRecordIdParam,
) -> Result<CommandResponse<String>, String> {
    let service = ClipboardService::new(state.inner().as_ref());
    Ok(string_result(
        service.copy_record(param.record_id, false).await,
    ))
}

#[tauri::command]
pub async fn copy_single_file(
    state: tauri::State<'_, Arc<AppContext>>,
    param: CopySingleFileParam,
) -> Result<CommandResponse<String>, String> {
    let service = ClipboardService::new(state.inner().as_ref());
    Ok(string_result(service.copy_single_file(param).await))
}

#[tauri::command]
pub async fn image_save_as(
    state: tauri::State<'_, Arc<AppContext>>,
    param: ClipRecordIdParam,
) -> Result<CommandResponse<String>, String> {
    let service = ClipboardService::new(state.inner().as_ref());
    Ok(string_result(service.image_save_as(param.record_id).await))
}

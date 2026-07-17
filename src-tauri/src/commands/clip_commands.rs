use std::{collections::HashMap, sync::Arc};

use crate::{
    app_context::AppContext,
    dto::clip_dto::{
        ClipRecordIdParam, ClipRecordLiteDTO, FullContentResponse, GetFullContentParam,
        GetImageParam, ImageInfo, ImagePathInfo, PinnedClipRecordParam, QueryParam,
    },
    response::{command_result, CommandResponse},
    services::clip_record_service::ClipRecordService,
};

#[tauri::command]
pub async fn get_clip_records(
    state: tauri::State<'_, Arc<AppContext>>,
    param: QueryParam,
) -> Result<CommandResponse<Vec<ClipRecordLiteDTO>>, String> {
    let service = ClipRecordService::from_context(state.inner().as_ref());
    Ok(command_result(service.query_clip_records(param).await))
}

#[tauri::command]
pub async fn get_image_path(
    state: tauri::State<'_, Arc<AppContext>>,
    param: GetImageParam,
) -> Result<CommandResponse<ImagePathInfo>, String> {
    let service = ClipRecordService::from_context(state.inner().as_ref());
    Ok(command_result(
        service.get_image_path(&param.record_id).await,
    ))
}

#[tauri::command]
pub async fn get_image_info_batch(
    state: tauri::State<'_, Arc<AppContext>>,
    record_ids: Vec<String>,
) -> Result<CommandResponse<HashMap<String, ImageInfo>>, String> {
    let service = ClipRecordService::from_context(state.inner().as_ref());
    Ok(command_result(
        service.get_image_info_batch(record_ids).await,
    ))
}

#[tauri::command]
pub async fn get_full_text_content(
    state: tauri::State<'_, Arc<AppContext>>,
    param: GetFullContentParam,
) -> Result<CommandResponse<FullContentResponse>, String> {
    let service = ClipRecordService::from_context(state.inner().as_ref());
    Ok(command_result(
        service.get_full_text_content(param.record_id).await,
    ))
}

#[tauri::command]
pub async fn set_pinned(
    state: tauri::State<'_, Arc<AppContext>>,
    param: PinnedClipRecordParam,
) -> Result<CommandResponse<String>, String> {
    let service = ClipRecordService::from_context(state.inner().as_ref());
    Ok(command_result(
        service
            .set_pinned(&param.record_id, param.pinned_flag)
            .await,
    ))
}

#[tauri::command]
pub async fn del_record(
    state: tauri::State<'_, Arc<AppContext>>,
    param: ClipRecordIdParam,
) -> Result<CommandResponse<String>, String> {
    let service = ClipRecordService::from_context(state.inner().as_ref());
    Ok(command_result(service.delete_record(param.record_id).await))
}

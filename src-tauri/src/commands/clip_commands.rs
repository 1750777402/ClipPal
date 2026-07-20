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
/// 查询剪贴记录列表，将分页与搜索参数交给剪贴记录服务，并统一包装查询结果。
pub async fn get_clip_records(
    state: tauri::State<'_, Arc<AppContext>>,
    param: QueryParam,
) -> Result<CommandResponse<Vec<ClipRecordLiteDTO>>, String> {
    let service = ClipRecordService::from_context(state.inner().as_ref());
    Ok(command_result(service.query_clip_records(param).await))
}

#[tauri::command]
/// 获取指定图片记录的本地路径，供前端通过安全路径加载图片资源。
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
/// 批量读取图片记录的文件信息，减少前端逐条调用产生的 IPC 开销。
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
/// 获取被列表预览截断的完整文本内容，并返回原始内容长度。
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
/// 修改记录置顶状态，由服务层执行业务操作并返回统一响应。
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
/// 逻辑删除指定剪贴记录，并由服务层处理搜索索引和云同步队列。
pub async fn del_record(
    state: tauri::State<'_, Arc<AppContext>>,
    param: ClipRecordIdParam,
) -> Result<CommandResponse<String>, String> {
    let service = ClipRecordService::from_context(state.inner().as_ref());
    Ok(command_result(service.delete_record(param.record_id).await))
}

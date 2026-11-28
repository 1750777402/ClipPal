use std::{collections::HashMap, sync::Arc};

use {
    crate::app_context::AppContext,
    desktop_app_core::dto::ClipRecordIdParam,
    desktop_app_core::dto::ClipRecordLiteDTO,
    desktop_app_core::dto::FullContentResponse,
    desktop_app_core::dto::GetFullContentParam,
    desktop_app_core::dto::GetImageParam,
    desktop_app_core::dto::ImageInfo,
    desktop_app_core::dto::ImagePathInfo,
    desktop_app_core::dto::PinnedClipRecordParam,
    desktop_app_core::dto::QueryParam,
    crate::response::command_result,
    crate::response::CommandResponse,
    crate::core::application::ClipRecordService,
};

/// 同步版列表附加内容身份和同步限制提示。
#[cfg(feature = "sync")]
#[derive(serde::Serialize)]
pub struct ClipRecordView {
    #[serde(flatten)]
    record: ClipRecordLiteDTO,
    #[serde(skip_serializing_if = "Option::is_none")]
    sync_skip_reason: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    sync_content_key: Option<String>,
}

#[cfg(not(feature = "sync"))]
pub type ClipRecordView = ClipRecordLiteDTO;

#[tauri::command]
/// 查询剪贴记录列表，将分页与搜索参数交给剪贴记录服务，并统一包装查询结果。
pub async fn get_clip_records(
    state: tauri::State<'_, Arc<AppContext>>,
    param: QueryParam,
) -> Result<CommandResponse<Vec<ClipRecordView>>, String> {
    let service = ClipRecordService::from_context(state.inner().as_ref());
    #[cfg(feature = "sync")]
    let result = {
        #[cfg(not(feature = "commercial-extension"))]
        let enabled = crate::sync::runtime::enabled();
        #[cfg(feature = "commercial-extension")]
        let enabled = state.sync().status().await.enabled;
        let limits = state.sync().limits().await;
        service.query_clip_records_with(param, |record, dto| {
            let text_bytes = dto.original_content_length.unwrap_or(dto.content.len());
            let reason = if enabled { selfhost_app_core::skip_reason(record, limits, text_bytes) } else { None };
            ClipRecordView { record: dto, sync_skip_reason: reason,
                sync_content_key: enabled.then(|| record.content_key.clone()) }
        }).await
    };
    #[cfg(not(feature = "sync"))]
    let result = service.query_clip_records(param).await;
    Ok(command_result(result))
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
/// 删除本机记录。启用部署能力的自建包同时登记同步删除。
pub async fn del_record(
    state: tauri::State<'_, Arc<AppContext>>,
    param: ClipRecordIdParam,
) -> Result<CommandResponse<String>, String> {
    #[cfg(all(feature = "sync", not(feature = "commercial-extension")))]
    if crate::sync::runtime::enabled() {
        return Ok(command_result(state.sync().delete_everywhere(param.record_id).await));
    }
    let service = ClipRecordService::from_context(state.inner().as_ref());
    Ok(command_result(service.delete_record(param.record_id).await))
}

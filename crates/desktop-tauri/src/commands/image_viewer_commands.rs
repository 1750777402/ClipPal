use std::sync::Arc;

use desktop_app_core::dto::GetImageParam;
use tauri::{Runtime, State, WebviewWindow};

use crate::{
    app_context::AppContext,
    desktop::shell::image_viewer::{self, ViewerImage},
    errors::AppError,
    response::{command_result, CommandResponse},
};

#[tauri::command]
pub async fn open_image_viewer<R: Runtime>(
    window: WebviewWindow<R>,
    state: State<'_, Arc<AppContext>>,
    param: GetImageParam,
) -> Result<CommandResponse<()>, String> {
    let result = if window.label() == "main" {
        image_viewer::open(&window, state.inner(), &param.record_id).await
    } else {
        Err(AppError::Config("请从剪贴板列表打开图片预览".into()))
    };
    Ok(command_result(result))
}

#[tauri::command]
pub async fn get_image_viewer_image<R: Runtime>(
    window: WebviewWindow<R>,
    state: State<'_, Arc<AppContext>>,
) -> Result<CommandResponse<ViewerImage>, String> {
    let result = if window.label() == image_viewer::LABEL {
        state
            .image_viewer()
            .lock()
            .await
            .image()
            .ok_or_else(|| AppError::Config("没有可预览的图片".into()))
    } else {
        Err(AppError::Config("请从图片预览窗口读取当前图片".into()))
    };
    Ok(command_result(result))
}

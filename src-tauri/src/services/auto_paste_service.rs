use crate::{
    app_context::AppContext,
    domain::clip::ClipRecord,
    errors::{AppError, AppResult},
    system::{
        auto_paste,
        clipboard::{show_accessibility_dialog, write_record},
    },
};

/// 自动粘贴应用服务，负责目标捕获、任务串行化和复制粘贴流程编排。
pub struct AutoPasteService<'a> {
    context: &'a AppContext,
}

impl<'a> AutoPasteService<'a> {
    pub fn from_context(context: &'a AppContext) -> Self {
        Self { context }
    }

    /// 捕获 ClipPal 获得焦点前的前台窗口，并保存到应用运行状态。
    pub fn capture_target(&self) -> AppResult<()> {
        let state = self.context.auto_paste_state();
        match auto_paste::capture_target() {
            Ok(target) => state.replace_target(target),
            Err(error) => {
                // 捕获失败时清空旧目标，避免随后向过期窗口发送粘贴快捷键。
                state.replace_target(None)?;
                Err(error)
            }
        }
    }

    /// 只复制记录，但仍与自动粘贴共用串行锁，防止覆盖待粘贴内容。
    pub async fn copy_only(&self, record: &ClipRecord) -> Result<(), String> {
        let _operation = self
            .context
            .auto_paste_state()
            .operation_lock()
            .lock()
            .await;
        write_record(self.context, record).await
    }

    /// 写入剪贴板并向已捕获的目标窗口发送粘贴快捷键。
    pub async fn copy_and_paste(&self, record: &ClipRecord) -> Result<(), String> {
        let state = self.context.auto_paste_state();
        let _operation = state.operation_lock().lock().await;

        write_record(self.context, record).await?;
        let target = state
            .target()
            .map_err(String::from)?
            .ok_or_else(|| "没有找到自动粘贴目标窗口，内容已复制".to_string())?;

        // 先隐藏 ClipPal，让系统有机会自然恢复之前的前台窗口。
        let window = self.context.main_window().map_err(String::from)?;
        if window.is_visible().unwrap_or(true) {
            window
                .hide()
                .map_err(|error| format!("隐藏 ClipPal 窗口失败: {}", error))?;
        }

        // 剪贴板写入完成后给目标应用一个很短的读取准备时间。
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;

        let paste_result = tokio::task::spawn_blocking(move || -> AppResult<()> {
            auto_paste::restore_and_verify(&target)?;
            auto_paste::send_paste_shortcut()
        })
        .await
        .map_err(|error| AppError::AutoPaste(format!("自动粘贴任务执行失败: {}", error)))?;

        if let Err(error) = &paste_result {
            if error.to_string().contains("权限") {
                if let Ok(app_handle) = self.context.app_handle() {
                    let app_handle_for_dialog = app_handle.clone();
                    let _ = app_handle.run_on_main_thread(move || {
                        show_accessibility_dialog(&app_handle_for_dialog);
                    });
                }
            }
        }

        paste_result.map_err(String::from)
    }
}

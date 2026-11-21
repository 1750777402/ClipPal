use tauri_plugin_dialog::DialogExt;

use {
    crate::app_context::AppContext,
    clip_core::domain::clip::ClipRecord,
    desktop_app_core::dto::CopySingleFileParam,
    crate::errors::AppError,
    crate::errors::AppResult,
    clip_core::repositories::ClipRecordRepository,
    crate::desktop::system::auto_paste,
    crate::desktop::system::clipboard::show_accessibility_dialog,
    crate::desktop::system::clipboard::write_record,
    crate::desktop::shell::window::WindowHideGuard,
};

/// 系统剪贴板应用服务，负责加载记录并编排复制、自动粘贴和另存为流程。
pub struct ClipboardService<'a> {
    context: &'a AppContext,
    repository: &'a dyn ClipRecordRepository,
}

impl<'a> ClipboardService<'a> {
    /// 从应用上下文取得剪贴记录仓储，创建一次 command 使用的服务实例。
    pub fn from_context(context: &'a AppContext) -> Self {
        Self {
            context,
            repository: context.repositories().clip_records(),
        }
    }

    /// 捕获 ClipPal 获得焦点前的前台窗口，供后续自动粘贴恢复。
    pub fn capture_auto_paste_target(&self) -> AppResult<()> {
        self.context.auto_paste_state().record_capture(auto_paste::capture_target())
    }
    /// 将完整记录写入系统剪贴板，并按调用参数和用户设置决定是否自动粘贴。
    pub async fn copy_record(
        &self,
        record_id: String,
        allow_auto_paste: bool,
    ) -> Result<String, String> {
        // 先通过仓储读取领域对象，系统剪贴板层不直接访问数据库。
        let record = self
            .repository
            .find_by_id(desktop_app_core::records::parse_id(&record_id).map_err(String::from)?)
            .await
            .map_err(|_| "剪贴记录查询失败".to_string())?
            .ok_or_else(|| "记录不存在".to_string())?;
        // 自动粘贴必须同时满足 command 允许和用户设置已开启两个条件。
        let auto_paste_enabled = allow_auto_paste
            && self
                .context
                .with_settings(|settings| settings.auto_paste == 1)
                .unwrap_or(false);

        if auto_paste_enabled {
            self.copy_and_paste(&record).await?;
        } else {
            self.copy_only(&record).await?;
        }

        Ok(String::new())
    }

    /// 将图片记录保存到用户选择的位置，并在保存对话框期间禁止主窗口自动隐藏。
    pub async fn image_save_as(&self, record_id: String) -> AppResult<String> {
        // 仓储只返回记录数据，文件存在性和系统对话框由本服务继续编排。
        let record = self.repository.find_by_id(desktop_app_core::records::parse_id(&record_id)?).await?
            .ok_or_else(|| AppError::General("未找到指定的剪贴板记录".into()))?;
        if record.r#type != clip_core::domain::clip::IMAGE_TYPE {
            return Err(AppError::General("仅支持图片类型另存为".into()));
        }
        let image_path = super::clip_record_service::ClipRecordService::from_context(self.context)
            .get_image_path(&record.id.to_string()).await?.file_path;
        let app_handle = self.context.app_handle()?;
        let hide_flag = self.context.window_hide_flag();
        // 在对话框打开之前进入保护，直到用户选择完成且文件写入结束。
        let _guard = WindowHideGuard::new(hide_flag);
        let (sender, receiver) = tokio::sync::oneshot::channel();
        app_handle.dialog().file().add_filter("图片", &["png"])
            .set_file_name(format!("clip_{}", record.id))
            .save_file(move |path| { let _ = sender.send(path); });
        let selected = receiver.await
            .map_err(|error| AppError::System(format!("保存对话框回调失败: {}", error)))?;
        let Some(selected) = selected else { return Ok(String::new()); };
        let destination = selected.as_path().map(|path| path.to_path_buf())
            .ok_or_else(|| AppError::System("保存位置不是本地文件路径".into()))?;
        tokio::task::spawn_blocking(move || std::fs::copy(image_path, destination)).await
            .map_err(|error| AppError::System(format!("保存图片任务失败: {}", error)))??;
        Ok("图片已成功保存".into())
    }

    /// 按资源自增 ID 精确定位，支持同一条记录中存在多个同名文件。
    pub async fn copy_single_file(&self, param: CopySingleFileParam) -> Result<String, String> {
        let record=self.repository.find_by_id(desktop_app_core::records::parse_id(&param.record_id).map_err(String::from)?).await
            .map_err(String::from)?.ok_or_else(||"记录不存在".to_string())?;
        let resource_id=desktop_app_core::records::parse_id(&param.resource_id).map_err(String::from)?;
        let payload=desktop_app_core::clipboard::prepare_copy(self.context.core(),&record,Some(resource_id)).map_err(String::from)?;
        let _operation=self.context.auto_paste_state().operation_lock().lock().await;
        crate::desktop::system::clipboard::write_payload(self.context,payload).await?;
        Ok(String::new())
    }

    /// 串行写入剪贴板，不切换焦点、不发送系统粘贴按键。
    async fn copy_only(&self, record: &ClipRecord) -> Result<(), String> {
        let _operation = self
            .context
            .auto_paste_state()
            .operation_lock()
            .lock()
            .await;
        write_record(self.context, record).await
    }

    /// 先复制，再验证权限和原目标窗口，最后恢复目标并发送一次粘贴按键。
    async fn copy_and_paste(&self, record: &ClipRecord) -> Result<(), String> {
        let state = self.context.auto_paste_state();
        let _operation = state.operation_lock().lock().await;

        // 复制前固定目标和唤起序号，异步写入期间的新唤起不能改变本次粘贴目的地。
        use std::sync::atomic::Ordering;
        let generation = state.wake_generation.clone();
        let started_generation = generation.load(Ordering::SeqCst);
        let target = state.target().map_err(String::from)?;
        write_record(self.context, record).await?;
        if let Err(error) = auto_paste::ensure_permission() {
            show_accessibility_dialog(self.context)?;
            return Err(error.into());
        }
        let target = target.ok_or_else(|| "没有找到自动粘贴目标窗口，内容已复制".to_string())?;
        if generation.load(Ordering::SeqCst) != started_generation {
            return Err("窗口已重新唤起，本次自动粘贴已取消，内容已复制".into());
        }
        if !self.context.window_hide_flag().is_can_hide() {
            return Err("请先完成当前对话框操作，内容已复制".into());
        }

        let window = self.context.main_window().map_err(String::from)?;
        if window.is_visible().unwrap_or(true) {
            window
                .hide()
                .map_err(|error| format!("隐藏 ClipPal 窗口失败: {}", error))?;
        }

        tokio::time::sleep(std::time::Duration::from_millis(100)).await;

        let paste_result = tokio::task::spawn_blocking(move || -> AppResult<()> {
            let verify_generation = || {
                if generation.load(Ordering::SeqCst) == started_generation { Ok(()) }
                else { Err(AppError::AutoPaste("窗口已重新唤起，本次自动粘贴已取消，内容已复制".into())) }
            };
            verify_generation()?;
            auto_paste::restore_and_verify(&target)?;
            verify_generation()?;
            auto_paste::send_paste_shortcut()
        })
        .await
        .map_err(|error| AppError::AutoPaste(format!("自动粘贴任务执行失败: {}", error)))?;

        if paste_result.is_err() {
            // 权限可能在等待目标恢复时被撤销；直接读取系统状态，不匹配错误文案。
            if crate::desktop::system::accessibility::status() == crate::desktop::system::accessibility::AutoPastePermission::Required {
                if let Err(dialog_error) = show_accessibility_dialog(self.context) {
                    log::error!("显示辅助功能权限提示失败: {}", dialog_error);
                }
            }
        }

        paste_result.map_err(String::from)
    }
}

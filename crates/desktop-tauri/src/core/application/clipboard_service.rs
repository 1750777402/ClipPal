use clipboard_listener::ClipType;
use tauri::Manager;
use tauri_plugin_clipboard_pal::desktop::ClipboardPal;
use tauri_plugin_dialog::DialogExt;

use {
    crate::app_context::AppContext,
    clip_core::domain::clip::ClipRecord,
    desktop_app_core::dto::CopySingleFileParam,
    crate::errors::AppError,
    crate::errors::AppResult,
    clip_core::repositories::ClipRecordRepository,
    crate::desktop::system::auto_paste,
    crate::desktop::system::clipboard::create_named_temp_files,
    crate::desktop::system::clipboard::show_accessibility_dialog,
    crate::desktop::system::clipboard::write_record,
    clip_core::files::paths::str_to_safe_string,
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
            .find_by_id(&record_id)
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
        let record = self.repository.find_by_id(&record_id).await?
            .filter(|record| !record.is_deleted())
            .ok_or_else(|| AppError::General("未找到指定的剪贴板记录".into()))?;
        if record.r#type != ClipType::Image.to_string() {
            return Err(AppError::General("仅支持图片类型另存为".into()));
        }
        let image_path = super::clip_record_service::ClipRecordService::from_context(self.context)
            .get_image_path(&record.id).await?.file_path;
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

    /// 从多文件记录中解析指定显示名称，并将对应真实文件写入系统剪贴板。
    pub async fn copy_single_file(&self, param: CopySingleFileParam) -> Result<String, String> {
        let record = match self.repository.find_by_id(&param.record_id).await {
            Ok(Some(record)) => record,
            Ok(None) => return Err("记录不存在".to_string()),
            Err(_) => return Err("剪贴记录查询失败".to_string()),
        };

        if record.r#type != ClipType::File.to_string() {
            return Err("只支持文件类型的单个文件复制".to_string());
        }

        let app_handle = self
            .context
            .app_handle()
            .map_err(|error| error.to_string())?;
        let clipboard = app_handle.state::<ClipboardPal>();
        let display_names = record.content.as_str().unwrap_or("");
        let actual_paths = record.local_file_path.as_deref().unwrap_or("");

        if display_names.is_empty() || actual_paths.is_empty() {
            return Err("文件信息无效".to_string());
        }

        let display_list: Vec<String> = display_names
            .split(":::")
            .map(|value| value.to_string())
            .collect();
        let actual_list: Vec<String> = actual_paths
            .split(":::")
            .map(|value| value.to_string())
            .collect();

        // 显示名称和真实路径按相同索引保存，需要先找到前端选择的显示名称。
        let file_index = display_list
            .iter()
            .position(|name| name == &param.file_path);
        let actual_file_path = match file_index {
            Some(index) if index < actual_list.len() => &actual_list[index],
            _ => return Err("指定的文件不在此记录中".to_string()),
        };

        if !std::path::Path::new(actual_file_path).exists() {
            return Err(format!(
                "文件不存在 {}",
                str_to_safe_string(&param.file_path)
            ));
        }

        // 单文件复制也参与同一串行队列，避免覆盖正在等待发送的自动粘贴内容。
        let _operation = self
            .context
            .auto_paste_state()
            .operation_lock()
            .lock()
            .await;

        // 优先创建保留显示名称的临时硬链接，失败时退回真实文件路径。
        match create_named_temp_files(&[param.file_path], &[actual_file_path.clone()]).await {
            Ok(temp_files) => {
                clipboard.write_files_uris(temp_files)?;
            }
            Err(error) => {
                log::warn!("创建单文件临时副本失败，使用真实路径: {}", error);
                clipboard.write_files_uris(vec![actual_file_path.clone()])?;
            }
        }

        Ok(String::new())
    }

    async fn copy_only(&self, record: &ClipRecord) -> Result<(), String> {
        let _operation = self
            .context
            .auto_paste_state()
            .operation_lock()
            .lock()
            .await;
        write_record(self.context, record).await
    }

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

        if let Err(error) = &paste_result {
            if error.to_string().contains("权限") {
                if let Err(dialog_error) = show_accessibility_dialog(self.context) {
                    log::error!("显示辅助功能权限提示失败: {}", dialog_error);
                }
            }
        }

        paste_result.map_err(String::from)
    }
}

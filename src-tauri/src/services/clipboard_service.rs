use clipboard_listener::ClipType;
use tauri::Manager;
use tauri_plugin_clipboard_pal::desktop::ClipboardPal;
use tauri_plugin_dialog::DialogExt;

use crate::{
    app_context::AppContext,
    auto_paste,
    dto::clip_dto::CopySingleFileParam,
    infra::repositories::ClipRecordRepository,
    system::clipboard::{create_named_temp_files, show_accessibility_dialog, write_record},
    utils::path_utils::str_to_safe_string,
    window::WindowHideGuard,
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
        // 根据记录类型写入文本、图片或文件 URI。
        write_record(self.context, &record).await?;

        // 自动粘贴必须同时满足 command 允许和用户设置已开启两个条件。
        let auto_paste_enabled = allow_auto_paste
            && self
                .context
                .with_settings(|settings| settings.auto_paste == 1)
                .unwrap_or(false);

        if auto_paste_enabled {
            let app_handle = self
                .context
                .app_handle()
                .map_err(|error| error.to_string())?;
            // 等待剪贴板内容稳定后再模拟粘贴，避免目标应用读取到旧内容。
            std::thread::spawn(move || {
                std::thread::sleep(std::time::Duration::from_millis(100));
                if let Err(error) = auto_paste::auto_paste_to_previous_window() {
                    if error.to_string().contains("权限") {
                        let app_handle_for_dialog = app_handle.clone();
                        let _ = app_handle.run_on_main_thread(move || {
                            show_accessibility_dialog(&app_handle_for_dialog);
                        });
                    }
                }
            });
        }

        Ok(String::new())
    }

    /// 将图片记录保存到用户选择的位置，并在保存对话框期间禁止主窗口自动隐藏。
    pub async fn image_save_as(&self, record_id: String) -> Result<String, String> {
        // 仓储只返回记录数据，文件存在性和系统对话框由本服务继续编排。
        match self.repository.find_by_id(&record_id).await {
            Ok(Some(record)) => {
                // 另存为仅接受图片记录，防止把文本或文件路径当作图片复制。
                if record.r#type != ClipType::Image.to_string() {
                    return Err("仅支持图片类型另存为".to_string());
                }

                let relative_path = record.content.as_str().ok_or("图片路径无效")?;
                let resources_dir =
                    crate::utils::file_dir::get_resources_dir().ok_or("资源目录获取失败")?;
                let image_path = resources_dir.join(relative_path);
                if !image_path.exists() {
                    return Err("图片资源丢失".to_string());
                }

                let app_handle = self
                    .context
                    .app_handle()
                    .map_err(|error| error.to_string())?;
                let image_path_for_save = image_path.clone();
                let hide_flag = self.context.window_hide_flag();
                // 回调执行期间使用隐藏保护器，避免主窗口因系统对话框失焦而消失。
                app_handle
                    .dialog()
                    .file()
                    .add_filter("图片", &["png"])
                    .set_file_name(format!("clip_{}", record.id))
                    .save_file(move |file_path| {
                        let _guard = WindowHideGuard::new(hide_flag.as_ref());
                        if let Some(selected_path) =
                            file_path.and_then(|path| path.as_path().map(|path| path.to_path_buf()))
                        {
                            let _ = std::fs::copy(&image_path_for_save, selected_path);
                        }
                    });

                Ok("图片已成功保存".to_string())
            }
            Ok(None) => Err("未找到指定的剪贴板记录".to_string()),
            Err(_) => Err("未找到该记录".to_string()),
        }
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

        // 优先创建保留显示名称的临时硬链接，失败时退回真实文件路径。
        match create_named_temp_files(&[param.file_path], &[actual_file_path.clone()]).await {
            Ok(temp_files) => {
                let _ = clipboard.write_files_uris(temp_files);
            }
            Err(_) => {
                let _ = clipboard.write_files_uris(vec![actual_file_path.clone()]);
            }
        }

        Ok(String::new())
    }
}

use clipboard_listener::ClipType;
use tauri::Manager;
use tauri_plugin_clipboard_pal::desktop::ClipboardPal;
use tauri_plugin_dialog::DialogExt;

use crate::{
    app_context::AppContext,
    auto_paste,
    biz::{
        clip_record::ClipRecord,
        copy_clip_record::{
            copy_record_to_clipboard, create_temp_files_with_correct_names,
            show_accessibility_permission_dialog, CopyClipRecord,
        },
    },
    dto::clip_dto::CopySingleFileParam,
    utils::path_utils::str_to_safe_string,
    window::WindowHideGuard,
};

pub struct ClipboardService<'a> {
    context: &'a AppContext,
}

impl<'a> ClipboardService<'a> {
    pub fn new(context: &'a AppContext) -> Self {
        Self { context }
    }

    pub async fn copy_record(
        &self,
        record_id: String,
        allow_auto_paste: bool,
    ) -> Result<String, String> {
        let param = CopyClipRecord { record_id };
        copy_record_to_clipboard(self.context, &param).await?;

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
            std::thread::spawn(move || {
                std::thread::sleep(std::time::Duration::from_millis(100));
                if let Err(error) = auto_paste::auto_paste_to_previous_window() {
                    if error.to_string().contains("权限") {
                        let app_handle_for_dialog = app_handle.clone();
                        let _ = app_handle.run_on_main_thread(move || {
                            show_accessibility_permission_dialog(&app_handle_for_dialog);
                        });
                    }
                }
            });
        }

        Ok(String::new())
    }

    pub async fn image_save_as(&self, record_id: String) -> Result<String, String> {
        let records = ClipRecord::select_by_id(self.context.db(), &record_id).await;
        match records {
            Ok(records) => {
                let record = records.first().ok_or("未找到指定的剪贴板记录")?;
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
            Err(_) => Err("未找到该记录".to_string()),
        }
    }

    pub async fn copy_single_file(&self, param: CopySingleFileParam) -> Result<String, String> {
        let record = match ClipRecord::select_by_id(self.context.db(), &param.record_id).await {
            Ok(records) => records
                .first()
                .cloned()
                .ok_or_else(|| "记录不存在".to_string())?,
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

        match create_temp_files_with_correct_names(&[param.file_path], &[actual_file_path.clone()])
            .await
        {
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

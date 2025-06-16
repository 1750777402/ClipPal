use std::{fs, path::PathBuf};

use serde::{Deserialize, Serialize};
use tauri::AppHandle;
use tauri_plugin_autostart::ManagerExt;

use crate::{CONTEXT, utils::file_dir::get_config_dir};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Settings {
    // 最大记录条数
    pub max_records: u32,
    // 是否自动启动 0 关闭 1 开启
    pub auto_start: u32,
    // 快捷键组合
    pub shortcut_key: String,
    // 是否开启云同步 0 关闭 1 开启
    pub cloud_sync: u32,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            max_records: 200,
            auto_start: 0,
            shortcut_key: String::from("Ctrl+`"),
            cloud_sync: 0,
        }
    }
}

/// 初始化系统设置
pub fn init_settings() {
    save_settings(load_settings());
}

pub fn get_settings_file_path() -> Option<PathBuf> {
    let config_dir = get_config_dir();
    if let Some(config_dir) = config_dir {
        Some(config_dir.join("settings.json"))
    } else {
        None
    }
}

#[tauri::command]
pub fn load_settings() -> Settings {
    let mut settings = Settings::default();
    if let Some(path) = get_settings_file_path() {
        if path.exists() {
            let data = fs::read_to_string(&path).unwrap_or_default();
            settings = serde_json::from_str(&data).unwrap_or_default()
        } else {
            let settings = Settings::default();
            save_settings(settings.clone());
        }
    }
    settings
}

#[tauri::command]
pub fn save_settings(settings: Settings) {
    if let Some(path) = get_settings_file_path() {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).ok();
        }
        if let Ok(json) = serde_json::to_string_pretty(&settings) {
            fs::write(path, json).ok();
            // 设置开机自启
            set_auto_start(settings.auto_start == 1);
        }
    }
    // 把系统配置存储到上下文中
    CONTEXT.set(settings.clone());
}

pub fn set_auto_start(auto_start: bool) {
    let app_handle = CONTEXT.get::<AppHandle>();
    let autostart_manager = app_handle.autolaunch();
    if auto_start {
        let _ = autostart_manager.enable();
    } else {
        let _ = autostart_manager.disable();
    }
}

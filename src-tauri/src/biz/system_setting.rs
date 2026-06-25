use log;
use std::{
    fs,
    marker::{Send, Sync},
    path::PathBuf,
    sync::{Arc, RwLock},
};

use serde::{Deserialize, Serialize};
use tauri_plugin_autostart::ManagerExt;
use tauri_plugin_global_shortcut::GlobalShortcutExt;

use crate::{
    app_context::{app_context, AppContext},
    biz::cloud_sync_timer::trigger_immediate_sync,
    biz::vip_checker::VipChecker,
    errors::{AppError, AppResult},
    global_shortcut::parse_shortcut_strict,
    response::{ok, string_result, CommandResponse},
    utils::{
        file_dir::get_config_dir,
        lock_utils::lock_utils::{safe_read_lock, safe_write_lock},
    },
};

pub static DEFAULT_BLOOM_FILTER_TRUST_THRESHOLD: usize = 1024 * 1024;
pub static DEFAULT_DIRECT_CONTAINS_THRESHOLD: usize = 128 * 1024;
pub static SYNC_INTERVAL_SECONDS: u32 = 30;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Settings {
    pub max_records: u32,
    pub auto_start: u32,
    pub shortcut_key: String,
    pub cloud_sync: u32,
    pub auto_paste: u32,
    pub tutorial_completed: u32,
    pub bloom_filter_trust_threshold: Option<usize>,
    pub direct_contains_threshold: Option<usize>,
    pub cloud_sync_interval: u32,
}

unsafe impl Send for Settings {}
unsafe impl Sync for Settings {}

impl Default for Settings {
    fn default() -> Self {
        Self {
            max_records: 200,
            auto_start: 0,
            shortcut_key: String::from("Ctrl+`"),
            cloud_sync: 0,
            auto_paste: 1,
            tutorial_completed: 0,
            bloom_filter_trust_threshold: Some(DEFAULT_BLOOM_FILTER_TRUST_THRESHOLD),
            direct_contains_threshold: Some(DEFAULT_DIRECT_CONTAINS_THRESHOLD),
            cloud_sync_interval: SYNC_INTERVAL_SECONDS,
        }
    }
}

#[allow(dead_code)]
pub fn load_settings_context() -> Arc<RwLock<Settings>> {
    let settings = load_settings_value();
    create_default_config_if_not_exists(&settings);
    Arc::new(RwLock::new(settings))
}

fn create_default_config_if_not_exists(settings: &Settings) {
    if let Some(path) = get_settings_file_path() {
        if !path.exists() {
            if let Err(e) = save_settings_to_file(settings) {
                log::warn!("创建默认配置文件失败: {}", e);
            }
        }
    }
}

pub fn get_settings_file_path() -> Option<PathBuf> {
    get_config_dir().map(|config_dir| config_dir.join("settings.json"))
}

#[tauri::command]
pub fn load_settings() -> CommandResponse<Settings> {
    ok(load_settings_value())
}

pub fn load_settings_value() -> Settings {
    if let Some(path) = get_settings_file_path() {
        if path.exists() {
            if let Ok(data) = fs::read_to_string(&path) {
                if let Ok(settings) = serde_json::from_str(&data) {
                    return settings;
                }
            }
        }
    }
    Settings::default()
}

#[tauri::command]
pub async fn save_settings(
    state: tauri::State<'_, Arc<AppContext>>,
    settings: Settings,
) -> Result<CommandResponse<()>, String> {
    Ok(string_result(save_settings_with_context(&state, settings).await))
}

pub async fn save_settings_with_context(
    app_context: &AppContext,
    settings: Settings,
) -> Result<(), String> {
    validate_settings(&settings)
        .await
        .map_err(|e| e.to_string())?;

    let current_settings = {
        let lock = app_context.settings();
        let current = safe_read_lock(&lock).map_err(|e| e.to_string())?;
        current.clone()
    };

    let mut applied_settings = Vec::new();

    if settings.shortcut_key != current_settings.shortcut_key {
        match update_global_shortcut(app_context, &settings.shortcut_key).await {
            Ok(_) => applied_settings.push("shortcut"),
            Err(e) => {
                let _ = rollback_settings(&current_settings, &applied_settings).await;
                return Err(format!("快捷键设置失败: {}", e));
            }
        }
    }

    if settings.cloud_sync != current_settings.cloud_sync && settings.cloud_sync == 1 {
        match validate_cloud_sync_permission().await {
            Ok(_) => applied_settings.push("cloud_sync"),
            Err(e) => {
                let _ = rollback_settings(&current_settings, &applied_settings).await;
                return Err(format!("开启云同步失败: {}", e));
            }
        }
    }

    if settings.auto_start != current_settings.auto_start {
        match set_auto_start(app_context, settings.auto_start == 1) {
            Ok(_) => applied_settings.push("autostart"),
            Err(e) => {
                let _ = rollback_settings(&current_settings, &applied_settings).await;
                return Err(format!("开机自启动设置失败: {}", e));
            }
        }
    }

    if let Err(e) = save_settings_to_file(&settings) {
        let _ = rollback_settings(&current_settings, &applied_settings).await;
        return Err(format!("保存配置失败: {}", e));
    }

    let need_trigger_sync =
        settings.cloud_sync != current_settings.cloud_sync && settings.cloud_sync == 1;

    {
        let lock = app_context.settings();
        let mut current = safe_write_lock(&lock).map_err(|e| e.to_string())?;
        *current = settings;
    }

    if need_trigger_sync {
        if let Err(e) = trigger_immediate_sync() {
            log::warn!("触发立即云同步失败: {}", e);
        }
    }

    Ok(())
}

async fn validate_settings(settings: &Settings) -> AppResult<()> {
    let max_allowed = VipChecker::get_cached_max_records_limit().unwrap_or(300);

    if settings.max_records < 50 {
        return Err(AppError::Config("记录条数不能少于 50".to_string()));
    }

    if settings.max_records > max_allowed {
        return Err(AppError::Config(format!(
            "记录条数不能超过 {}",
            max_allowed
        )));
    }

    if settings.shortcut_key.is_empty() {
        return Err(AppError::Config("快捷键不能为空".to_string()));
    }

    parse_shortcut_strict(&settings.shortcut_key).map_err(|e| {
        AppError::Config(format!(
            "快捷键格式错误，请使用如 Ctrl+Shift+C 的组合键。{}",
            e
        ))
    })?;

    Ok(())
}

fn is_valid_shortcut_format(shortcut: &str) -> bool {
    let parts: Vec<&str> = shortcut.split('+').collect();
    if parts.len() < 2 || parts.len() > 4 {
        return false;
    }

    let modifier_count = parts
        .iter()
        .filter(|&&part| matches!(part, "Ctrl" | "Shift" | "Alt" | "Meta" | "Cmd"))
        .count();

    modifier_count >= 1 && modifier_count < parts.len()
}

async fn update_global_shortcut(app_context: &AppContext, shortcut: &str) -> AppResult<()> {
    let app_handle = app_context.app_handle()?;
    let shortcut_obj = parse_shortcut_strict(shortcut)
        .map_err(|e| AppError::GlobalShortcut(format!("快捷键格式无效: {}", e)))?;

    let _ = app_handle.global_shortcut().unregister_all();

    match app_handle.global_shortcut().on_shortcut(shortcut_obj, {
        let app_handle_clone = app_handle.clone();
        move |_app, _shortcut_triggered, event| {
            if event.state() == tauri_plugin_global_shortcut::ShortcutState::Pressed {
                use tauri::Manager;
                if let Some(window) = app_handle_clone.get_webview_window("main") {
                    let _ = window.show();
                    let _ = window.set_focus();
                }
            }
        }
    }) {
        Ok(_) => Ok(()),
        Err(e) => Err(AppError::GlobalShortcut(format!("快捷键注册失败: {}", e))),
    }
}

fn set_auto_start(app_context: &AppContext, auto_start: bool) -> AppResult<()> {
    let app_handle = app_context.app_handle()?;
    let autostart_manager = app_handle.autolaunch();

    match if auto_start {
        autostart_manager.enable()
    } else {
        autostart_manager.disable()
    } {
        Ok(_) => Ok(()),
        Err(e) => Err(AppError::Config(format!("开机自启动设置失败: {}", e))),
    }
}

pub fn save_settings_to_file(settings: &Settings) -> AppResult<()> {
    let path = get_settings_file_path()
        .ok_or_else(|| AppError::Config("无法获取配置文件路径".to_string()))?;

    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }

    let json =
        serde_json::to_string_pretty(settings).map_err(|e| AppError::Serde(e.to_string()))?;
    fs::write(path, json).map_err(AppError::Io)?;

    Ok(())
}

async fn rollback_settings(previous_settings: &Settings, applied_settings: &[&str]) -> AppResult<()> {
    let context = app_context()?;

    for setting_type in applied_settings {
        match *setting_type {
            "shortcut" => {
                let _ = update_global_shortcut(&context, &previous_settings.shortcut_key).await;
            }
            "autostart" => {
                let _ = set_auto_start(&context, previous_settings.auto_start == 1);
            }
            _ => {}
        }
    }

    Ok(())
}

#[tauri::command]
pub async fn validate_shortcut(
    state: tauri::State<'_, Arc<AppContext>>,
    shortcut: String,
) -> Result<CommandResponse<bool>, String> {
    Ok(string_result(async {
        if !is_valid_shortcut_format(&shortcut) {
            return Ok(false);
        }

        let current_shortcut = {
            let lock = state.settings();
            let result = match safe_read_lock(&lock) {
                Ok(current) => current.shortcut_key.clone(),
                Err(_) => String::new(),
            };
            result
        };

        if shortcut == current_shortcut {
            return Ok(true);
        }

        parse_shortcut_strict(&shortcut)?;
        Ok(true)
    }
    .await))
}

pub async fn check_cloud_sync_enabled() -> bool {
    let Ok(context) = app_context() else {
        return false;
    };
    let settings_lock = context.settings();
    if let Ok(settings) = safe_read_lock(&settings_lock) {
        return settings.cloud_sync == 1;
    }
    false
}

pub async fn disable_cloud_sync() -> Result<(), String> {
    log::info!("禁用云同步功能");

    let context = app_context().map_err(|e| e.to_string())?;
    let settings_lock = context.settings();

    {
        let mut current_settings = safe_write_lock(&settings_lock).map_err(|e| e.to_string())?;
        if current_settings.cloud_sync == 0 {
            return Ok(());
        }
        current_settings.cloud_sync = 0;
    }

    let settings_for_file = {
        let current_settings = safe_read_lock(&settings_lock).map_err(|e| e.to_string())?;
        current_settings.clone()
    };

    save_settings_to_file(&settings_for_file).map_err(|e| e.to_string())
}

async fn validate_cloud_sync_permission() -> Result<(), String> {
    use crate::utils::token_manager::has_valid_auth;

    if !has_valid_auth() {
        return Err("请先登录账号才能开启云同步功能".to_string());
    }

    match VipChecker::check_cloud_sync_permission().await {
        Ok((allowed, message)) => {
            if !allowed {
                return Err(message);
            }
            Ok(())
        }
        Err(e) => Err(format!("权限检查失败: {}", e)),
    }
}

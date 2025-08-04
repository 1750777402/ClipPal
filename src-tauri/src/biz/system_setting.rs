use log;
use std::{
    fs,
    marker::{Send, Sync},
    path::PathBuf,
    sync::{Arc, RwLock},
};

use serde::{Deserialize, Serialize};
use tauri::AppHandle;
use tauri_plugin_autostart::ManagerExt;
use tauri_plugin_global_shortcut::GlobalShortcutExt;

use crate::{
    CONTEXT,
    errors::{AppError, AppResult, lock_utils::{safe_read_lock, safe_write_lock}},
    global_shortcut::parse_shortcut,
    utils::file_dir::get_config_dir,
};

// 默认超过这个大小的内容，使用布隆过滤器进行搜索   不会进行contains
pub static DEFAULT_BLOOM_FILTER_TRUST_THRESHOLD: usize = 1 * 1024 * 1024;

// 默认小于这个大小的内容，直接使用contains进行搜索
pub static DEFAULT_DIRECT_CONTAINS_THRESHOLD: usize = 128 * 1024;

// 定时任务间隔（秒）
pub static SYNC_INTERVAL_SECONDS: u32 = 30;

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
    // 是否开启自动粘贴 0 关闭 1 开启
    pub auto_paste: u32,
    // 是否已完成新手引导 0 未完成 1 已完成
    pub tutorial_completed: u32,
    // 搜索索引最大内容大小（字节）
    pub bloom_filter_trust_threshold: Option<usize>,
    // 直接使用contains搜索的内容大小阈值（字节）
    pub direct_contains_threshold: Option<usize>,
    // 拉取云端记录的定时任务间隔时间
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
            auto_paste: 1,         // 默认开启自动粘贴
            tutorial_completed: 0, // 默认未完成引导
            bloom_filter_trust_threshold: Some(DEFAULT_BLOOM_FILTER_TRUST_THRESHOLD), // 默认1MB
            direct_contains_threshold: Some(DEFAULT_DIRECT_CONTAINS_THRESHOLD), // 默认128KB
            cloud_sync_interval: SYNC_INTERVAL_SECONDS, // 默认30秒
        }
    }
}

/// 初始化系统设置
pub fn init_settings() {
    let settings = load_settings();
    // 把系统配置存储到上下文中，使用 RwLock 允许并发读取
    CONTEXT.set(Arc::new(RwLock::new(settings.clone())));
    
    // 如果配置文件不存在，使用已加载的设置创建默认配置文件
    create_default_config_if_not_exists(&settings);
}

/// 如果配置文件不存在，创建默认配置文件
fn create_default_config_if_not_exists(settings: &Settings) {
    if let Some(path) = get_settings_file_path() {
        if !path.exists() {
            if let Err(e) = save_settings_to_file(settings) {
                log::warn!("创建默认配置文件失败: {}", e);
            } else {
                log::info!("已创建默认配置文件");
            }
        }
    }
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
    if let Some(path) = get_settings_file_path() {
        if path.exists() {
            let data = fs::read_to_string(&path).unwrap_or_default();
            if let Ok(settings) = serde_json::from_str(&data) {
                return settings;
            }
        }
    }
    // 如果文件不存在或解析失败，返回默认设置
    Settings::default()
}

#[tauri::command]
pub async fn save_settings(settings: Settings) -> Result<(), String> {
    // 1. 验证设置的有效性
    validate_settings(&settings).map_err(|e| e.to_string())?;

    // 2. 获取当前设置并立即释放锁
    let current_settings = {
        let lock = CONTEXT.get::<Arc<RwLock<Settings>>>().clone();
        let current = safe_read_lock(&lock).map_err(|e| e.to_string())?;
        current.clone()
    };

    // 3. 尝试应用新设置（按顺序执行，失败时回滚）
    let mut applied_settings = Vec::new();

    // 3.1 尝试更新全局快捷键
    if settings.shortcut_key != current_settings.shortcut_key {
        match update_global_shortcut(&settings.shortcut_key).await {
            Ok(_) => applied_settings.push(("shortcut", true)),
            Err(e) => {
                // 回滚已应用的设置
                if let Err(rollback_err) = rollback_settings(&applied_settings).await {
                    log::error!("回滚设置失败: {}", rollback_err);
                }
                return Err(format!("快捷键设置失败: {}", e));
            }
        }
    }

    // 3.2 尝试设置开机自启
    if settings.auto_start != current_settings.auto_start {
        match set_auto_start(settings.auto_start == 1) {
            Ok(_) => applied_settings.push(("autostart", true)),
            Err(e) => {
                if let Err(rollback_err) = rollback_settings(&applied_settings).await {
                    log::error!("回滚设置失败: {}", rollback_err);
                }
                return Err(format!("开机自启设置失败: {}", e));
            }
        }
    }

    // 3.3 保存到文件
    match save_settings_to_file(&settings) {
        Ok(_) => applied_settings.push(("file", true)),
        Err(e) => {
            if let Err(rollback_err) = rollback_settings(&applied_settings).await {
                log::error!("回滚设置失败: {}", rollback_err);
            }
            return Err(format!("文件保存失败: {}", e));
        }
    }

    // 4. 更新上下文中的设置
    {
        let lock = CONTEXT.get::<Arc<RwLock<Settings>>>().clone();
        let mut current = safe_write_lock(&lock).map_err(|e| e.to_string())?;
        *current = settings;
    }

    Ok(())
}

// 验证设置的有效性
fn validate_settings(settings: &Settings) -> AppResult<()> {
    if settings.max_records < 50 || settings.max_records > 1000 {
        return Err(AppError::Config(
            "最大记录条数必须在50-1000之间".to_string(),
        ));
    }

    if settings.shortcut_key.is_empty() {
        return Err(AppError::Config("快捷键不能为空".to_string()));
    }

    // 验证快捷键格式
    if !is_valid_shortcut_format(&settings.shortcut_key) {
        return Err(AppError::Config("快捷键格式无效".to_string()));
    }

    Ok(())
}

// 验证快捷键格式
fn is_valid_shortcut_format(shortcut: &str) -> bool {
    let parts: Vec<&str> = shortcut.split('+').collect();
    if parts.len() < 2 || parts.len() > 3 {
        return false;
    }

    // 检查是否包含修饰键
    parts
        .iter()
        .any(|&part| matches!(part, "Ctrl" | "Shift" | "Alt" | "Meta"))
}

// 更新全局快捷键
async fn update_global_shortcut(shortcut: &str) -> AppResult<()> {
    let app_handle = CONTEXT.get::<AppHandle>();
    log::info!("更新全局快捷键:{}", shortcut);

    // 先取消注册所有快捷键
    let _ = app_handle.global_shortcut().unregister_all();

    // 解析快捷键字符串为Shortcut类型
    let shortcut_obj = parse_shortcut(shortcut);

    // 注册新的快捷键
    match app_handle.global_shortcut().on_shortcut(shortcut_obj, {
        let app_handle_clone = app_handle.clone();
        move |_app, shortcut_triggered, event| {
            log::debug!(
                "快捷键触发: {:?}, 状态: {:?}",
                shortcut_triggered,
                event.state()
            );
            if event.state() == tauri_plugin_global_shortcut::ShortcutState::Pressed {
                use tauri::Manager;
                if let Some(window) = app_handle_clone.get_webview_window("main") {
                    let _ = window.show();
                    let _ = window.set_focus();
                }
            }
        }
    }) {
        Ok(_) => {
            log::info!("更新全局快捷键成功:{}", shortcut);
            Ok(())
        }
        Err(e) => {
            log::error!("更新全局快捷键失败:{:?}", e);
            Err(AppError::GlobalShortcut(format!("快捷键注册失败: {}", e)))
        }
    }
}

// 设置开机自启
fn set_auto_start(auto_start: bool) -> AppResult<()> {
    let app_handle = CONTEXT.get::<AppHandle>();
    let autostart_manager = app_handle.autolaunch();

    match if auto_start {
        autostart_manager.enable()
    } else {
        autostart_manager.disable()
    } {
        Ok(_) => Ok(()),
        Err(e) => Err(AppError::Config(format!("开机自启设置失败: {}", e))),
    }
}

// 保存设置到文件
fn save_settings_to_file(settings: &Settings) -> AppResult<()> {
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

// 回滚设置
async fn rollback_settings(applied_settings: &[(&str, bool)]) -> AppResult<()> {
    let app_handle = CONTEXT.get::<AppHandle>();

    // 在 await 点之前获取当前设置
    let current_settings = {
        let lock = CONTEXT.get::<Arc<RwLock<Settings>>>().clone();
        let current = safe_read_lock(&lock)?;
        current.clone()
    };

    for (setting_type, _) in applied_settings {
        match *setting_type {
            "shortcut" => {
                // 恢复原快捷键
                let shortcut_obj = parse_shortcut(&current_settings.shortcut_key);
                if let Err(e) = app_handle.global_shortcut().on_shortcut(shortcut_obj, {
                    let app_handle_clone = app_handle.clone();
                    move |_app, shortcut_triggered, event| {
                        log::debug!(
                            "恢复快捷键触发: {:?}, 状态: {:?}",
                            shortcut_triggered,
                            event.state()
                        );
                        if event.state() == tauri_plugin_global_shortcut::ShortcutState::Pressed {
                            use tauri::Manager;
                            if let Some(window) = app_handle_clone.get_webview_window("main") {
                                let _ = window.show();
                                let _ = window.set_focus();
                            }
                        }
                    }
                }) {
                    log::error!("恢复快捷键失败: {}", e);
                }
            }
            "autostart" => {
                // 恢复原开机自启设置
                if let Err(e) = set_auto_start(current_settings.auto_start == 1) {
                    log::error!("恢复开机自启设置失败: {}", e);
                }
            }
            _ => {}
        }
    }

    Ok(())
}

// 验证快捷键是否可用
#[tauri::command]
pub async fn validate_shortcut(shortcut: String) -> Result<bool, String> {
    // 1. 验证格式
    if !is_valid_shortcut_format(&shortcut) {
        return Ok(false);
    }

    // 2. 获取当前设置的快捷键
    let current_shortcut = {
        let lock = CONTEXT.get::<Arc<RwLock<Settings>>>().clone();
        let result = match safe_read_lock(&lock) {
            Ok(current) => current.shortcut_key.clone(),
            Err(_) => String::new(),
        };
        result
    };

    // 3. 如果和当前设置一样，直接返回true（允许保存相同快捷键）
    if shortcut == current_shortcut {
        return Ok(true);
    }

    // 4. 尝试解析快捷键字符串验证其有效性
    let _shortcut_obj = match shortcut.parse::<tauri_plugin_global_shortcut::Shortcut>() {
        Ok(s) => s,
        Err(_) => {
            // 如果解析失败，使用自定义解析器
            parse_shortcut(&shortcut)
        }
    };

    // 5. 格式验证通过，返回true
    // 实际的冲突检测将在注册时进行
    Ok(true)
}

/// 检查是否开启了云同步功能
pub async fn check_cloud_sync_enabled() -> bool {
    let settings_lock = CONTEXT.get::<Arc<RwLock<Settings>>>();
    if let Ok(settings) = safe_read_lock(&settings_lock) {
        return settings.cloud_sync == 1;
    }
    false
}

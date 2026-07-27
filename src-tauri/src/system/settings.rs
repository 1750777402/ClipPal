use crate::{
    app_context::AppContext,
    errors::{AppError, AppResult},
    global_shortcut::{parse_shortcut_strict, register_shortcut_handler},
};
use tauri_plugin_autostart::ManagerExt;
use tauri_plugin_global_shortcut::GlobalShortcutExt;

/// 设置相关系统能力适配器，统一封装快捷键、自启动、权限和同步触发操作。
pub struct SettingsSystem<'a> {
    context: &'a AppContext,
}

impl<'a> SettingsSystem<'a> {
    /// 绑定应用上下文，后续系统操作从中取得 AppHandle 和运行状态。
    pub fn new(context: &'a AppContext) -> Self {
        Self { context }
    }

    /// 快速检查快捷键字符串是否具有“修饰键 + 主键”的基本结构。
    pub fn is_valid_shortcut_format(&self, shortcut: &str) -> bool {
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

    /// 使用系统快捷键解析器验证完整键位，成功时丢弃解析对象。
    pub fn parse_shortcut(&self, shortcut: &str) -> Result<(), String> {
        parse_shortcut_strict(shortcut).map(|_| ())
    }

    /// 注销当前全局快捷键并注册新的快捷键处理器。
    pub async fn update_shortcut(&self, shortcut: &str) -> AppResult<()> {
        let app_handle = self.context.app_handle()?;
        let shortcut = parse_shortcut_strict(shortcut)
            .map_err(|error| AppError::GlobalShortcut(format!("快捷键格式无效: {}", error)))?;

        let _ = app_handle.global_shortcut().unregister_all();
        register_shortcut_handler(&app_handle, crate::app_context::app_context()?, shortcut)
            .map_err(AppError::GlobalShortcut)
    }

    /// 通过系统自启动插件开启或关闭开机启动。
    pub fn set_auto_start(&self, enabled: bool) -> AppResult<()> {
        let app_handle = self.context.app_handle()?;
        let autostart_manager = app_handle.autolaunch();

        match if enabled {
            autostart_manager.enable()
        } else {
            autostart_manager.disable()
        } {
            Ok(_) => Ok(()),
            Err(error) => Err(AppError::Config(format!("开机自启动设置失败: {}", error))),
        }
    }
}

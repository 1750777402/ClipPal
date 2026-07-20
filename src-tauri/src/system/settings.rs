use crate::{
    app_context::AppContext,
    biz::{
        cloud_sync_timer::trigger_immediate_sync,
        system_setting::{
            is_valid_shortcut_format, rollback_settings, set_auto_start, update_global_shortcut,
            validate_cloud_sync_permission, validate_settings,
        },
    },
    domain::settings::Settings,
    errors::AppResult,
    global_shortcut::parse_shortcut_strict,
};

/// 设置相关系统能力适配器，统一封装快捷键、自启动、权限和同步触发操作。
pub struct SettingsSystem<'a> {
    context: &'a AppContext,
}

impl<'a> SettingsSystem<'a> {
    /// 绑定应用上下文，后续系统操作从中取得 AppHandle 和运行状态。
    pub fn new(context: &'a AppContext) -> Self {
        Self { context }
    }

    /// 校验设置业务规则，包括记录数限制和快捷键合法性。
    pub async fn validate(&self, settings: &Settings) -> AppResult<()> {
        validate_settings(settings).await
    }

    /// 快速检查快捷键字符串是否具有“修饰键 + 主键”的基本结构。
    pub fn is_valid_shortcut_format(&self, shortcut: &str) -> bool {
        is_valid_shortcut_format(shortcut)
    }

    /// 使用系统快捷键解析器验证完整键位，成功时丢弃解析对象。
    pub fn parse_shortcut(&self, shortcut: &str) -> Result<(), String> {
        parse_shortcut_strict(shortcut).map(|_| ())
    }

    /// 注销当前全局快捷键并注册新的快捷键处理器。
    pub async fn update_shortcut(&self, shortcut: &str) -> AppResult<()> {
        update_global_shortcut(self.context, shortcut).await
    }

    /// 检查登录状态和 VIP 权益是否允许开启云同步。
    pub async fn validate_cloud_sync_permission(&self) -> Result<(), String> {
        validate_cloud_sync_permission().await
    }

    /// 通过系统自启动插件开启或关闭开机启动。
    pub fn set_auto_start(&self, enabled: bool) -> AppResult<()> {
        set_auto_start(self.context, enabled)
    }

    /// 根据已应用项目恢复快捷键和自启动状态，供设置保存失败时使用。
    pub async fn rollback(&self, previous: &Settings, applied: &[&str]) -> AppResult<()> {
        rollback_settings(previous, applied).await
    }

    /// 向云同步后台任务发送立即执行信号。
    pub fn trigger_immediate_sync(&self) -> Result<(), &'static str> {
        trigger_immediate_sync()
    }
}

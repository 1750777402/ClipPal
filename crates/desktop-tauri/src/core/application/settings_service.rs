use {
    crate::app_context::AppContext,
    clip_core::domain::settings::Settings,
    crate::errors::AppError,
    crate::errors::AppResult,
    clip_core::repositories::SettingsRepository,
    crate::desktop::system::settings::SettingsSystem,
};


/// 设置页面的数据包含系统实际状态；运行期状态不写入配置文件。
#[derive(serde::Serialize)]
pub struct SettingsView {
    /// 直接展开已有设置字段，保持前端原来的平铺数据结构。
    #[serde(flatten)]
    pub settings: Settings,
    /// 系统中的真实注册结果，不能用配置字符串非空代替。
    pub shortcut_registered: bool,
    /// 系统实时权限状态，不写回 settings.json。
    pub auto_paste_permission: crate::desktop::system::accessibility::AutoPastePermission,
}

/// 设置应用服务，负责配置校验、系统能力应用、持久化和内存缓存更新。
pub struct SettingsService<'a> {
    context: &'a AppContext,
    repository: &'a dyn SettingsRepository,
    system: SettingsSystem<'a>,
}

impl<'a> SettingsService<'a> {
    /// 从应用上下文取得设置仓储和系统能力适配器。
    pub fn from_context(context: &'a AppContext) -> Self {
        Self {
            context,
            repository: context.repositories().settings(),
            system: SettingsSystem::new(context),
        }
    }
    /// 配置读取后核对系统状态；读取失败通过统一响应报告，避免显示错误状态。
    pub async fn load_settings(&self) -> AppResult<SettingsView> {
        let _save = self.context.core().settings_save_lock().lock().await;
        let mut settings = self.repository.load();
        settings.auto_start = u32::from(self.system.is_auto_start_enabled()?);
        let actual = self.system.registered_shortcut()?;
        if let Some(key) = &actual { settings.shortcut_key = key.clone(); }
        Ok(SettingsView { settings, shortcut_registered: actual.is_some(),
            auto_paste_permission: crate::desktop::system::accessibility::status() })
    }

    /// 保存完整设置，并保证系统设置、配置文件和内存缓存按顺序一致更新。
    pub async fn save_settings(&self, settings: Settings) -> AppResult<()> {
        let _save = self.context.core().settings_save_lock().lock().await;
        // 先完成记录数、快捷键和权限等业务校验，校验失败时不产生任何副作用。
        self.validate_settings(&settings).await?;

        // 保存实际系统快照；相同配置也执行注册检查，修复启动时未能注册的快捷键。
        let previous_shortcut = self.system.registered_shortcut()?;
        let previous_autostart = self.system.is_auto_start_enabled()?;
        crate::desktop::system::settings::apply_settings_transaction(
            previous_shortcut.as_deref(), &settings.shortcut_key,
            previous_autostart, settings.auto_start == 1,
            |key| self.system.update_shortcut(key),
            |enabled| self.system.set_auto_start(enabled),
            || self.repository.save(&settings),
        )?;

        // 配置文件写入成功后更新共享内存，使后台任务立即读取到新值。
        self.context
            .update_settings(|current| *current = settings)?;

        if let Err(error) = super::capture_listener::clean_records(self.context).await {
            log::error!("应用记录保留设置失败: {}", error);
        }
        if let Some(app) = self.context.try_app_handle() {
            use tauri::Emitter;
            let _ = app.emit("clip_record_change", ());
        }

        Ok(())
    }

    /// 校验候选快捷键的系统可用性，与保存串行，避免临时注册和保存互相干扰。
    pub async fn validate_shortcut(&self, shortcut: String) -> AppResult<bool> {
        let _save = self.context.core().settings_save_lock().lock().await;
        self.system.validate_shortcut(&shortcut)
    }

    /// 先按版本策略校验记录上限和开关，再校验系统快捷键格式，不产生副作用。
    async fn validate_settings(&self, settings: &Settings) -> AppResult<()> {
        let max_allowed = self.context.policy().max_records(self.context)
            .unwrap_or(300);

        desktop_app_core::settings::validate(settings, max_allowed)?;

        self.system
            .parse_shortcut(&settings.shortcut_key)
            .map_err(|error| {
                AppError::Config(format!(
                    "快捷键格式错误，请使用如 Ctrl+Shift+C 的组合键。{}",
                    error
                ))
            })
    }


}

#[cfg(test)]
mod tests {
    use super::*;

    /// 验证设置响应保持平铺，快捷键和权限运行状态不写入配置。
    #[test]
    fn settings_view_keeps_existing_flat_ipc_fields_without_persisting_runtime_status() {
        let view = SettingsView { settings: Settings::default(), shortcut_registered: false,
            auto_paste_permission: crate::desktop::system::accessibility::AutoPastePermission::Required };
        let response = serde_json::to_value(view).unwrap();
        assert_eq!(response["shortcut_registered"], false);
        assert_eq!(response["auto_paste_permission"], "required");
        assert!(response.get("settings").is_none());
        assert!(response.get("auto_start").is_some());
        let settings: Settings = serde_json::from_value(response).unwrap();
        let persisted = serde_json::to_value(settings).unwrap();
        assert!(persisted.get("shortcut_registered").is_none());
        assert!(persisted.get("auto_paste_permission").is_none());
    }
}

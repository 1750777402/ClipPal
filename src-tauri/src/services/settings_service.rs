use crate::{
    app_context::AppContext,
    domain::settings::Settings,
    infra::repositories::SettingsRepository,
    system::settings::SettingsSystem,
    utils::lock_utils::lock_utils::{safe_read_lock, safe_write_lock},
};

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
    /// 从配置文件仓储读取设置；仓储会在文件缺失或损坏时返回默认值。
    pub fn load_settings(&self) -> Settings {
        self.repository.load()
    }

    /// 保存完整设置，并保证系统设置、配置文件和内存缓存按顺序一致更新。
    pub async fn save_settings(&self, settings: Settings) -> Result<(), String> {
        // 先完成记录数、快捷键和权限等业务校验，校验失败时不产生任何副作用。
        self.system
            .validate(&settings)
            .await
            .map_err(|error| error.to_string())?;

        // 保存变更前快照，后续系统能力应用失败时用于回滚。
        let current_settings = {
            let lock = self.context.settings();
            let current = safe_read_lock(&lock).map_err(|error| error.to_string())?;
            current.clone()
        };

        // 记录已成功应用的系统设置，只回滚真正发生过的副作用。
        let mut applied_settings = Vec::new();

        // 快捷键注册属于系统副作用，必须先应用成功再持久化配置。
        if settings.shortcut_key != current_settings.shortcut_key {
            match self.system.update_shortcut(&settings.shortcut_key).await {
                Ok(_) => applied_settings.push("shortcut"),
                Err(error) => {
                    let _ = self
                        .system
                        .rollback(&current_settings, &applied_settings)
                        .await;
                    return Err(format!("快捷键设置失败: {}", error));
                }
            }
        }

        // 开启云同步前验证登录和 VIP 权限；关闭云同步不需要额外权限。
        if settings.cloud_sync != current_settings.cloud_sync && settings.cloud_sync == 1 {
            match self.system.validate_cloud_sync_permission().await {
                Ok(_) => applied_settings.push("cloud_sync"),
                Err(error) => {
                    let _ = self
                        .system
                        .rollback(&current_settings, &applied_settings)
                        .await;
                    return Err(format!("开启云同步失败: {}", error));
                }
            }
        }

        // 开机自启动通过 Tauri 系统插件应用，失败时恢复此前已修改的设置。
        if settings.auto_start != current_settings.auto_start {
            match self.system.set_auto_start(settings.auto_start == 1) {
                Ok(_) => applied_settings.push("autostart"),
                Err(error) => {
                    let _ = self
                        .system
                        .rollback(&current_settings, &applied_settings)
                        .await;
                    return Err(format!("开机自启动设置失败: {}", error));
                }
            }
        }

        // 所有系统副作用成功后再写文件，避免持久化不可应用的设置。
        if let Err(error) = self.repository.save(&settings) {
            let _ = self
                .system
                .rollback(&current_settings, &applied_settings)
                .await;
            return Err(format!("保存配置失败: {}", error));
        }

        let need_trigger_sync =
            settings.cloud_sync != current_settings.cloud_sync && settings.cloud_sync == 1;

        // 配置文件写入成功后更新共享内存，使后台任务立即读取到新值。
        {
            let lock = self.context.settings();
            let mut current = safe_write_lock(&lock).map_err(|error| error.to_string())?;
            *current = settings;
        }

        // 云同步刚被开启时主动唤醒同步任务，不等待下一次定时周期。
        if need_trigger_sync {
            if let Err(error) = self.system.trigger_immediate_sync() {
                log::warn!("触发立即云同步失败: {}", error);
            }
        }

        Ok(())
    }

    /// 验证候选快捷键，不注册快捷键也不修改当前配置。
    pub fn validate_shortcut(&self, shortcut: String) -> Result<bool, String> {
        if !self.system.is_valid_shortcut_format(&shortcut) {
            return Ok(false);
        }

        let current_shortcut = {
            let lock = self.context.settings();
            let shortcut = match safe_read_lock(&lock) {
                Ok(current) => current.shortcut_key.clone(),
                Err(_) => String::new(),
            };
            shortcut
        };

        if shortcut == current_shortcut {
            return Ok(true);
        }

        self.system.parse_shortcut(&shortcut)?;
        Ok(true)
    }
}

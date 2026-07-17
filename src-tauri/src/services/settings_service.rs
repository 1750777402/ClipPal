use crate::{
    app_context::AppContext,
    biz::{
        cloud_sync_timer::trigger_immediate_sync,
        system_setting::{
            is_valid_shortcut_format, rollback_settings, set_auto_start, update_global_shortcut,
            validate_cloud_sync_permission, validate_settings, Settings,
        },
    },
    global_shortcut::parse_shortcut_strict,
    infra::repositories::{FileSettingsRepository, SettingsRepository},
    utils::lock_utils::lock_utils::{safe_read_lock, safe_write_lock},
};

pub struct SettingsService<'a, R> {
    context: &'a AppContext,
    repository: R,
}

impl<'a> SettingsService<'a, FileSettingsRepository> {
    pub fn from_context(context: &'a AppContext) -> Self {
        Self {
            context,
            repository: FileSettingsRepository,
        }
    }
}

impl<R> SettingsService<'_, R>
where
    R: SettingsRepository,
{
    pub fn load_settings(&self) -> Settings {
        self.repository.load()
    }

    pub async fn save_settings(&self, settings: Settings) -> Result<(), String> {
        validate_settings(&settings)
            .await
            .map_err(|error| error.to_string())?;

        let current_settings = {
            let lock = self.context.settings();
            let current = safe_read_lock(&lock).map_err(|error| error.to_string())?;
            current.clone()
        };

        let mut applied_settings = Vec::new();

        if settings.shortcut_key != current_settings.shortcut_key {
            match update_global_shortcut(self.context, &settings.shortcut_key).await {
                Ok(_) => applied_settings.push("shortcut"),
                Err(error) => {
                    let _ = rollback_settings(&current_settings, &applied_settings).await;
                    return Err(format!("快捷键设置失败: {}", error));
                }
            }
        }

        if settings.cloud_sync != current_settings.cloud_sync && settings.cloud_sync == 1 {
            match validate_cloud_sync_permission().await {
                Ok(_) => applied_settings.push("cloud_sync"),
                Err(error) => {
                    let _ = rollback_settings(&current_settings, &applied_settings).await;
                    return Err(format!("开启云同步失败: {}", error));
                }
            }
        }

        if settings.auto_start != current_settings.auto_start {
            match set_auto_start(self.context, settings.auto_start == 1) {
                Ok(_) => applied_settings.push("autostart"),
                Err(error) => {
                    let _ = rollback_settings(&current_settings, &applied_settings).await;
                    return Err(format!("开机自启动设置失败: {}", error));
                }
            }
        }

        if let Err(error) = self.repository.save(&settings) {
            let _ = rollback_settings(&current_settings, &applied_settings).await;
            return Err(format!("保存配置失败: {}", error));
        }

        let need_trigger_sync =
            settings.cloud_sync != current_settings.cloud_sync && settings.cloud_sync == 1;

        {
            let lock = self.context.settings();
            let mut current = safe_write_lock(&lock).map_err(|error| error.to_string())?;
            *current = settings;
        }

        if need_trigger_sync {
            if let Err(error) = trigger_immediate_sync() {
                log::warn!("触发立即云同步失败: {}", error);
            }
        }

        Ok(())
    }

    pub fn validate_shortcut(&self, shortcut: String) -> Result<bool, String> {
        if !is_valid_shortcut_format(&shortcut) {
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

        parse_shortcut_strict(&shortcut)?;
        Ok(true)
    }
}

use std::{
    fs,
    path::{Path, PathBuf},
};

use crate::{
    domain::settings::Settings,
    errors::{AppError, AppResult},
    infra::repositories::SettingsRepository,
    utils::file_dir::get_config_dir,
};

#[derive(Default)]
/// 基于本地 JSON 配置文件的设置仓储实现。
pub struct FileSettingsRepository;

impl FileSettingsRepository {
    fn settings_file_path() -> Option<PathBuf> {
        get_config_dir().map(|config_dir| config_dir.join("settings.json"))
    }

    fn load_value(&self) -> Settings {
        let Some(path) = Self::settings_file_path() else {
            return Settings::default();
        };

        if !path.exists() {
            let settings = Settings::default();
            if let Err(error) = self.save(&settings) {
                log::warn!("创建默认配置文件失败: {}", error);
            }
            return settings;
        }

        Self::load_from_path(&path)
    }

    fn load_from_path(path: &Path) -> Settings {
        fs::read_to_string(path)
            .ok()
            .and_then(|data| serde_json::from_str(&data).ok())
            .unwrap_or_default()
    }

    fn save_to_path(path: &Path, settings: &Settings) -> AppResult<()> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }

        let json = serde_json::to_string_pretty(settings)
            .map_err(|error| AppError::Serde(error.to_string()))?;
        fs::write(path, json).map_err(AppError::Io)
    }
}

impl SettingsRepository for FileSettingsRepository {
    /// 从应用配置目录读取设置，沿用统一的默认配置回退策略。
    fn load(&self) -> Settings {
        self.load_value()
    }

    /// 将完整设置序列化并写入配置文件。
    fn save(&self, settings: &Settings) -> AppResult<()> {
        let path = Self::settings_file_path()
            .ok_or_else(|| AppError::Config("无法获取配置文件路径".to_string()))?;
        Self::save_to_path(&path, settings)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn isolated_settings_path() -> PathBuf {
        std::env::temp_dir()
            .join(format!("clip-pal-settings-test-{}", uuid::Uuid::new_v4()))
            .join("settings.json")
    }

    #[test]
    fn saves_and_loads_settings_without_changing_the_contract() {
        let path = isolated_settings_path();
        let mut settings = Settings::default();
        settings.max_records = 500;
        settings.shortcut_key = "Ctrl+Shift+V".to_string();
        settings.auto_paste = 0;

        FileSettingsRepository::save_to_path(&path, &settings).unwrap();
        let loaded = FileSettingsRepository::load_from_path(&path);

        assert_eq!(loaded.max_records, 500);
        assert_eq!(loaded.shortcut_key, "Ctrl+Shift+V");
        assert_eq!(loaded.auto_paste, 0);

        let _ = fs::remove_dir_all(path.parent().unwrap());
    }

    #[test]
    fn invalid_json_falls_back_to_default_settings() {
        let path = isolated_settings_path();
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, "not-json").unwrap();

        let loaded = FileSettingsRepository::load_from_path(&path);
        let defaults = Settings::default();

        assert_eq!(loaded.max_records, defaults.max_records);
        assert_eq!(loaded.shortcut_key, defaults.shortcut_key);
        assert_eq!(loaded.cloud_sync, defaults.cloud_sync);

        let _ = fs::remove_dir_all(path.parent().unwrap());
    }
}

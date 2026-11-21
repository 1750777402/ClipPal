use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
};

use crate::{
    domain::settings::Settings,
    errors::{AppError, AppResult},
    repositories::SettingsRepository,
};

#[derive(Default)]
/// 基于本地 JSON 配置文件的设置仓储实现。
pub struct FileSettingsRepository {
    path: Option<PathBuf>,
}

impl FileSettingsRepository {
    /// 路径由桌面组装层注入，None 保留原来的默认配置回退行为。
    pub fn new(path: Option<PathBuf>) -> Self {
        Self { path }
    }

    /// 读取已配置的设置文件；缺失时尝试保存默认值，失败仍使用内存默认设置。
    fn load_value(&self) -> Settings {
        let Some(path) = self.path.clone() else {
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

    /// 解析并修正无效设置，防止损坏的保留数量触发启动时误删历史。
    fn load_from_path(path: &Path) -> Settings {
        let mut settings: Settings = match fs::read_to_string(path)
            .map_err(AppError::Io).and_then(|data| serde_json::from_str(&data).map_err(AppError::from)) {
            Ok(settings) => settings,
            Err(error) => {
                log::warn!("读取设置失败，使用默认设置: {}", error);
                return Settings::default();
            }
        };
        // 损坏的设置不得在启动清理阶段删除全部历史记录。
        let defaults = Settings::default();
        if settings.max_records < 50 { settings.max_records = defaults.max_records; }
        settings.max_records = settings.max_records.min(i32::MAX as u32);
        if settings.shortcut_key.trim().is_empty() { settings.shortcut_key = defaults.shortcut_key; }
        if settings.auto_start > 1 { settings.auto_start = defaults.auto_start; }
        if settings.auto_paste > 1 { settings.auto_paste = defaults.auto_paste; }
        if settings.tutorial_completed > 1 { settings.tutorial_completed = defaults.tutorial_completed; }
        settings
    }

    /// 先写同目录临时文件并落盘，再原子替换正式设置，失败保留旧文件。
    fn save_to_path(path: &Path, settings: &Settings) -> AppResult<()> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }

        let json = serde_json::to_string_pretty(settings)
            .map_err(|error| AppError::Serde(error.to_string()))?;
        // 同目录临时文件写入完成后原子替换；写入失败时保留上一份有效设置。
        let parent = path.parent().filter(|path| !path.as_os_str().is_empty()).unwrap_or(Path::new("."));
        let mut temporary = tempfile::NamedTempFile::new_in(parent)?;
        temporary.write_all(json.as_bytes())?;
        temporary.as_file().sync_all()?;
        temporary.persist(path).map_err(|error| AppError::Io(error.error))?;
        Ok(())
    }
}

impl SettingsRepository for FileSettingsRepository {
    /// 从应用配置目录读取设置，沿用统一的默认配置回退策略。
    fn load(&self) -> Settings {
        self.load_value()
    }

    /// 将完整设置序列化并写入配置文件。
    fn save(&self, settings: &Settings) -> AppResult<()> {
        let path = self.path.clone()
            .ok_or_else(|| AppError::Config("无法获取配置文件路径".to_string()))?;
        Self::save_to_path(&path, settings)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 生成测试专用设置路径，不读取或修改用户实际配置。
    fn isolated_settings_path() -> PathBuf {
        std::env::temp_dir()
            .join(format!("clip-pal-settings-test-{}", uuid::Uuid::new_v4()))
            .join("settings.json")
    }

    /// 验证设置保存后可原样读取，快捷键和用户开关不会丢失。
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

    /// 验证损坏 JSON 使用默认设置，不中断应用启动。
    #[test]
    fn invalid_json_falls_back_to_default_settings() {
        let path = isolated_settings_path();
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, "not-json").unwrap();

        let loaded = FileSettingsRepository::load_from_path(&path);
        let defaults = Settings::default();

        assert_eq!(loaded.max_records, defaults.max_records);
        assert_eq!(loaded.shortcut_key, defaults.shortcut_key);

        let _ = fs::remove_dir_all(path.parent().unwrap());
    }
}

#[cfg(test)]
mod stability_tests {
    use super::*;
    /// 验证旧设置缺少字段时保留已有值，非法保留数量不会清空历史。
    #[test]
    fn partial_old_settings_preserve_user_values_and_invalid_retention_is_safe() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        fs::write(&path, r#"{"max_records":500,"auto_paste":0}"#).unwrap();
        let settings = FileSettingsRepository::load_from_path(&path);
        assert_eq!(settings.max_records, 500);
        assert_eq!(settings.auto_paste, 0);
        assert_eq!(settings.shortcut_key, Settings::default().shortcut_key);
        fs::write(&path, r#"{"max_records":0,"auto_paste":42}"#).unwrap();
        let settings = FileSettingsRepository::load_from_path(&path);
        assert_eq!(settings.max_records, Settings::default().max_records);
        assert_eq!(settings.auto_paste, Settings::default().auto_paste);
    }
    /// 验证连续保存可替换原设置，且不会遗留临时配置文件。
    #[test]
    fn repeatedly_replaces_existing_settings_without_leaving_temporary_files() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        for max_records in [200, 500, 1000] {
            let settings = Settings { max_records, ..Settings::default() };
            FileSettingsRepository::save_to_path(&path, &settings).unwrap();
            assert_eq!(FileSettingsRepository::load_from_path(&path).max_records, max_records);
        }
        assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 1);
    }
}

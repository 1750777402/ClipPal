use crate::{
    biz::system_setting::{load_settings_value, save_settings_to_file},
    domain::settings::Settings,
    errors::AppResult,
    infra::repositories::SettingsRepository,
};

#[derive(Default)]
/// 基于本地 JSON 配置文件的设置仓储实现。
pub struct FileSettingsRepository;

impl SettingsRepository for FileSettingsRepository {
    /// 从应用配置目录读取设置，沿用统一的默认配置回退策略。
    fn load(&self) -> Settings {
        load_settings_value()
    }

    /// 将完整设置序列化并写入配置文件。
    fn save(&self, settings: &Settings) -> AppResult<()> {
        save_settings_to_file(settings)
    }
}

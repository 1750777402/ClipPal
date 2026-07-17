use crate::{
    biz::system_setting::{load_settings_value, save_settings_to_file, Settings},
    errors::AppResult,
    infra::repositories::SettingsRepository,
};

#[derive(Default)]
pub struct FileSettingsRepository;

impl SettingsRepository for FileSettingsRepository {
    fn load(&self) -> Settings {
        load_settings_value()
    }

    fn save(&self, settings: &Settings) -> AppResult<()> {
        save_settings_to_file(settings)
    }
}

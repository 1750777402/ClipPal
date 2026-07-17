use crate::{biz::system_setting::Settings, errors::AppResult};

pub trait SettingsRepository: Send + Sync {
    fn load(&self) -> Settings;

    fn save(&self, settings: &Settings) -> AppResult<()>;
}

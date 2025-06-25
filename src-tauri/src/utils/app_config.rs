use config::Config;
use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct AppConfig {
    pub content_key: String,
}
pub fn load_config() -> Result<AppConfig, config::ConfigError> {
    let config = Config::builder()
        .add_source(config::File::with_name("config"))
        .build()?;
    config.try_deserialize::<AppConfig>()
}

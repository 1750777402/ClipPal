use crate::errors::{AppError, AppResult};
use std::sync::OnceLock;
use serde::{Deserialize, Serialize};

/// 应用配置结构
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct AppConfig {
    pub app_secret: AppSecret,
    #[cfg(feature = "sync")]
    pub cloud_sync: CloudSync,
}

/// 应用密钥配置
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct AppSecret {
    pub secret_key: String,
}

#[derive(Deserialize, Serialize, Clone)]
#[cfg(feature = "sync")]
pub struct CloudSync {
    #[serde(default, skip_serializing)]
    pub deployment_token: Option<String>,
    pub domain: String,
    pub max_file_size_kb: u64,
}

/// 已解析的配置及托盘资源只属于当前进程，不读取宿主文件。
struct RuntimeConfig {
    app: AppConfig,
    tray_icon_png: &'static [u8],
}
static GLOBAL_CONFIG: OnceLock<RuntimeConfig> = OnceLock::new();

pub(crate) fn initialize(assets: crate::PackagedAssets) -> AppResult<()> {
    let app = serde_json::from_str(assets.config_json)
        .map_err(|error| AppError::Config(format!("解析打包配置失败: {}", error)))?;
    GLOBAL_CONFIG.set(RuntimeConfig { app, tray_icon_png: assets.tray_icon_png })
        .map_err(|_| AppError::Config("打包配置已初始化，不能重复设置".into()))
}

pub(crate) fn tray_icon_png() -> AppResult<&'static [u8]> {
    GLOBAL_CONFIG.get().map(|config| config.tray_icon_png)
        .ok_or_else(|| AppError::Config("打包资源尚未初始化".into()))
}

/// 获取全局缓存的配置
pub fn get_global_config() -> AppResult<&'static AppConfig> {
    GLOBAL_CONFIG.get().map(|config| &config.app)
        .ok_or_else(|| AppError::Config("打包配置尚未初始化".into()))
}

/// 获取全局缓存的密钥
pub fn get_global_secret() -> AppResult<&'static AppSecret> {
    let config = get_global_config()?;
    Ok(&config.app_secret)
}

/// 获取全局缓存的内容密钥
pub fn get_global_secret_key() -> AppResult<&'static str> {
    let secret = get_global_secret()?;
    Ok(&secret.secret_key)
}

/// 获取全局缓存的云同步配置
#[cfg(feature = "sync")]
pub fn get_cloud_sync() -> AppResult<&'static CloudSync> {
    let config = get_global_config()?;
    Ok(&config.cloud_sync)
}

/// 获取全局缓存的云同步域名
#[cfg(feature = "sync")]
pub fn get_cloud_sync_domain() -> AppResult<&'static str> {
    let secret = get_cloud_sync()?;
    Ok(&secret.domain)
}

/// 获取全局缓存的文件大小限制（字节）
#[cfg(feature = "sync")]
pub fn get_max_file_size_bytes() -> AppResult<u64> {
    let cloud_sync = get_cloud_sync()?;
    cloud_sync.max_file_size_kb.checked_mul(1024)
        .ok_or_else(|| AppError::Config("同步文件大小配置超出范围".into()))
}

#[cfg(feature = "sync")]
impl std::fmt::Debug for CloudSync {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CloudSync").field("domain", &self.domain)
            .field("max_file_size_kb", &self.max_file_size_kb)
            .field("deployment_token", &"[REDACTED]").finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn injected_assets_initialize_once_and_invalid_input_leaves_no_partial_state() {
        let make_assets = |config_json| crate::PackagedAssets {
            config_json,
            tray_icon_png: b"test-icon-bytes",
        };
        assert!(get_global_config().is_err());
        assert!(initialize(make_assets("invalid-json")).is_err());
        assert!(tray_icon_png().is_err());

        let config = r#"{"app_secret":{"secret_key":"test-packaged-value"},"cloud_sync":{"domain":"https://sync.test","max_file_size_kb":1024}}"#;
        initialize(make_assets(config)).unwrap();
        assert_eq!(get_global_secret_key().unwrap(), "test-packaged-value");
        assert_eq!(tray_icon_png().unwrap(), b"test-icon-bytes");
        assert!(initialize(make_assets(config)).is_err());
        assert_eq!(get_global_secret_key().unwrap(), "test-packaged-value");
    }

    #[test]
    #[cfg(feature = "sync")]
    fn deployment_token_is_optional_and_never_serialized_or_debugged() {
        let old: CloudSync = serde_json::from_str(r#"{"domain":"https://sync.test","max_file_size_kb":100}"#).unwrap();
        assert!(old.deployment_token.is_none());
        let configured: CloudSync = serde_json::from_str(r#"{"domain":"https://sync.test","max_file_size_kb":100,"deployment_token":"test-private-token"}"#).unwrap();
        assert!(!format!("{configured:?}").contains("test-private-token"));
        let value = serde_json::to_value(configured).unwrap();
        assert!(value.get("deployment_token").is_none());
    }

    #[test]
    #[cfg(not(feature = "sync"))]
    fn standalone_config_does_not_require_cloud_settings() {
        let config: AppConfig = serde_json::from_str(r#"{"app_secret":{"secret_key":"test"}}"#).unwrap();
        assert_eq!(config.app_secret.secret_key, "test");
    }
}

use crate::errors::{AppError, AppResult};
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};

/// 应用配置结构
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct AppConfig {
    pub app_secret: AppSecret,
    pub cloud_sync: CloudSync,
}

/// 应用密钥配置
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct AppSecret {
    pub content_key: String,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct CloudSync {
    pub domain: String,
}

/// 配置管理器
pub struct ConfigManager;

impl ConfigManager {
    /// 获取应用配置
    pub fn get_app_config() -> AppResult<AppConfig> {
        // 使用include_str!内嵌配置文件
        let config_content = include_str!("../../config.json");
        // 解析JSON配置
        let app_config: AppConfig = serde_json::from_str(config_content)
            .map_err(|e| AppError::Config(format!("解析配置文件失败: {}", e)))?;

        Ok(app_config)
    }
}

/// 全局配置缓存
static GLOBAL_CONFIG: Lazy<AppResult<AppConfig>> = Lazy::new(|| ConfigManager::get_app_config());

/// 获取全局缓存的配置
pub fn get_global_config() -> AppResult<&'static AppConfig> {
    GLOBAL_CONFIG
        .as_ref()
        .map_err(|e| AppError::Config(e.to_string()))
}

/// 获取全局缓存的密钥
pub fn get_global_secret() -> AppResult<&'static AppSecret> {
    let config = get_global_config()?;
    Ok(&config.app_secret)
}

/// 获取全局缓存的内容密钥
pub fn get_global_content_key() -> AppResult<&'static str> {
    let secret = get_global_secret()?;
    Ok(&secret.content_key)
}

/// 获取全局缓存的云同步配置
pub fn get_cloud_sync() -> AppResult<&'static CloudSync> {
    let config = get_global_config()?;
    Ok(&config.cloud_sync)
}

/// 获取全局缓存的云同步域名
pub fn get_cloud_sync_domain() -> AppResult<&'static str> {
    let secret = get_cloud_sync()?;
    Ok(&secret.domain)
}

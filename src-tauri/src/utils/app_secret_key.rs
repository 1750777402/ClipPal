use crate::errors::{AppError, AppResult};
use crate::utils::config::get_global_secret_key;
use base64::{engine::general_purpose, Engine as _};
use once_cell::sync::Lazy;
use serde::Deserialize;

#[derive(Debug, Deserialize, Clone)]
pub struct AppSecretKey {
    pub secret_key: String,
}

// 全局静态变量，只读一次配置文件
static GLOBAL_APP_SECRET_KEY: Lazy<AppSecretKey> = Lazy::new(|| {
    // 使用配置管理器获取密钥
    match get_global_secret_key() {
        Ok(secret_key) => AppSecretKey {
            secret_key: secret_key.to_string(),
        },
        Err(e) => {
            log::warn!("读取配置文件失败: {}", e);
            // 正常不会走到这里，只是一个兜底
            AppSecretKey {
                secret_key: "jW8QgaaT7QH5T8bZg4IYOk099gCbU2JzrhC+P+Zy94d=".to_string(),
            }
        }
    }
});

// 获取密钥的统一入口
pub fn get_app_secret_key() -> AppResult<AppSecretKey> {
    Ok(GLOBAL_APP_SECRET_KEY.clone())
}

/// 解混淆密钥 - 简单的字符替换 + base64
fn decode_obfuscated_key(obfuscated: &str) -> AppResult<String> {
    // 步骤1: 字符替换还原 (简单的替换混淆)
    let step1 = obfuscated
        .replace('j', "u") // j -> u
        .replace('W', "V") // W -> V
        .replace('Q', "P") // Q -> P
        .replace('I', "H") // I -> H
        .replace('C', "B") // C -> B
        .replace('J', "I") // J -> I
        .replace('U', "T") // U -> T
        .replace('d', "c"); // d -> c

    // 步骤2: Base64解码验证
    match general_purpose::STANDARD.decode(&step1) {
        Ok(_) => Ok(step1),
        Err(e) => Err(AppError::Crypto(format!("Base64解码验证失败: {}", e))),
    }
}

/// 获取解混淆后的真实密钥
pub fn get_decoded_secret_key() -> AppResult<AppSecretKey> {
    let app_secret = get_app_secret_key()?;
    let decoded_key = decode_obfuscated_key(&app_secret.secret_key)?;

    Ok(AppSecretKey {
        secret_key: decoded_key,
    })
}

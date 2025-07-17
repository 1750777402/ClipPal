use crate::errors::{AppError, AppResult};
use base64::{Engine as _, engine::general_purpose};
use once_cell::sync::Lazy;
use serde::Deserialize;

#[derive(Debug, Deserialize, Clone)]
pub struct AppSecretKey {
    pub content_key: String,
}

// 混淆后的密钥 - 使用include_str!内嵌到可执行文件中
const OBFUSCATED_KEY: &str = include_str!("../../config.json");

// 全局静态变量，只读一次配置文件
static GLOBAL_APP_SECRET_KEY: Lazy<AppSecretKey> = Lazy::new(|| {
    // 解析内嵌的配置文件
    match serde_json::from_str::<serde_json::Value>(OBFUSCATED_KEY) {
        Ok(json) => {
            if let Some(content_key) = json["app_secret"]["content_key"].as_str() {
                log::info!("配置文件读取成功: {}", content_key);
                return AppSecretKey {
                    content_key: content_key.to_string(),
                };
            }
        }
        Err(e) => {
            log::warn!("解析内嵌配置文件失败: {}", e);
        }
    }

    // 如果配置文件解析失败，使用默认密钥
    log::warn!("使用默认密钥");
    AppSecretKey {
        // 正常不会走到这里，只是一个兜底
        content_key: "jW8QgaaT7QH5T8bZg4IYOk099gCbU2JzrhC+P+Zy94d=".to_string(),
    }
});

// 获取密钥的统一入口
pub fn get_app_secret_key() -> AppResult<AppSecretKey> {
    Ok(GLOBAL_APP_SECRET_KEY.clone())
}

/// 解混淆密钥 - 简单的字符替换 + base64
fn decode_obfuscated_key(obfuscated: &str) -> Result<String, String> {
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
        Err(e) => Err(format!("Base64解码验证失败: {}", e)),
    }
}

/// 获取解混淆后的真实密钥
pub fn get_decoded_secret_key() -> AppResult<AppSecretKey> {
    let app_secret = get_app_secret_key()?;
    let decoded_key = decode_obfuscated_key(&app_secret.content_key)
        .map_err(|e| AppError::Config(format!("解码密钥失败: {}", e)))?;

    Ok(AppSecretKey {
        content_key: decoded_key,
    })
}

/// 混淆密钥的辅助函数（开发时使用，生产时可以删除）
#[allow(dead_code)]
fn obfuscate_key(original_key: &str) -> String {
    // 字符替换混淆
    original_key
        .replace('u', "j") // u -> j
        .replace('V', "W") // V -> W
        .replace('P', "Q") // P -> Q
        .replace('H', "I") // H -> I
        .replace('B', "C") // B -> C
        .replace('I', "J") // I -> J
        .replace('T', "U") // T -> U
        .replace('c', "d") // c -> d
}

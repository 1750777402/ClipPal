use crate::errors::{AppError, AppResult};
use base64::{Engine as _, engine::general_purpose};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct AppSecretKey {
    pub content_key: String,
}

// 混淆后的密钥 - 不是直接的base64，而是经过简单变换的
const OBFUSCATED_KEY: &str = "jW8QgaaT7QH5T8bZg4IYOk099gCbU2JzrhC+P+Zy94d=";

pub fn load_config() -> AppResult<AppSecretKey> {
    // 解混淆获取真实密钥
    let content_key = decode_obfuscated_key(OBFUSCATED_KEY)
        .map_err(|e| AppError::Config(format!("解码密钥失败: {}", e)))?;

    Ok(AppSecretKey { content_key })
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

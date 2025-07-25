use aes_gcm::aead::{Aead, KeyInit};
use aes_gcm::{Aes256Gcm, Key, Nonce};
use base64::{Engine as _, engine::general_purpose};
use rand::TryRngCore;
use rand::rngs::OsRng;

use crate::{
    errors::{AppError, AppResult},
    utils::app_secret_key::get_decoded_secret_key,
};

const KEY_SIZE: usize = 32; // 256-bit
const NONCE_SIZE: usize = 12;

/// 内容加密
pub fn encrypt_content(content: &str) -> AppResult<String> {
    // 加载配置
    let app_config = get_decoded_secret_key()?;

    let decode_res = decode_base64_key(&app_config.secret_key)
        .map_err(|e| AppError::Crypto(format!("密钥解码失败: {}", e)))?;

    let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(&decode_res));

    let mut nonce_bytes = [0u8; NONCE_SIZE];
    OsRng
        .try_fill_bytes(&mut nonce_bytes)
        .map_err(|e| AppError::Crypto(format!("生成随机数失败: {}", e)))?;

    let nonce = Nonce::from_slice(&nonce_bytes);

    let ciphertext = cipher
        .encrypt(nonce, content.as_bytes())
        .map_err(|e| AppError::Crypto(format!("加密失败: {}", e)))?;

    // 拼接 nonce + ciphertext
    let mut result = Vec::new();
    result.extend_from_slice(&nonce_bytes);
    result.extend_from_slice(&ciphertext);

    Ok(general_purpose::STANDARD.encode(result))
}

/// 内容解密
pub fn decrypt_content(encoded: &str) -> AppResult<String> {
    // 加载配置
    let app_config = get_decoded_secret_key()?;

    let decode_res = decode_base64_key(&app_config.secret_key)
        .map_err(|e| AppError::Crypto(format!("密钥解码失败: {}", e)))?;

    let data = general_purpose::STANDARD
        .decode(encoded)
        .map_err(|e| AppError::Crypto(format!("Base64解码失败: {}", e)))?;

    if data.len() < NONCE_SIZE {
        return Err(AppError::Crypto("数据长度不足".to_string()));
    }

    let (nonce_bytes, ciphertext) = data.split_at(NONCE_SIZE);
    let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(&decode_res));
    let nonce = Nonce::from_slice(nonce_bytes);

    let decrypted_bytes = cipher
        .decrypt(nonce, ciphertext)
        .map_err(|e| AppError::Crypto(format!("解密失败: {}", e)))?;

    String::from_utf8(decrypted_bytes)
        .map_err(|e| AppError::Crypto(format!("UTF-8转换失败: {}", e)))
}

#[allow(dead_code)]
fn generate_global_aes_gcm_key() -> String {
    let mut key = [0u8; KEY_SIZE]; // 32字节 = 256位
    let _ = OsRng.try_fill_bytes(&mut key); // 使用操作系统提供的随机源填充
    general_purpose::STANDARD.encode(&key)
}

// 解密base64字符串   获得秘钥
fn decode_base64_key(base64_str: &str) -> anyhow::Result<[u8; KEY_SIZE]> {
    let bytes = general_purpose::STANDARD.decode(base64_str)?;
    let array: [u8; KEY_SIZE] = bytes
        .try_into()
        .map_err(|_| anyhow::anyhow!("密钥长度错误"))?;
    Ok(array)
}

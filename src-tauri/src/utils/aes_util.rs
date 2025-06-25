use aes_gcm::aead::{Aead, KeyInit};
use aes_gcm::{Aes256Gcm, Key, Nonce};
use anyhow::Error;
use base64::{Engine as _, engine::general_purpose};
use rand::TryRngCore;
use rand::rngs::OsRng;

const KEY_SIZE: usize = 32; // 256-bit
const NONCE_SIZE: usize = 12;

/// 粘贴板内容加密解密工具类
pub struct ClipboardAesUtil;

impl ClipboardAesUtil {
    /// 内容加密
    pub fn encrypt_content(content: &str, key: &str) -> Result<String, Error> {
        let decode_res = decode_base64_key(key)?;
        let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(&decode_res));
        let mut nonce_bytes = [0u8; NONCE_SIZE];
        let _ = OsRng.try_fill_bytes(&mut nonce_bytes);
        let nonce = Nonce::from_slice(&nonce_bytes);

        let ciphertext = cipher.encrypt(nonce, content.as_bytes()).expect("加密失败");

        // 拼接 nonce + ciphertext
        let mut result = Vec::new();
        result.extend_from_slice(&nonce_bytes);
        result.extend_from_slice(&ciphertext);

        Ok(general_purpose::STANDARD.encode(result))
    }

    /// 内容解密
    pub fn decrypt_content(encoded: &str, key: &str) -> Option<String> {
        let decode_res = decode_base64_key(key);
        if decode_res.is_err() {
            return None;
        }
        let data = general_purpose::STANDARD.decode(encoded).ok()?;
        if data.len() < NONCE_SIZE {
            return None;
        }

        let (nonce_bytes, ciphertext) = data.split_at(NONCE_SIZE);
        let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(&decode_res.unwrap()));
        let nonce = Nonce::from_slice(nonce_bytes);

        cipher
            .decrypt(nonce, ciphertext)
            .ok()
            .and_then(|bytes| String::from_utf8(bytes).ok())
    }
}

#[allow(dead_code)]
fn generate_global_aes_gcm_key() -> String {
    let mut key = [0u8; KEY_SIZE]; // 32字节 = 256位
    let _ = OsRng.try_fill_bytes(&mut key); // 使用操作系统提供的随机源填充
    general_purpose::STANDARD.encode(&key)
}

fn decode_base64_key(base64_str: &str) -> anyhow::Result<[u8; KEY_SIZE]> {
    let bytes = general_purpose::STANDARD.decode(base64_str)?;
    let array: [u8; KEY_SIZE] = bytes
        .try_into()
        .map_err(|_| anyhow::anyhow!("密钥长度错误"))?;
    Ok(array)
}

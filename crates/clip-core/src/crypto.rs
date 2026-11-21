//! 参数化的本地 AES-256-GCM，保持既有密文格式，不读取应用配置。
use aes_gcm::{Aes256Gcm, Key, Nonce, aead::{Aead, KeyInit}};
use base64::{engine::general_purpose, Engine as _};
use rand::{rngs::OsRng, TryRngCore};
use crate::errors::{AppError, AppResult};
const KEY_SIZE: usize = 32; // 256-bit
const NONCE_SIZE: usize = 12;

pub struct ContentCipher { cipher: Aes256Gcm }
impl ContentCipher {
    /// 解密base64字符串，获得秘钥。密钥来自调用方，核心不保存打包配置。
    pub fn from_base64_key(key: &str) -> AppResult<Self> {
        let bytes = general_purpose::STANDARD.decode(key)?;
        let key: [u8; KEY_SIZE] = bytes.try_into().map_err(|_| AppError::Crypto("密钥长度错误".into()))?;
        Ok(Self { cipher: Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(&key)) })
    }
    /// 内容加密
    pub fn encrypt(&self, content: &str) -> AppResult<String> {
        let mut nonce_bytes = [0u8; NONCE_SIZE];
        OsRng.try_fill_bytes(&mut nonce_bytes).map_err(|error| AppError::Crypto(format!("生成随机数失败: {}", error)))?;
        let encrypted = self.cipher.encrypt(Nonce::from_slice(&nonce_bytes), content.as_bytes())
            .map_err(|error| AppError::Crypto(format!("加密失败: {}", error)))?;
        // 拼接 nonce + ciphertext
        let mut output = nonce_bytes.to_vec();
        output.extend_from_slice(&encrypted);
        Ok(general_purpose::STANDARD.encode(output))
    }
    /// 内容解密
    pub fn decrypt(&self, encoded: &str) -> AppResult<String> {
        let data = general_purpose::STANDARD.decode(encoded).map_err(|error| AppError::Crypto(format!("Base64解码失败: {}", error)))?;
        if data.len() < NONCE_SIZE { return Err(AppError::Crypto("数据长度不足".into())); }
        let (nonce, ciphertext) = data.split_at(NONCE_SIZE);
        let plaintext = self.cipher.decrypt(Nonce::from_slice(nonce), ciphertext)
            .map_err(|error| AppError::Crypto(format!("解密失败: {}", error)))?;
        String::from_utf8(plaintext).map_err(|error| AppError::Crypto(format!("UTF-8转换失败: {}", error)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    const ZERO_KEY: &str = "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=";

    #[test]
    fn decrypts_legacy_nonce_ciphertext_tag_layout() {
        // AES-256-GCM 已知向量：全零 key / nonce / 16 字节明文。
        // 固定预期密文验证旧格式兼容，避免仅用新代码自加密自解密。
        let mut legacy = vec![0u8; 12];
        legacy.extend_from_slice(&[
            0xce, 0xa7, 0x40, 0x3d, 0x4d, 0x60, 0x6b, 0x6e,
            0x07, 0x4e, 0xc5, 0xd3, 0xba, 0xf3, 0x9d, 0x18,
            0xd0, 0xd1, 0xc8, 0xa7, 0x99, 0x99, 0x6b, 0xf0,
            0x26, 0x5b, 0x98, 0xb5, 0xd4, 0x8a, 0xb9, 0x19,
        ]);
        let cipher = ContentCipher::from_base64_key(ZERO_KEY).unwrap();
        assert_eq!(cipher.decrypt(&general_purpose::STANDARD.encode(legacy)).unwrap(), "\0".repeat(16));
    }

    #[test]
    fn unicode_roundtrip_random_nonce_and_tamper_detection() {
        let cipher = ContentCipher::from_base64_key(ZERO_KEY).unwrap();
        let text = "中文剪贴板🙂\nsecond line";
        let encrypted = cipher.encrypt(text).unwrap();
        assert_ne!(encrypted, cipher.encrypt(text).unwrap());
        assert_eq!(cipher.decrypt(&encrypted).unwrap(), text);
        let mut bytes = general_purpose::STANDARD.decode(encrypted).unwrap();
        *bytes.last_mut().unwrap() ^= 1;
        assert!(cipher.decrypt(&general_purpose::STANDARD.encode(bytes)).is_err());
        assert!(cipher.decrypt("invalid").is_err());
        assert!(ContentCipher::from_base64_key("AA==").is_err());
    }
}

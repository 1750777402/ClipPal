//! 桌面层提供既有打包密钥；实际算法在 clip-core，密文格式不变。
use crate::{errors::AppResult, desktop::secret_key::get_decoded_secret_key};
use clip_core::crypto::ContentCipher;

pub fn encrypt_content(content: &str) -> AppResult<String> {
    ContentCipher::from_base64_key(&get_decoded_secret_key()?.secret_key)?.encrypt(content)
}
pub fn decrypt_content(content: &str) -> AppResult<String> {
    ContentCipher::from_base64_key(&get_decoded_secret_key()?.secret_key)?.decrypt(content)
}

use once_cell::sync::Lazy;
use std::sync::RwLock;
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use serde::{Serialize, de::DeserializeOwned};
use crate::errors::{AppError, AppResult};
use crate::utils::file_dir::get_data_dir;
use crate::utils::aes_util::{encrypt_content, decrypt_content};
use base64::engine::general_purpose::STANDARD;
use base64::Engine;

const STORE_FILE: &str = "clipPal_store.bin";

/// 支持任意类型加密存储的工具类，所有内容存储在一个文件，带内存缓存
pub struct SecureStore {
    dir: PathBuf,
    cache: HashMap<String, Vec<u8>>, // key->原始二进制数据
    loaded: bool,
}

impl SecureStore {
    /// 自动获取存储目录
    pub fn new() -> AppResult<Self> {
        let dir = get_data_dir().ok_or(AppError::Config("无法获取配置目录".to_string()))?;
        Ok(Self { dir, cache: HashMap::new(), loaded: false })
    }

    /// 加载文件到内存（只加载一次）
    fn load_from_file(&mut self) -> AppResult<()> {
        if self.loaded { return Ok(()); }
        let file_path = self.dir.join(STORE_FILE);
        if !file_path.exists() {
            self.loaded = true;
            return Ok(());
        }
        let encrypted = fs::read_to_string(&file_path).map_err(AppError::Io)?;
        let decrypted = decrypt_content(&encrypted)?;
        let decoded = STANDARD.decode(&decrypted)
            .map_err(|e| AppError::Crypto(format!("Base64解码失败: {}", e)))?;
        self.cache = bincode::deserialize(&decoded)
            .map_err(|e| AppError::Serde(e.to_string()))?;
        self.loaded = true;
        Ok(())
    }

    /// 持久化内存到文件
    fn save_to_file(&self) -> AppResult<()> {
        let file_path = self.dir.join(STORE_FILE);
        let serialized = bincode::serialize(&self.cache)
            .map_err(|e| AppError::Serde(e.to_string()))?;
        let encoded = STANDARD.encode(&serialized);
        let encrypted = encrypt_content(&encoded)?;
        fs::write(&file_path, &encrypted).map_err(AppError::Io)?;
        Ok(())
    }

    /// 加密存储任意类型数据到文件，并更新内存缓存
    pub fn save<T: Serialize>(&mut self, key: &str, value: &T) -> AppResult<()> {
        self.load_from_file()?;
        let serialized = bincode::serialize(value)
            .map_err(|e| AppError::Serde(e.to_string()))?;
        self.cache.insert(key.to_string(), serialized);
        self.save_to_file()
    }

    /// 解密读取任意类型数据，优先查内存缓存
    pub fn load<T: DeserializeOwned>(&mut self, key: &str) -> AppResult<T> {
        self.load_from_file()?;
        let cached = self.cache.get(key)
            .ok_or_else(|| AppError::Config(format!("未找到key: {}", key)))?;
        let value = bincode::deserialize(cached)
            .map_err(|e| AppError::Serde(e.to_string()))?;
        Ok(value)
    }

    /// 清空内存缓存和文件
    pub fn clear_all(&mut self) -> AppResult<()> {
        self.cache.clear();
        self.save_to_file()
    }
}

pub static SECURE_STORE: Lazy<RwLock<SecureStore>> = Lazy::new(|| {
    RwLock::new(SecureStore::new().expect("SecureStore初始化失败"))
});

// 保存
// SECURE_STORE.write().unwrap().save("jwt_token", &token)?;

// 读取
// let token: String = SECURE_STORE.read().unwrap().load("jwt_token")?;
use crate::errors::{AppError, AppResult};
use crate::utils::aes_util::{decrypt_content, encrypt_content};
use crate::utils::file_dir::get_data_dir;
use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use std::sync::RwLock;

const STORE_FILE: &str = "clipPal_store.dat";

#[derive(Serialize, Deserialize, Default, Clone)]
pub struct SecureData {
    pub jwt_token: Option<String>,
}

pub struct SecureStore {
    dir: PathBuf,
    data: SecureData,
    loaded: bool,
}

impl SecureStore {
    pub fn new() -> AppResult<Self> {
        let dir = get_data_dir().ok_or(AppError::Config("无法获取配置目录".to_string()))?;
        Ok(Self {
            dir,
            data: SecureData::default(),
            loaded: false,
        })
    }

    fn load_from_file(&mut self) -> AppResult<()> {
        if self.loaded {
            return Ok(());
        }
        let file_path = self.dir.join(STORE_FILE);
        if !file_path.exists() {
            self.loaded = true;
            return Ok(());
        }
        let encrypted = fs::read_to_string(&file_path).map_err(AppError::Io)?;
        let decrypted = decrypt_content(&encrypted)?;
        let decoded = STANDARD
            .decode(&decrypted)
            .map_err(|e| AppError::Crypto(format!("Base64解码失败: {}", e)))?;
        self.data = bincode::deserialize(&decoded).map_err(|e| AppError::Serde(e.to_string()))?;
        self.loaded = true;
        Ok(())
    }

    fn save_to_file(&self) -> AppResult<()> {
        let file_path = self.dir.join(STORE_FILE);
        let serialized =
            bincode::serialize(&self.data).map_err(|e| AppError::Serde(e.to_string()))?;
        let encoded = STANDARD.encode(&serialized);
        let encrypted = encrypt_content(&encoded)?;
        fs::write(&file_path, &encrypted).map_err(AppError::Io)?;
        Ok(())
    }

    /// 加载数据到内存
    pub fn load(&mut self) -> AppResult<()> {
        self.load_from_file()
    }
    /// 保存内存到文件
    pub fn save(&self) -> AppResult<()> {
        self.save_to_file()
    }
    /// 获取只读数据
    pub fn data(&self) -> &SecureData {
        &self.data
    }
    /// 获取可写数据
    pub fn data_mut(&mut self) -> &mut SecureData {
        &mut self.data
    }
    /// 清空所有内容
    pub fn clear_all(&mut self) -> AppResult<()> {
        self.data = SecureData::default();
        self.save_to_file()
    }

    /// 获取jwt_token
    pub fn get_jwt_token(&mut self) -> AppResult<Option<String>> {
        if !self.loaded {
            self.load()?;
        }
        Ok(self.data.jwt_token.clone())
    }
    /// 设置jwt_token并自动保存
    pub fn set_jwt_token(&mut self, token: String) -> AppResult<()> {
        if !self.loaded {
            self.load()?;
        }
        self.data.jwt_token = Some(token);
        self.save()
    }
}

pub static SECURE_STORE: Lazy<RwLock<SecureStore>> =
    Lazy::new(|| RwLock::new(SecureStore::new().expect("SecureStore初始化失败")));

// 写入 jwt_token
// SECURE_STORE.write().unwrap().set_jwt_token("your_jwt_token".to_string())?;

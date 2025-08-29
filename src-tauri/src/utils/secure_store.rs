#![allow(dead_code)]

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
    pub access_token: Option<String>,
    pub refresh_token: Option<String>,
    pub user_info: Option<String>,
    pub token_expires: Option<i32>,

    // 新增VIP相关字段
    pub vip_info: Option<String>,      // JSON序列化的VIP信息
    pub vip_last_check: Option<u64>,   // 上次检查VIP状态的时间戳
    pub server_config: Option<String>, // 服务器配置信息
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
        Ok(self.data.access_token.clone())
    }
    /// 设置jwt_token并自动保存
    pub fn set_jwt_token(&mut self, token: String) -> AppResult<()> {
        if !self.loaded {
            self.load()?;
        }
        self.data.access_token = Some(token);
        self.save()
    }
    /// 获取refresh_token
    pub fn get_refresh_token(&mut self) -> AppResult<Option<String>> {
        if !self.loaded {
            self.load()?;
        }
        Ok(self.data.refresh_token.clone())
    }
    /// 设置refresh_token并自动保存
    pub fn set_refresh_token(&mut self, token: String) -> AppResult<()> {
        if !self.loaded {
            self.load()?;
        }
        self.data.refresh_token = Some(token);
        self.save()
    }

    /// 获取用户信息
    pub fn get_user_info(&mut self) -> AppResult<Option<String>> {
        if !self.loaded {
            self.load()?;
        }
        Ok(self.data.user_info.clone())
    }

    /// 设置用户信息并自动保存
    pub fn set_user_info(&mut self, user_info: String) -> AppResult<()> {
        if !self.loaded {
            self.load()?;
        }
        self.data.user_info = Some(user_info);
        self.save()
    }

    /// 获取令牌过期时间
    pub fn get_token_expires(&mut self) -> AppResult<Option<i32>> {
        if !self.loaded {
            self.load()?;
        }
        Ok(self.data.token_expires.clone())
    }

    /// 设置令牌过期时间并自动保存
    pub fn set_token_expires(&mut self, expires: i32) -> AppResult<()> {
        if !self.loaded {
            self.load()?;
        }
        self.data.token_expires = Some(expires);
        self.save()
    }

    /// 清除所有认证数据
    pub fn clear_auth_data(&mut self) -> AppResult<()> {
        if !self.loaded {
            self.load()?;
        }
        self.data.access_token = None;
        self.data.refresh_token = None;
        self.data.user_info = None;
        self.data.token_expires = None;
        self.save()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VipInfo {
    pub vip_flag: bool,
    pub vip_type: VipType,
    pub expire_time: Option<u64>,      // 到期时间戳
    pub max_records: u32,              // 最大记录数限制
    pub max_sync_records: u32,         // 可云同步的最大记录数
    pub max_file_size: u64,            // 最大文件大小限制(字节)
    pub features: Option<Vec<String>>, // VIP功能列表
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum VipType {
    Free,      // 免费用户
    Monthly,   // 月付费
    Quarterly, // 季度付费
    Yearly,    // 年付费
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    pub max_file_size: u64,   // 服务器控制的文件大小限制
    pub free_sync_limit: u32, // 免费用户云同步限制
    pub vip_sync_limit: u32,  // VIP用户云同步限制
}

impl SecureStore {
    /// 获取VIP信息
    pub fn get_vip_info(&mut self) -> AppResult<Option<VipInfo>> {
        if !self.loaded {
            self.load()?;
        }

        if let Some(vip_str) = &self.data.vip_info {
            let vip_info: VipInfo = serde_json::from_str(vip_str)
                .map_err(|e| AppError::Serde(format!("VIP信息反序列化失败: {}", e)))?;
            Ok(Some(vip_info))
        } else {
            Ok(None)
        }
    }

    /// 设置VIP信息并自动保存
    pub fn set_vip_info(&mut self, vip_info: VipInfo) -> AppResult<()> {
        if !self.loaded {
            self.load()?;
        }

        let vip_str = serde_json::to_string(&vip_info)
            .map_err(|e| AppError::Serde(format!("VIP信息序列化失败: {}", e)))?;

        self.data.vip_info = Some(vip_str);
        self.save()
    }

    /// 清除VIP信息
    pub fn clear_vip_info(&mut self) -> AppResult<()> {
        if !self.loaded {
            self.load()?;
        }
        self.data.vip_info = None;
        self.data.vip_last_check = None;
        self.save()
    }

    /// 检查是否需要更新VIP状态(超过1小时)
    pub fn should_check_vip_status(&mut self) -> AppResult<bool> {
        if !self.loaded {
            self.load()?;
        }

        if let Some(last_check) = self.data.vip_last_check {
            let current_time = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs();

            Ok(current_time - last_check > 3600) // 1小时
        } else {
            Ok(true) // 从未检查过
        }
    }

    /// 更新VIP检查时间戳
    pub fn update_vip_check_time(&mut self) -> AppResult<()> {
        if !self.loaded {
            self.load()?;
        }

        let current_time = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();

        self.data.vip_last_check = Some(current_time);
        self.save()
    }
}

pub static SECURE_STORE: Lazy<RwLock<SecureStore>> =
    Lazy::new(|| RwLock::new(SecureStore::new().expect("SecureStore初始化失败")));

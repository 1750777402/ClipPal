use crate::{domain::vip::VipInfo, utils::secure_store::SECURE_STORE};

/// VIP 本地存储，只负责加密权益快照和刷新时间的持久化。
pub trait VipStore: Send + Sync {
    fn get_info(&self) -> Result<Option<VipInfo>, String>;
    fn save_checked(&self, info: &VipInfo) -> Result<(), String>;
    fn clear_info(&self) -> Result<(), String>;
    fn should_refresh(&self) -> Result<bool, String>;
}

#[derive(Default)]
pub struct SecureVipStore;

impl VipStore for SecureVipStore {
    fn get_info(&self) -> Result<Option<VipInfo>, String> {
        let mut store = SECURE_STORE
            .write()
            .map_err(|_| "获取 VIP 缓存锁失败".to_string())?;
        store.get_vip_info().map_err(|error| error.to_string())
    }

    fn save_checked(&self, info: &VipInfo) -> Result<(), String> {
        let mut store = SECURE_STORE
            .write()
            .map_err(|_| "获取 VIP 缓存锁失败".to_string())?;
        store
            .set_vip_info_checked(info.clone())
            .map_err(|error| error.to_string())
    }

    fn clear_info(&self) -> Result<(), String> {
        let mut store = SECURE_STORE
            .write()
            .map_err(|_| "获取 VIP 缓存锁失败".to_string())?;
        store.clear_vip_info().map_err(|error| error.to_string())
    }

    fn should_refresh(&self) -> Result<bool, String> {
        let mut store = SECURE_STORE
            .write()
            .map_err(|_| "获取 VIP 缓存锁失败".to_string())?;
        store
            .should_check_vip_status()
            .map_err(|error| error.to_string())
    }
}

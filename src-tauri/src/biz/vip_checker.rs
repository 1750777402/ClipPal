use crate::{
    api::vip_api::{UserVipInfoResponse, user_vip_check},
    biz::clip_record::ClipRecord,
    biz::system_setting::{load_settings, save_settings},
    errors::{AppError, AppResult},
    utils::secure_store::{SECURE_STORE, VipInfo, VipType},
};
use log;

pub struct VipChecker;

impl VipChecker {
    /// 检查用户是否为VIP - 必须调用服务端验证
    pub async fn is_vip_user() -> AppResult<bool> {
        // VIP状态必须通过服务端实时验证，不能依赖本地时间
        match user_vip_check().await {
            Ok(Some(vip_response)) => {
                // 更新本地缓存
                let vip_info = Self::convert_api_response_to_vip_info(vip_response.clone())?;
                let mut store = SECURE_STORE
                    .write()
                    .map_err(|_| AppError::Config("获取存储锁失败".to_string()))?;
                store.set_vip_info(vip_info)?;
                store.update_vip_check_time()?;

                // 返回服务端的VIP状态
                Ok(vip_response.vip_flag)
            }
            Ok(None) => {
                log::warn!("服务端返回空的VIP信息");
                Ok(false)
            }
            Err(e) => {
                log::error!("VIP状态检查失败: {:?}", e);
                // 网络错误时，使用本地缓存作为fallback（但需要警告）
                if let Some(cached_vip) = Self::get_local_vip_info()? {
                    log::warn!("网络错误，使用本地缓存的VIP状态: {}", cached_vip.vip_flag);
                    Ok(cached_vip.vip_flag)
                } else {
                    Ok(false)
                }
            }
        }
    }

    /// 获取本地缓存的VIP状态（仅用于离线fallback）
    pub fn get_cached_vip_status() -> AppResult<bool> {
        if let Some(vip_info) = Self::get_local_vip_info()? {
            Ok(vip_info.vip_flag)
        } else {
            Ok(false)
        }
    }

    /// 检查云同步权限（需要传入RBatis实例）
    pub async fn check_cloud_sync_permission_with_rb(
        rb: &rbatis::RBatis,
    ) -> AppResult<(bool, String)> {
        // 首先检查是否登录
        let has_token = {
            let mut store = SECURE_STORE
                .write()
                .map_err(|_| AppError::Config("获取存储锁失败".to_string()))?;
            store.get_jwt_token()?.is_some()
        };

        if !has_token {
            return Ok((false, "需要登录后才能使用云同步功能".to_string()));
        }

        // 检查VIP状态（调用服务端验证）
        if Self::is_vip_user().await? {
            return Ok((true, "VIP用户，享受完整云同步功能".to_string()));
        }

        // 免费用户检查10条限制
        let current_sync_count = Self::get_current_sync_count(rb).await?;
        if current_sync_count < 10 {
            Ok((
                true,
                format!("免费体验，已使用 {}/10 条云同步", current_sync_count),
            ))
        } else {
            Ok((false, "免费用户云同步额度已用完，请升级VIP".to_string()))
        }
    }

    /// 检查云同步权限（兼容性方法，不需要数据库）
    pub async fn check_cloud_sync_permission() -> AppResult<(bool, String)> {
        Self::check_cloud_sync_permission_with_vip_status(None).await
    }

    /// 检查云同步权限（优化版本，可传入已知的VIP状态避免重复检查）
    pub async fn check_cloud_sync_permission_with_vip_status(
        known_vip_status: Option<bool>,
    ) -> AppResult<(bool, String)> {
        // 首先检查是否登录
        let has_token = {
            let mut store = SECURE_STORE
                .write()
                .map_err(|_| AppError::Config("获取存储锁失败".to_string()))?;
            store.get_jwt_token()?.is_some()
        };

        if !has_token {
            return Ok((false, "需要登录后才能使用云同步功能".to_string()));
        }

        // 使用已知VIP状态或检查VIP状态
        let is_vip = match known_vip_status {
            Some(status) => status,
            None => Self::is_vip_user().await?,
        };

        if is_vip {
            return Ok((true, "VIP用户，享受完整云同步功能".to_string()));
        }

        // 免费用户没有数据库连接时，允许尝试同步（会在实际同步时检查限制）
        Ok((true, "免费用户可尝试云同步".to_string()))
    }

    /// 获取最大记录数限制
    pub async fn get_max_records_limit() -> AppResult<u32> {
        if Self::is_vip_user().await? {
            Ok(1000)
        } else {
            Ok(500)
        }
    }

    /// 验证设置的记录条数是否合法
    pub async fn validate_max_records(max_records: u32) -> AppResult<()> {
        let limit = Self::get_max_records_limit().await?;

        if max_records < 50 || max_records > limit {
            return Err(AppError::Config(format!(
                "最大记录条数必须在50-{}之间",
                limit
            )));
        }

        Ok(())
    }

    /// 重置为免费用户状态
    pub async fn reset_to_free_user() -> AppResult<()> {
        log::info!("重置用户状态为免费用户");

        // 清除VIP信息
        let mut store = SECURE_STORE
            .write()
            .map_err(|_| AppError::Config("获取存储锁失败".to_string()))?;
        store.clear_vip_info()?;
        drop(store);

        // 更新系统设置
        let mut settings = load_settings();
        settings.cloud_sync = 0; // 关闭云同步

        // 如果当前记录数超过免费限制，调整为500
        if settings.max_records > 500 {
            settings.max_records = 500;
        }

        save_settings(settings)
            .await
            .map_err(|e| AppError::Config(e))?;

        Ok(())
    }

    /// 从服务器刷新VIP状态 - 调用现有的user_vip_check方法
    /// 如果服务器获取失败，返回成功状态，让前端继续显示本地缓存
    pub async fn refresh_vip_from_server() -> AppResult<bool> {
        log::info!("从服务器刷新VIP状态");

        // 调用现有的user_vip_check API
        match user_vip_check().await {
            Ok(Some(vip_response)) => {
                // 转换API响应为本地VIP信息结构
                let vip_info = Self::convert_api_response_to_vip_info(vip_response)?;

                // 保存到加密存储
                let mut store = SECURE_STORE
                    .write()
                    .map_err(|_| AppError::Config("获取存储锁失败".to_string()))?;
                store.set_vip_info(vip_info)?;
                store.update_vip_check_time()?;

                log::info!("VIP状态已从服务器更新");
                Ok(true)
            }
            Ok(None) => {
                log::warn!("服务器返回空的VIP信息，使用本地缓存");
                Ok(false)
            }
            Err(e) => {
                log::warn!("从服务器获取VIP状态失败，将使用本地缓存: {:?}", e);
                // 服务器获取失败时，返回false但不抛出错误，让前端继续显示本地缓存
                Ok(false)
            }
        }
    }

    /// 获取本地VIP信息
    pub fn get_local_vip_info() -> AppResult<Option<VipInfo>> {
        let mut store = SECURE_STORE
            .write()
            .map_err(|_| AppError::Config("获取存储锁失败".to_string()))?;
        store.get_vip_info()
    }

    /// 获取当前云同步记录数（需要传入RBatis实例）
    pub async fn get_current_sync_count(rb: &rbatis::RBatis) -> AppResult<u32> {
        // 查询已同步到云端的记录数量
        let count = ClipRecord::select_sync_count(rb)
            .await
            .map_err(|e| AppError::Config(format!("查询同步记录数失败: {}", e)))?;

        Ok(count as u32)
    }

    /// 获取最大文件大小限制
    pub async fn get_max_file_size() -> AppResult<u64> {
        if Self::is_vip_user().await? {
            Ok(5 * 1024 * 1024) // VIP用户5MB
        } else {
            Ok(0) // 免费用户不支持文件
        }
    }

    /// 转换API响应为VIP信息结构
    fn convert_api_response_to_vip_info(response: UserVipInfoResponse) -> AppResult<VipInfo> {
        let vip_type = match response.vip_type.as_deref() {
            Some("monthly") => VipType::Monthly,
            Some("quarterly") => VipType::Quarterly,
            Some("yearly") => VipType::Yearly,
            _ => VipType::Free,
        };

        Ok(VipInfo {
            vip_flag: response.vip_flag,
            vip_type,
            expire_time: response.expire_time,
            max_records: response.max_records,
            max_sync_records: response.max_sync_records,
            features: response.features,
        })
    }

    /// 检查是否需要刷新VIP状态（超过1小时或从未检查过）
    pub fn should_refresh_vip_status() -> AppResult<bool> {
        let mut store = SECURE_STORE
            .write()
            .map_err(|_| AppError::Config("获取存储锁失败".to_string()))?;
        store.should_check_vip_status()
    }
}

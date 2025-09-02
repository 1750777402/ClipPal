use crate::{
    CONTEXT,
    api::vip_api::{UserVipInfoResponse, user_vip_check},
    biz::{
        clip_record::ClipRecord,
        system_setting::{load_settings, save_settings},
    },
    errors::{AppError, AppResult},
    utils::secure_store::{SECURE_STORE, VipInfo, VipType},
};
use log;
use rbatis::RBatis;

pub struct VipChecker;

impl VipChecker {
    /// 检查用户是否为VIP - 必须调用服务端验证，同时处理权益更新
    pub async fn is_vip_user() -> AppResult<bool> {
        // VIP状态必须通过服务端实时验证，不能依赖本地时间
        match user_vip_check().await {
            Ok(Some(vip_response)) => {
                // 更新本地缓存
                let vip_info = Self::convert_api_response_to_vip_info(vip_response.clone())?;
                {
                    let mut store = SECURE_STORE
                        .write()
                        .map_err(|_| AppError::Config("获取存储锁失败".to_string()))?;
                    store.set_vip_info(vip_info)?;
                    store.update_vip_check_time()?;
                } // store在这里被drop

                // 处理本地记录条数限制
                Self::enforce_local_records_limit(&vip_response).await?;

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
    pub async fn check_cloud_sync_permission_with_rb() -> AppResult<(bool, String)> {
        // 首先检查是否登录
        let has_token = {
            let mut store = SECURE_STORE
                .write()
                .map_err(|_| AppError::Config("获取存储锁失败".to_string()))?;
            let result = store.get_jwt_token()?.is_some();
            drop(store);
            result
        };

        if !has_token {
            return Ok((false, "需要登录后才能使用云同步功能".to_string()));
        }

        // 检查VIP状态（调用服务端验证）
        if Self::is_vip_user().await? {
            return Ok((true, "VIP用户，享受完整云同步功能".to_string()));
        }

        // 免费用户也可以使用云同步，不再限制条数
        Ok((true, "云同步功能已启用".to_string()))
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
            let result = store.get_jwt_token()?.is_some();
            drop(store);
            result
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

        // 免费用户也可以使用云同步，不再限制条数
        Ok((true, "云同步功能已启用".to_string()))
    }

    /// 获取最大记录数限制（基于服务端返回的数据）
    pub async fn get_max_records_limit() -> AppResult<u32> {
        // 调用服务端检查VIP状态，这会同时更新本地VIP信息
        if Self::is_vip_user().await? {
            // 从本地缓存获取服务端返回的具体限制
            if let Some(vip_info) = Self::get_local_vip_info()? {
                Ok(vip_info.max_records)
            } else {
                Ok(1000) // 默认VIP限制
            }
        } else {
            // 免费用户：尝试从服务器配置获取，如果没有则使用默认值300
            if let Ok(Some(server_config)) = crate::api::vip_api::get_server_config().await {
                if let Some(free_config) = server_config.get(&VipType::Free) {
                    return Ok(free_config.record_limit);
                }
            }
            Ok(300) // 免费用户默认限制300条
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
                let vip_info = Self::convert_api_response_to_vip_info(vip_response.clone())?;

                // 保存到加密存储
                {
                    let mut store = SECURE_STORE
                        .write()
                        .map_err(|_| AppError::Config("获取存储锁失败".to_string()))?;
                    store.set_vip_info(vip_info)?;
                    store.update_vip_check_time()?;
                } // store在这里被drop

                // 处理本地记录条数限制 - VIP状态变化时自动调整max_records设置
                Self::enforce_local_records_limit(&vip_response).await?;

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

    /// 获取最大文件大小限制（基于服务端返回的数据，转换为字节）
    pub async fn get_max_file_size() -> AppResult<u64> {
        // 调用服务端检查VIP状态，这会同时更新本地VIP信息
        if Self::is_vip_user().await? {
            if let Some(vip_info) = Self::get_local_vip_info()? {
                // 服务端返回KB，转换为字节进行文件大小比较
                Ok(vip_info.max_file_size * 1024)
            } else {
                Ok(5120 * 1024) // 默认VIP限制5MB，服务端返回5120KB，转换为字节
            }
        } else {
            Ok(0) // 免费用户不支持文件
        }
    }

    /// 转换API响应为VIP信息结构
    fn convert_api_response_to_vip_info(response: UserVipInfoResponse) -> AppResult<VipInfo> {
        let vip_type = response.vip_type.unwrap_or(VipType::Free);

        Ok(VipInfo {
            vip_flag: response.vip_flag,
            vip_type,
            expire_time: response.expire_time,
            max_records: response.max_records,
            max_file_size: response.max_file_size, // 使用服务端返回的动态文件大小限制
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

    /// 强制执行本地记录条数限制（仅更新本地设置，避免递归）
    async fn enforce_local_records_limit(vip_response: &UserVipInfoResponse) -> AppResult<()> {
        use crate::biz::system_setting::{Settings, load_settings};
        use std::sync::Arc;

        let mut settings = load_settings();
        let current_max = settings.max_records;
        let server_max = vip_response.max_records;

        // 如果当前设置超过服务器允许的最大值，强制调整
        if current_max > server_max {
            log::warn!(
                "本地记录条数({})超过服务端限制({})，自动调整",
                current_max,
                server_max
            );
            settings.max_records = server_max;

            // 更新内存设置
            let settings_lock = CONTEXT.get::<Arc<std::sync::RwLock<Settings>>>();
            match settings_lock.write() {
                Ok(mut guard) => {
                    *guard = settings.clone();
                    log::info!("VIP记录数限制已应用到内存设置");
                }
                Err(e) => {
                    log::error!("更新内存设置失败: {}", e);
                    return Err(AppError::Config("更新内存设置失败".to_string()));
                }
            }

            // 持久化到磁盘
            use crate::biz::system_setting::save_settings_to_file;
            if let Err(e) = save_settings_to_file(&settings) {
                log::error!("持久化设置到磁盘失败: {}", e);
                // 不返回错误，因为内存已经更新成功
            } else {
                log::info!("VIP记录数限制已持久化到磁盘");
            }
        }

        Ok(())
    }

    /// 获取VIP感知的文件大小限制（完全基于服务端缓存的数据，转换为字节）
    pub async fn get_vip_aware_max_file_size() -> AppResult<u64> {
        // 直接从本地缓存获取服务端返回的文件大小限制
        if let Some(vip_info) = Self::get_local_vip_info()? {
            // 服务端返回KB，转换为字节进行文件大小比较
            Ok(vip_info.max_file_size * 1024)
        } else {
            // 无VIP信息时，默认为免费用户（不支持文件）
            Ok(0)
        }
    }

    /// 获取VIP记录数限制（仅使用本地缓存，不触发服务器调用）
    pub fn get_cached_max_records_limit() -> AppResult<u32> {
        // 优先从VIP信息获取（适用于VIP用户）
        if let Some(vip_info) = Self::get_local_vip_info()? {
            // 如果是VIP用户，使用VIP信息中的限制
            if vip_info.vip_flag {
                return Ok(vip_info.max_records);
            }
        }

        Ok(300) // 免费用户默认限制300条
    }

    /// 获取VIP文件大小限制（仅使用本地缓存，不触发服务器调用，转换为字节）
    pub fn get_cached_max_file_size() -> AppResult<u64> {
        if let Some(vip_info) = Self::get_local_vip_info()? {
            // 服务端返回KB，转换为字节
            Ok(vip_info.max_file_size * 1024)
        } else {
            // 无缓存时，免费用户不支持文件
            Ok(0)
        }
    }

    // /// 获取云同步记录限制（基于服务端缓存的数据）- 不再需要条数限制
    // pub async fn get_sync_records_limit() -> AppResult<u32> {
    //     if let Some(vip_info) = Self::get_local_vip_info()? {
    //         // 使用服务端返回的动态云同步限制
    //         Ok(vip_info.max_sync_records)
    //     } else {
    //         // 无VIP信息时，默认免费用户限制
    //         Ok(10)
    //     }
    // }

    /// 检查文件是否可以同步（基于VIP状态和文件大小）
    pub async fn can_sync_file(file_size: u64) -> AppResult<(bool, String)> {
        let max_file_size = Self::get_vip_aware_max_file_size().await?;

        if max_file_size == 0 {
            return Ok((false, "免费用户不支持文件云同步".to_string()));
        }

        if file_size > max_file_size {
            let size_mb = file_size as f64 / 1024.0 / 1024.0;
            let max_mb = max_file_size as f64 / 1024.0 / 1024.0;
            return Ok((
                false,
                format!("文件大小 {:.2}MB 超过 {:.2}MB 限制", size_mb, max_mb),
            ));
        }

        Ok((true, "文件可以同步".to_string()))
    }

    /// 检查并强制执行本地记录条数限制
    pub async fn enforce_local_records_limit_from_db() -> AppResult<()> {
        use crate::biz::system_setting::{load_settings, save_settings};

        // 获取当前VIP状态和限制
        let max_allowed = Self::get_max_records_limit().await?;
        let mut settings = load_settings();

        // 如果当前设置超过允许的最大值，强制调整
        if settings.max_records > max_allowed {
            log::warn!(
                "本地记录条数({})超过VIP限制({})，自动调整",
                settings.max_records,
                max_allowed
            );
            settings.max_records = max_allowed;
            save_settings(settings)
                .await
                .map_err(|e| AppError::Config(format!("保存设置失败: {}", e)))?;
        }

        // 检查数据库中的实际记录数，如果超过限制则进行清理
        let rb: &RBatis = CONTEXT.get::<RBatis>();
        let current_count = ClipRecord::count_all_records(rb)
            .await
            .map_err(|e| AppError::Config(format!("查询记录总数失败: {}", e)))?;

        if current_count > max_allowed as i64 {
            log::warn!(
                "数据库记录数({})超过VIP限制({})，执行清理",
                current_count,
                max_allowed
            );
            // 保留最新的记录，删除超出部分
            let excess_count = current_count - max_allowed as i64;
            ClipRecord::delete_oldest_records(rb, excess_count as i32)
                .await
                .map_err(|e| AppError::Config(format!("清理超出记录失败: {}", e)))?;
        }

        Ok(())
    }

    /// 启动时初始化VIP状态并执行限制检查
    pub async fn initialize_vip_and_enforce_limits() -> AppResult<()> {
        log::info!("初始化VIP状态并执行权益限制检查");

        // 检查VIP状态（这会同时更新本地状态和执行记录数限制）
        let _ = Self::is_vip_user().await?;

        // 额外检查数据库记录数并清理超出部分
        Self::enforce_local_records_limit_from_db().await?;

        log::info!("VIP状态初始化和权益限制检查完成");
        Ok(())
    }
}

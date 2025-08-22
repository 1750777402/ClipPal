use crate::{
    api::user_auth_api::{refresh_token as api_refresh_token, RefreshTokenRequestParam, AuthResponse},
    utils::secure_store::SECURE_STORE,
    CONTEXT
};
use std::sync::{Arc, RwLock, OnceLock};
use serde_json;
use tauri::Emitter;

/// JWT令牌管理器，负责自动刷新令牌
pub struct TokenManager {
    is_refreshing: Arc<RwLock<bool>>,
}

static TOKEN_MANAGER: OnceLock<TokenManager> = OnceLock::new();

impl TokenManager {
    pub fn new() -> Self {
        Self {
            is_refreshing: Arc::new(RwLock::new(false)),
        }
    }

    /// 获取全局令牌管理器实例
    pub fn instance() -> &'static TokenManager {
        TOKEN_MANAGER.get_or_init(|| TokenManager::new())
    }

    /// 获取有效的访问令牌，如果过期则自动刷新
    pub async fn get_valid_access_token(&self) -> Result<Option<String>, String> {
        // 先尝试获取当前令牌
        let current_token = self.get_stored_access_token();
        if current_token.is_some() {
            // 这里可以添加令牌过期检查逻辑
            // 目前先返回现有令牌，实际使用中如果API返回401会触发刷新
            return Ok(current_token);
        }

        // 如果没有令牌，返回None
        Ok(None)
    }

    /// 当API返回401时调用此方法刷新令牌
    pub async fn refresh_access_token(&self) -> Result<Option<String>, String> {
        // 防止并发刷新
        {
            let is_refreshing = self.is_refreshing.read().map_err(|e| format!("获取刷新锁失败: {}", e))?;
            if *is_refreshing {
                // 等待其他线程完成刷新，然后返回新令牌
                std::thread::sleep(std::time::Duration::from_millis(100));
                return Ok(self.get_stored_access_token());
            }
        }

        // 设置刷新状态
        {
            let mut is_refreshing = self.is_refreshing.write().map_err(|e| format!("设置刷新锁失败: {}", e))?;
            *is_refreshing = true;
        }

        let result = self.do_refresh_token().await;

        // 清除刷新状态
        {
            let mut is_refreshing = self.is_refreshing.write().map_err(|e| format!("清除刷新锁失败: {}", e))?;
            *is_refreshing = false;
        }

        result
    }

    /// 执行实际的令牌刷新
    async fn do_refresh_token(&self) -> Result<Option<String>, String> {
        let refresh_token = self.get_stored_refresh_token()
            .ok_or("没有有效的刷新令牌")?;

        log::info!("开始刷新访问令牌");

        let request = RefreshTokenRequestParam {
            refresh_token: refresh_token.clone(),
        };

        match api_refresh_token(&request).await {
            Ok(Some(auth_response)) => {
                log::info!("令牌刷新成功");
                
                // 更新存储的令牌信息
                if let Err(e) = self.update_stored_tokens(&auth_response).await {
                    log::error!("更新存储的令牌失败: {}", e);
                    return Err(e);
                }

                Ok(Some(auth_response.access_token))
            }
            Ok(None) => {
                log::warn!("令牌刷新返回空响应");
                // 刷新令牌可能已过期，清除所有认证数据
                self.clear_auth_data()?;
                // 通知前端登录状态失效
                self.notify_auth_expired().await;
                Err("刷新令牌已过期，需要重新登录".to_string())
            }
            Err(e) => {
                log::error!("令牌刷新失败: {}", e);
                // 刷新失败，清除所有认证数据
                self.clear_auth_data()?;
                // 通知前端登录状态失效
                self.notify_auth_expired().await;
                Err(format!("令牌刷新失败: {}", e))
            }
        }
    }

    /// 更新存储的令牌信息
    async fn update_stored_tokens(&self, auth_response: &AuthResponse) -> Result<(), String> {
        let mut store = SECURE_STORE
            .write()
            .map_err(|e| format!("获取存储写锁失败: {}", e))?;

        // 更新访问令牌
        store
            .set_jwt_token(auth_response.access_token.clone())
            .map_err(|e| format!("存储访问令牌失败: {}", e))?;

        // 更新刷新令牌
        store
            .set_refresh_token(auth_response.refresh_token.clone())
            .map_err(|e| format!("存储刷新令牌失败: {}", e))?;

        // 更新过期时间
        store
            .set_token_expires(auth_response.expires_in.clone())
            .map_err(|e| format!("存储过期时间失败: {}", e))?;

        // 更新用户信息（如果有的话）
        let user_info_json = serde_json::to_string(&auth_response.user_info)
            .map_err(|e| format!("序列化用户信息失败: {}", e))?;
        store
            .set_user_info(user_info_json)
            .map_err(|e| format!("存储用户信息失败: {}", e))?;

        log::info!("令牌信息已更新");
        Ok(())
    }

    /// 获取存储的访问令牌
    fn get_stored_access_token(&self) -> Option<String> {
        SECURE_STORE
            .read()
            .ok()
            .and_then(|store| store.data().access_token.clone())
    }

    /// 获取存储的刷新令牌
    fn get_stored_refresh_token(&self) -> Option<String> {
        SECURE_STORE
            .read()
            .ok()
            .and_then(|store| store.data().refresh_token.clone())
    }

    /// 清除认证数据
    fn clear_auth_data(&self) -> Result<(), String> {
        let mut store = SECURE_STORE
            .write()
            .map_err(|e| format!("获取存储写锁失败: {}", e))?;

        store
            .clear_auth_data()
            .map_err(|e| format!("清除认证数据失败: {}", e))?;

        log::info!("认证数据已清除");
        Ok(())
    }

    /// 通知前端认证已过期
    async fn notify_auth_expired(&self) {
        log::info!("通知前端认证已过期");
        
        // 通过Tauri事件系统通知前端
        if let Some(app_handle) = CONTEXT.try_get::<tauri::AppHandle>() {
            if let Err(e) = app_handle.emit("auth-expired", ()) {
                log::error!("发送认证过期事件失败: {}", e);
            }
        }

        // 关闭云同步功能
        self.disable_cloud_sync().await;
    }

    /// 禁用云同步功能
    async fn disable_cloud_sync(&self) {
        log::info!("认证失效，禁用云同步功能");
        
        // 实际修改设置中的云同步开关
        if let Err(e) = crate::biz::system_setting::disable_cloud_sync().await {
            log::error!("禁用云同步设置失败: {}", e);
        }
        
        // 通知前端云同步已被禁用，前端需要更新UI状态
        if let Some(app_handle) = CONTEXT.try_get::<tauri::AppHandle>() {
            if let Err(e) = app_handle.emit("cloud-sync-disabled", ()) {
                log::error!("发送云同步禁用事件失败: {}", e);
            }
        }
    }

    /// 检查是否有有效的登录状态
    pub fn has_valid_auth(&self) -> bool {
        self.get_stored_access_token().is_some() && self.get_stored_refresh_token().is_some()
    }
}

/// 便捷函数：获取有效的访问令牌
pub async fn get_valid_access_token() -> Result<Option<String>, String> {
    TokenManager::instance().get_valid_access_token().await
}

/// 便捷函数：刷新访问令牌
pub async fn refresh_access_token() -> Result<Option<String>, String> {
    TokenManager::instance().refresh_access_token().await
}

/// 便捷函数：检查是否有有效的登录状态
pub fn has_valid_auth() -> bool {
    TokenManager::instance().has_valid_auth()
}
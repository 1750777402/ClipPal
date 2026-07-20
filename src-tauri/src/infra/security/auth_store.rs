use crate::{
    api::user_auth_api::{AuthResponse, UserInfo as ApiUserInfo},
    utils::secure_store::SECURE_STORE,
};

/// 认证安全存储接口，隔离仓储与加密文件存储的具体实现。
pub trait AuthStore: Send + Sync {
    /// 保存登录接口返回的访问令牌、刷新令牌、用户资料和过期时间。
    fn store_auth_data(&self, response: &AuthResponse) -> Result<(), String>;

    /// 读取本地访问令牌。
    fn access_token(&self) -> Option<String>;

    /// 读取并反序列化本地用户资料。
    fn user_info(&self) -> Option<ApiUserInfo>;

    /// 清除所有认证相关本地数据。
    fn clear_auth_data(&self) -> Result<(), String>;

    /// 只更新本地用户资料中的昵称字段。
    fn update_user_nickname(&self, nickname: &str) -> Result<(), String>;
}

#[derive(Default)]
/// 基于应用加密存储文件的认证数据实现。
pub struct SecureAuthStore;

impl AuthStore for SecureAuthStore {
    /// 在写锁保护下顺序保存完整认证响应；任一字段失败都会向上返回错误。
    fn store_auth_data(&self, response: &AuthResponse) -> Result<(), String> {
        // 全部认证字段共享同一个 SecureStore，写锁保证并发写入不会交叉。
        let mut store = SECURE_STORE
            .write()
            .map_err(|error| format!("获取存储写锁失败: {}", error))?;

        store
            .set_jwt_token(response.access_token.clone())
            .map_err(|error| format!("存储访问令牌失败: {}", error))?;
        store
            .set_refresh_token(response.refresh_token.clone())
            .map_err(|error| format!("存储刷新令牌失败: {}", error))?;

        let user_info = serde_json::to_string(&response.user_info)
            .map_err(|error| format!("序列化用户信息失败: {}", error))?;
        store
            .set_user_info(user_info)
            .map_err(|error| format!("存储用户信息失败: {}", error))?;
        store
            .set_token_expires(response.expires_in)
            .map_err(|error| format!("存储过期时间失败: {}", error))?;

        Ok(())
    }

    /// 从安全存储读取访问令牌；锁或文件读取失败时记录日志并返回空。
    fn access_token(&self) -> Option<String> {
        match SECURE_STORE.write() {
            Ok(mut store) => store.get_jwt_token().ok().flatten(),
            Err(error) => {
                log::error!("获取访问令牌存储锁失败: {}", error);
                None
            }
        }
    }

    /// 读取 JSON 用户资料并反序列化为认证 API 模型。
    fn user_info(&self) -> Option<ApiUserInfo> {
        match SECURE_STORE.write() {
            Ok(mut store) => match store.get_user_info() {
                Ok(Some(user_info)) => serde_json::from_str(&user_info).ok(),
                _ => None,
            },
            Err(error) => {
                log::error!("获取用户信息存储锁失败: {}", error);
                None
            }
        }
    }

    /// 清除令牌、用户资料和过期时间并持久化。
    fn clear_auth_data(&self) -> Result<(), String> {
        let mut store = SECURE_STORE
            .write()
            .map_err(|error| format!("获取存储写锁失败: {}", error))?;
        store
            .clear_auth_data()
            .map_err(|error| format!("清除认证数据失败: {}", error))
    }

    /// 基于现有用户 JSON 更新昵称后重新写入安全存储。
    fn update_user_nickname(&self, nickname: &str) -> Result<(), String> {
        let mut store = SECURE_STORE
            .write()
            .map_err(|error| format!("获取存储写锁失败: {}", error))?;
        let user_info = store
            .get_user_info()
            .map_err(|error| format!("获取用户信息失败: {}", error))?;

        if let Some(user_info) = user_info {
            let mut user: ApiUserInfo = serde_json::from_str(&user_info)
                .map_err(|error| format!("反序列化用户信息失败: {}", error))?;
            user.nick_name = Some(nickname.to_string());
            let user_info = serde_json::to_string(&user)
                .map_err(|error| format!("序列化更新后的用户信息失败: {}", error))?;
            store
                .set_user_info(user_info)
                .map_err(|error| format!("存储更新后的用户信息失败: {}", error))?;
        }

        Ok(())
    }
}

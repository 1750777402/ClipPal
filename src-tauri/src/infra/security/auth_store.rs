use crate::{
    api::user_auth_api::{AuthResponse, UserInfo as ApiUserInfo},
    utils::secure_store::SECURE_STORE,
};

pub trait AuthStore: Send + Sync {
    fn store_auth_data(&self, response: &AuthResponse) -> Result<(), String>;

    fn access_token(&self) -> Option<String>;

    fn user_info(&self) -> Option<ApiUserInfo>;

    fn clear_auth_data(&self) -> Result<(), String>;

    fn update_user_nickname(&self, nickname: &str) -> Result<(), String>;
}

#[derive(Default)]
pub struct SecureAuthStore;

impl AuthStore for SecureAuthStore {
    fn store_auth_data(&self, response: &AuthResponse) -> Result<(), String> {
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

    fn access_token(&self) -> Option<String> {
        match SECURE_STORE.write() {
            Ok(mut store) => store.get_jwt_token().ok().flatten(),
            Err(error) => {
                log::error!("获取访问令牌存储锁失败: {}", error);
                None
            }
        }
    }

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

    fn clear_auth_data(&self) -> Result<(), String> {
        let mut store = SECURE_STORE
            .write()
            .map_err(|error| format!("获取存储写锁失败: {}", error))?;
        store
            .clear_auth_data()
            .map_err(|error| format!("清除认证数据失败: {}", error))
    }

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

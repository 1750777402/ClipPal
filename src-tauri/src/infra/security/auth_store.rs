use serde::{Deserialize, Serialize};

use crate::{
    domain::user::{AuthSession, UserInfo},
    utils::secure_store::SECURE_STORE,
};

/// 认证本地存储，只负责会话数据的加密持久化。
pub trait AuthStore: Send + Sync {
    fn save_session(&self, session: &AuthSession) -> Result<(), String>;
    fn access_token(&self) -> Option<String>;
    fn refresh_token(&self) -> Option<String>;
    fn user_info(&self) -> Option<UserInfo>;
    fn clear(&self) -> Result<(), String>;
    fn update_user_nickname(&self, nickname: &str) -> Result<(), String>;

    fn has_access_token(&self) -> bool {
        self.access_token().is_some()
    }

    fn has_complete_session(&self) -> bool {
        self.access_token().is_some() && self.refresh_token().is_some()
    }
}

#[derive(Default)]
pub struct SecureAuthStore;

#[derive(Serialize, Deserialize)]
struct StoredUserInfo {
    id: u64,
    #[serde(alias = "username")]
    account: String,
    #[serde(alias = "nickName")]
    nickname: Option<String>,
    email: Option<String>,
    phone: Option<String>,
}

impl From<UserInfo> for StoredUserInfo {
    fn from(user: UserInfo) -> Self {
        Self {
            id: user.id,
            account: user.account,
            nickname: user.nickname,
            email: user.email,
            phone: user.phone,
        }
    }
}

impl From<StoredUserInfo> for UserInfo {
    fn from(user: StoredUserInfo) -> Self {
        Self {
            id: user.id,
            account: user.account,
            nickname: user.nickname,
            email: user.email,
            phone: user.phone,
        }
    }
}

impl AuthStore for SecureAuthStore {
    fn save_session(&self, session: &AuthSession) -> Result<(), String> {
        let user_info = serde_json::to_string(&StoredUserInfo::from(session.user_info.clone()))
            .map_err(|error| format!("序列化用户信息失败: {}", error))?;
        let mut store = SECURE_STORE
            .write()
            .map_err(|error| format!("获取存储写锁失败: {}", error))?;
        store
            .set_auth_data(
                session.access_token.clone(),
                session.refresh_token.clone(),
                user_info,
                session.expires_in,
            )
            .map_err(|error| format!("保存认证会话失败: {}", error))
    }

    fn access_token(&self) -> Option<String> {
        SECURE_STORE
            .write()
            .ok()
            .and_then(|mut store| store.get_jwt_token().ok().flatten())
    }

    fn refresh_token(&self) -> Option<String> {
        SECURE_STORE
            .write()
            .ok()
            .and_then(|mut store| store.get_refresh_token().ok().flatten())
    }

    fn user_info(&self) -> Option<UserInfo> {
        SECURE_STORE
            .write()
            .ok()
            .and_then(|mut store| store.get_user_info().ok().flatten())
            .and_then(|user| serde_json::from_str::<StoredUserInfo>(&user).ok())
            .map(Into::into)
    }

    fn clear(&self) -> Result<(), String> {
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
        let Some(user_info) = store
            .get_user_info()
            .map_err(|error| format!("获取用户信息失败: {}", error))?
        else {
            return Ok(());
        };

        let mut user: StoredUserInfo = serde_json::from_str(&user_info)
            .map_err(|error| format!("反序列化用户信息失败: {}", error))?;
        user.nickname = Some(nickname.to_string());
        let user_info = serde_json::to_string(&user)
            .map_err(|error| format!("序列化更新后的用户信息失败: {}", error))?;
        store
            .set_user_info(user_info)
            .map_err(|error| format!("保存更新后的用户信息失败: {}", error))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_legacy_api_user_json() {
        let legacy = r#"{"id":1,"username":"demo","nickName":"Demo","email":null,"phone":null}"#;
        let user: StoredUserInfo = serde_json::from_str(legacy).unwrap();

        assert_eq!(user.account, "demo");
        assert_eq!(user.nickname.as_deref(), Some("Demo"));
    }
}

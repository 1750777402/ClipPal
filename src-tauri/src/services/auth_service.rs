use tauri::Emitter;

use crate::{
    app_context::AppContext,
    domain::user::{
        FrontendCheckUsernameRequest, FrontendLoginRequest, FrontendRegisterRequest,
        FrontendSendEmailCodeRequest, LoginResponse, UserInfo,
    },
    infra::security::AuthStore,
    services::{ports::AuthGateway, settings_service::SettingsService, vip_service::VipService},
};

/// 认证应用服务，负责编排远程认证端口、本地会话存储、VIP 初始化、设置更新和前端事件。
pub struct AuthService<'a> {
    context: &'a AppContext,
    gateway: &'a dyn AuthGateway,
    store: &'a dyn AuthStore,
}

impl<'a> AuthService<'a> {
    /// 从应用上下文取得认证端口和本地会话存储，创建一次 command 调用使用的服务实例。
    pub fn from_context(context: &'a AppContext) -> Self {
        Self {
            context,
            gateway: context.auth_gateway(),
            store: context.auth_store(),
        }
    }
    /// 执行用户登录；远程端口获取会话，本地存储持久化令牌，登录成功后异步初始化 VIP 权益。
    pub async fn login(&self, param: FrontendLoginRequest) -> Result<LoginResponse, String> {
        log::info!("用户登录请求: {}", param.account);
        let session = self
            .gateway
            .login(param)
            .await?
            .ok_or_else(|| "登录响应为空".to_string())?;
        // 登录新会话前清理上一账号的 VIP 快照，避免新账号在远程刷新完成前继承旧权益。
        self.context.vip_store().clear_info()?;
        // 只有完整会话一次落盘成功后，才向上层返回登录成功。
        self.store.save_session(&session)?;
        let response = LoginResponse {
            user_info: session.user_info,
            token: session.access_token,
            expires_in: session.expires_in,
        };
        log::info!("用户登录成功: {}", response.user_info.account);

        // VIP 检查可能访问网络和清理超限记录，不阻塞登录响应返回。
        let context = crate::app_context::app_context().map_err(|error| error.to_string())?;
        tokio::spawn(async move {
            if let Err(error) = VipService::from_context(context.as_ref())
                .initialize_and_enforce_limits()
                .await
            {
                log::error!("登录后 VIP 状态初始化失败: {}", error);
            }
        });

        Ok(response)
    }

    /// 注册用户账号，返回服务端创建的用户资料。
    pub async fn register(&self, param: FrontendRegisterRequest) -> Result<UserInfo, String> {
        log::info!("用户注册请求: {}", param.account);
        self.gateway
            .register(param)
            .await?
            .ok_or_else(|| "注册响应为空".to_string())
    }

    /// 通过远程认证端口请求发送邮箱验证码。
    pub async fn send_email_code(
        &self,
        param: FrontendSendEmailCodeRequest,
    ) -> Result<String, String> {
        match self.gateway.send_email_code(param).await? {
            Some(true) => Ok("验证码已发送".to_string()),
            Some(false) | None => Err("验证码发送失败".to_string()),
        }
    }

    /// 退出当前账号，依次通知服务端、清除本地认证数据并关闭云同步。
    pub async fn logout(&self) -> Result<String, String> {
        // 仅在本地存在令牌时通知服务端，避免无意义的未认证请求。
        if self.store.has_access_token() {
            if let Err(error) = self.gateway.logout().await {
                log::warn!("通知服务端退出失败，继续清理本地会话: {}", error);
            }
        }

        // 本地认证状态必须清理成功，随后通知前端关闭登录相关界面状态。
        self.store.clear()?;
        if let Err(error) = self.context.vip_store().clear_info() {
            log::error!("清除本地 VIP 缓存失败: {}", error);
        }
        self.emit("auth-cleared");

        // 登出后通过设置服务统一更新配置文件和运行期缓存。
        if let Err(error) = SettingsService::from_context(self.context).disable_cloud_sync() {
            log::error!("禁用云同步设置失败: {}", error);
        }
        self.emit("cloud-sync-disabled");

        Ok("退出成功".to_string())
    }

    /// 判断本地是否存在访问令牌，用于前端进行轻量登录态检查。
    pub fn validate_token(&self) -> Result<bool, String> {
        Ok(self.store.has_access_token())
    }

    /// 获取当前用户信息；本地没有用户资料时返回“未登录”错误。
    pub fn get_user_info(&self) -> Result<UserInfo, String> {
        self.store
            .user_info()
            .ok_or_else(|| "用户未登录".to_string())
    }

    /// 恢复登录状态；令牌存在但用户资料损坏时主动清理不完整认证数据。
    pub fn check_login_status(&self) -> Result<Option<UserInfo>, String> {
        if self.store.has_access_token() {
            match self.store.user_info() {
                Some(user_info) => Ok(Some(user_info.into())),
                None => {
                    let _ = self.store.clear();
                    Ok(None)
                }
            }
        } else {
            Ok(None)
        }
    }

    /// 通过远程认证端口检查用户名可用性。
    pub async fn check_username(
        &self,
        param: FrontendCheckUsernameRequest,
    ) -> Result<bool, String> {
        self.gateway
            .check_username(param)
            .await?
            .ok_or_else(|| "用户名不可用".to_string())
    }

    /// 校验昵称业务规则后更新服务端和本地缓存。
    pub async fn update_user_info(&self, nickname: String) -> Result<bool, String> {
        let nickname = nickname.trim();
        if nickname.is_empty() {
            return Err("昵称不能为空".to_string());
        }
        // 字符长度限制在发起网络请求前完成，避免提交无效数据。
        if nickname.len() > 20 {
            return Err("昵称长度不能超过20个字符".to_string());
        }

        match self.gateway.update_nickname(nickname).await? {
            Some(true) => {
                if let Err(error) = self.store.update_user_nickname(nickname) {
                    log::warn!("更新本地用户信息失败: {}", error);
                }
                Ok(true)
            }
            Some(false) | None => Err("昵称更新失败".to_string()),
        }
    }

    /// 向前端发送无负载认证事件；应用句柄尚未初始化时安全跳过。
    fn emit(&self, event: &str) {
        if let Some(app_handle) = self.context.try_app_handle() {
            if let Err(error) = app_handle.emit(event, ()) {
                log::error!("发送 {} 事件失败: {}", event, error);
            }
        }
    }
}

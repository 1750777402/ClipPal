use std::sync::Arc;

use tauri::Emitter;

use crate::{
    app_context::AppContext,
    domain::user::{
        FrontendCheckUsernameRequest, FrontendLoginRequest, FrontendRegisterRequest,
        FrontendSendEmailCodeRequest, LoginResponse, UserInfo,
    },
    infra::repositories::{AuthRepository, SettingsRepository, VipRepository},
};

/// 认证应用服务，负责编排认证仓储、VIP 初始化、设置更新和前端事件。
pub struct AuthService<'a> {
    context: &'a AppContext,
    repository: &'a dyn AuthRepository,
    settings_repository: &'a dyn SettingsRepository,
    vip_repository: Arc<dyn VipRepository>,
}

impl<'a> AuthService<'a> {
    /// 从应用上下文取得认证、设置和 VIP 仓储，创建一次 command 调用使用的服务实例。
    pub fn from_context(context: &'a AppContext) -> Self {
        Self {
            context,
            repository: context.repositories().auth(),
            settings_repository: context.repositories().settings(),
            vip_repository: context.repositories().vip_shared(),
        }
    }
    /// 执行用户登录；认证仓储负责远程请求和令牌持久化，登录成功后异步初始化 VIP 权益。
    pub async fn login(&self, param: FrontendLoginRequest) -> Result<LoginResponse, String> {
        log::info!("用户登录请求: {}", param.account);
        // 登录成功时仓储已经将令牌和用户信息写入安全存储。
        let response = self.repository.login(param).await?;
        log::info!("用户登录成功: {}", response.user_info.account);

        // VIP 检查可能访问网络和清理超限记录，不阻塞登录响应返回。
        let vip_repository = self.vip_repository.clone();
        tokio::spawn(async move {
            if let Err(error) = vip_repository.initialize_and_enforce_limits().await {
                log::error!("登录后 VIP 状态初始化失败: {}", error);
            }
        });

        Ok(response)
    }

    /// 注册用户账号，返回服务端创建的用户资料。
    pub async fn register(&self, param: FrontendRegisterRequest) -> Result<UserInfo, String> {
        log::info!("用户注册请求: {}", param.account);
        self.repository.register(param).await
    }

    /// 请求发送邮箱验证码，仓储负责转换参数和调用认证接口。
    pub async fn send_email_code(
        &self,
        param: FrontendSendEmailCodeRequest,
    ) -> Result<String, String> {
        self.repository.send_email_code(param).await
    }

    /// 退出当前账号，依次通知服务端、清除本地认证数据并关闭云同步。
    pub async fn logout(&self) -> Result<String, String> {
        // 仅在本地存在令牌时通知服务端，避免无意义的未认证请求。
        if self.repository.has_access_token() {
            self.repository.logout_remote().await;
        }

        // 本地认证状态必须清理成功，随后通知前端关闭登录相关界面状态。
        self.repository.clear_auth_data()?;
        self.emit("auth-cleared");

        // 登出后同步修改内存设置和持久化文件，防止重启后重新开启云同步。
        let updated_settings = self.context.update_settings(|settings| {
            if settings.cloud_sync == 0 {
                None
            } else {
                settings.cloud_sync = 0;
                Some(settings.clone())
            }
        });
        match updated_settings {
            Ok(Some(settings)) => {
                if let Err(error) = self.settings_repository.save(&settings) {
                    log::error!("禁用云同步设置失败: {}", error);
                }
            }
            Ok(None) => {}
            Err(error) => log::error!("禁用云同步设置失败: {}", error),
        }
        self.emit("cloud-sync-disabled");

        Ok("退出成功".to_string())
    }

    /// 判断本地是否存在访问令牌，用于前端进行轻量登录态检查。
    pub fn validate_token(&self) -> Result<bool, String> {
        Ok(self.repository.has_access_token())
    }

    /// 获取当前用户信息；本地没有用户资料时返回“未登录”错误。
    pub fn get_user_info(&self) -> Result<UserInfo, String> {
        self.repository
            .user_info()
            .ok_or_else(|| "用户未登录".to_string())
    }

    /// 恢复登录状态；令牌存在但用户资料损坏时主动清理不完整认证数据。
    pub fn check_login_status(&self) -> Result<Option<UserInfo>, String> {
        if self.repository.has_access_token() {
            match self.repository.user_info() {
                Some(user_info) => Ok(Some(user_info.into())),
                None => {
                    let _ = self.repository.clear_auth_data();
                    Ok(None)
                }
            }
        } else {
            Ok(None)
        }
    }

    /// 调用认证仓储检查用户名可用性。
    pub async fn check_username(
        &self,
        param: FrontendCheckUsernameRequest,
    ) -> Result<bool, String> {
        self.repository.check_username(param).await
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

        self.repository.update_nickname(nickname).await
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

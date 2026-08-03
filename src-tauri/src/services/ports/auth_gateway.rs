use async_trait::async_trait;

use crate::domain::user::{
    AuthSession, FrontendCheckUsernameRequest, FrontendLoginRequest, FrontendRegisterRequest,
    FrontendSendEmailCodeRequest, UserInfo,
};

#[async_trait]
/// 认证远程端口，定义应用服务需要的账号和会话操作，不暴露具体传输协议。
pub trait AuthGateway: Send + Sync {
    /// 使用账号密码创建完整认证会话。
    async fn login(&self, request: FrontendLoginRequest) -> Result<Option<AuthSession>, String>;

    /// 注册账号并返回服务端创建的用户资料。
    async fn register(&self, request: FrontendRegisterRequest) -> Result<Option<UserInfo>, String>;

    /// 请求服务端向指定邮箱发送验证码。
    async fn send_email_code(
        &self,
        request: FrontendSendEmailCodeRequest,
    ) -> Result<Option<bool>, String>;

    /// 使当前服务端会话退出。
    async fn logout(&self) -> Result<(), String>;

    /// 检查用户名是否可用于注册。
    async fn check_username(
        &self,
        request: FrontendCheckUsernameRequest,
    ) -> Result<Option<bool>, String>;

    /// 更新当前用户昵称。
    async fn update_nickname(&self, nickname: &str) -> Result<Option<bool>, String>;

    /// 使用 refresh token 获取新的完整认证会话。
    async fn refresh_session(&self, refresh_token: &str) -> Result<Option<AuthSession>, String>;
}

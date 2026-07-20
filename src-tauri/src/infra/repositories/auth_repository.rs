use async_trait::async_trait;

use crate::domain::user::{
    FrontendCheckUsernameRequest, FrontendLoginRequest, FrontendRegisterRequest,
    FrontendSendEmailCodeRequest, LoginResponse, UserInfo,
};

#[async_trait]
/// 认证仓储接口，统一封装认证 HTTP 请求和本地安全存储。
pub trait AuthRepository: Send + Sync {
    /// 登录并持久化服务端返回的令牌与用户资料。
    async fn login(&self, request: FrontendLoginRequest) -> Result<LoginResponse, String>;

    /// 注册新用户并返回服务端用户资料。
    async fn register(&self, request: FrontendRegisterRequest) -> Result<UserInfo, String>;

    /// 请求服务端发送邮箱验证码。
    async fn send_email_code(
        &self,
        request: FrontendSendEmailCodeRequest,
    ) -> Result<String, String>;

    /// 通知服务端当前会话退出；远程失败不影响本地清理流程。
    async fn logout_remote(&self);

    /// 判断本地安全存储中是否存在访问令牌。
    fn has_access_token(&self) -> bool;

    /// 从安全存储恢复当前用户资料。
    fn user_info(&self) -> Option<UserInfo>;

    /// 清除访问令牌、刷新令牌、用户资料和过期时间。
    fn clear_auth_data(&self) -> Result<(), String>;

    /// 检查用户名是否可以注册。
    async fn check_username(&self, request: FrontendCheckUsernameRequest) -> Result<bool, String>;

    /// 更新服务端昵称，并在成功后更新本地用户资料缓存。
    async fn update_nickname(&self, nickname: &str) -> Result<bool, String>;
}

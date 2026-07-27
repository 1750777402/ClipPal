use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
/// 前端展示和本地登录态使用的用户资料。
pub struct UserInfo {
    /// 服务端用户唯一标识。
    pub id: u64,
    /// 登录账号。
    pub account: String,
    /// 用户显示昵称。
    pub nickname: Option<String>,
    /// 绑定邮箱。
    pub email: Option<String>,
    /// 绑定手机号。
    pub phone: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
/// 登录成功响应，包含用户资料和后续认证所需的令牌信息。
pub struct LoginResponse {
    /// 已登录用户资料。
    pub user_info: UserInfo,
    /// 后续认证请求使用的访问令牌。
    pub token: String,
    /// 访问令牌有效时间，单位由服务端认证协议定义。
    pub expires_in: i32,
}

#[derive(Debug, Clone)]
/// 认证服务成功返回的完整会话，仅供后端认证流程和安全存储使用。
pub struct AuthSession {
    pub access_token: String,
    pub refresh_token: String,
    pub expires_in: i32,
    pub user_info: UserInfo,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
/// 前端登录请求参数。
pub struct FrontendLoginRequest {
    /// 用户输入的登录账号。
    pub account: String,
    /// 用户输入的明文密码，仅用于本次登录请求。
    pub password: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
/// 前端注册请求参数，字段命名保持与 Tauri IPC 契约一致。
pub struct FrontendRegisterRequest {
    /// 初始显示昵称。
    pub nickname: String,
    /// 注册账号。
    pub account: String,
    /// 注册密码。
    pub password: String,
    /// 前端再次确认的密码。
    pub confirm_password: String,
    /// 用于验证和找回账号的邮箱。
    pub email: String,
    /// 邮箱验证码。
    pub captcha: String,
    /// 可选手机号。
    pub phone: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
/// 发送邮箱验证码的请求参数。
pub struct FrontendSendEmailCodeRequest {
    /// 接收验证码的邮箱。
    pub email: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
/// 检查用户名可用性的请求参数。
pub struct FrontendCheckUsernameRequest {
    /// 需要检查可用性的用户名。
    pub username: String,
}

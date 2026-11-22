//! 同步客户端可复用实现；Tauri、认证、VIP 和部署配置均留在适配层。
//! 旧协议与新版有序同步分开放置。
pub mod queue;
pub mod transport;
#[cfg(feature = "sqlite")]
pub mod cursor;
pub mod lock;
pub mod auth;

/// 首期文本同步的传输端口和会话调度。
pub mod v2;

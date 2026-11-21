//! 同步客户端可复用实现；Tauri、认证、VIP 和部署配置均留在适配层。
//! 当前仍是既有 HTTP 协议，队列为内存队列，不具备持久化 Outbox 保证。
pub mod queue;
pub mod transport;
#[cfg(feature = "sqlite")]
pub mod cursor;
pub mod lock;
pub mod auth;

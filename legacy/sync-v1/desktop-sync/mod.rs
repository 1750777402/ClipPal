//! 桌面同步适配层：应用上下文、事件通知和现有网络调用的组装位置。
//! 共享 JSON 契约和客户端基础能力位于 clip-sync-protocol / clip-sync-client。

pub mod runtime;
#[cfg(feature = "sync")]
pub(crate) mod consumer;

#[cfg(feature = "sync")]
pub mod client;
#[cfg(feature = "sync")]
pub mod policy;

//! 自建同步的桌面适配与策略。
pub mod policy;

#[cfg(not(feature = "commercial-extension"))]
pub(crate) mod runtime;

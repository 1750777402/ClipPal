//! 单机版公共核心。路径、解密等应用配置由调用方注入。
//! 本 crate 不依赖 Tauri、剪贴板监听器、云端认证或商业服务。

pub mod domain;
pub mod errors;
pub mod repositories;
#[cfg(feature = "search")]
pub mod search;
#[cfg(feature = "sqlite")]
pub mod storage;
pub mod files;
#[cfg(feature = "crypto")]
pub mod crypto;

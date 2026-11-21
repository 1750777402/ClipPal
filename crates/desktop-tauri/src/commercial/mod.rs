//! 商业版扩展的稳定入口。
//!
//! 自建版不应依赖这里的实现。商业仓库后续可以将这些接口替换为自己的账号、
//! VIP、支付和配额实现，而不修改本地核心与通用同步协议。

pub mod auth_service;
pub mod vip_service;
pub mod context;
pub mod sync_policy;
pub mod domain;
pub mod ports;
pub mod http;
pub mod storage;
pub mod api;
pub mod token_manager;

//! 桌面本地能力组装层。
//!
//! 领域模型、持久化、搜索已迁入独立 clip-core；这里保留依赖 AppContext、
//! Tauri 和系统功能的应用服务，逐步把扩展策略改为依赖注入端口。
pub mod application;
pub mod policy;

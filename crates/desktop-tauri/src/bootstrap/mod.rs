//! 应用启动编排层。
//!
//! - `core`：日志、设置、数据库、搜索索引、AppContext 等核心资源初始化；
//! - `plugins`：Tauri 插件注册；
//! - `setup`：Tauri `.setup()` 阶段的窗口、托盘、菜单、快捷键、监听器初始化；
//! - `runtime`：Tauri `RunEvent` 阶段的后台任务启动和退出清理。

pub mod core;
pub mod plugins;
pub mod runtime;
pub mod setup;

pub use core::init_core;
pub use plugins::register_plugins;
pub use runtime::handle_run_event;
pub use setup::setup_app;

/// 登录项、首次启动和单实例回调共用同一个后台启动参数。
pub const AUTOSTART_ARG: &str = "--autostart";

/// 检查完整参数，避免路径中的同名文本被当成自启标记。
pub fn is_autostart(args: impl IntoIterator<Item = impl AsRef<std::ffi::OsStr>>) -> bool {
    args.into_iter().any(|arg| arg.as_ref() == AUTOSTART_ARG)
}

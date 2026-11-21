use tauri::{App, Manager};

use {
    crate::bootstrap::core::BootstrapCore,
    crate::desktop::listener,
    crate::desktop::shell::shortcut,
    crate::desktop::shell::menu,
    crate::desktop::shell::tray,
    crate::desktop::shell::window,
};

#[cfg(feature = "sync")]
use crate::sync::runtime::{start_cloud_file_download_timer, start_cloud_sync_timer};

/// 执行 Tauri `.setup()` 阶段初始化。
///
/// 这个阶段可以拿到 `AppHandle`、窗口、托盘、插件 state 等 Tauri 运行期资源。
/// 因此这里负责：
/// - 把 `AppHandle` 和主窗口写入 `AppContext`；
/// - 初始化菜单、托盘、主窗口、全局快捷键；
/// - 接入剪贴板监听器；
/// - 启动依赖 `AppHandle` 的后台任务。
pub fn setup_app(app: &mut App, core: BootstrapCore) -> tauri::Result<()> {
    let app_handle = app.handle().clone();

    if let Err(e) = core.app_context.set_app_handle(app_handle) {
        log::warn!("设置 AppHandle 到 AppContext 失败: {}", e);
    }

    init_desktop_shell(app, &core)?;
    start_setup_tasks(app, &core);

    Ok(())
}

/// 初始化桌面外壳能力。
///
/// 菜单、托盘、主窗口、全局快捷键、剪贴板监听都属于 Tauri / 系统能力，
/// 所以放在 setup 阶段集中编排。
fn init_desktop_shell(app: &mut App, core: &BootstrapCore) -> tauri::Result<()> {
    let _ = menu::init_menu(app);

    tray::create_tray(app.handle(), core.app_context.clone())?;

    window::init_main_window(
        app,
        core.app_context.window_focus_state(),
        core.app_context.window_hide_flag(),
    )?;
    if let Some(main_window) = app.get_webview_window("main") {
        if let Err(e) = core.app_context.set_main_window(main_window) {
            log::warn!("设置主窗口到 AppContext 失败: {}", e);
        }
    }

    // 普通启动显示前捕获目标；自动启动保持后台运行。
    if !std::env::args().any(|arg| arg == "--autostart") {
        window::show_main_window(&core.app_context).map_err(|error| std::io::Error::other(error.to_string()))?;
    }

    if let Err(error) = shortcut::init_global_shortcut(app, core.app_context.clone()) { log::warn!("全局快捷键不可用: {}", error); }

    listener::init_clip_board_listener(app, core.clipboard_event_manager.clone())?;

    Ok(())
}

/// 启动 setup 阶段即可运行的后台任务。
///
/// 这些任务需要 `AppHandle` 或数据库连接，但不依赖 `RunEvent::Ready`。
#[cfg(feature = "sync")]
fn start_setup_tasks(app: &mut App, core: &BootstrapCore) {
    let app_handle = app.handle().clone();
    let db = core.app_context.core().db().clone();
    tokio::spawn(async move {
        start_cloud_sync_timer(app_handle, db).await;
    });

    let app_handle_download = app.handle().clone();
    tokio::spawn(async move {
        start_cloud_file_download_timer(app_handle_download).await;
    });
}

#[cfg(not(feature = "sync"))]
fn start_setup_tasks(_app: &mut App, _core: &BootstrapCore) {
    log::info!("当前构建未包含云同步运行时，跳过同步后台任务");
}

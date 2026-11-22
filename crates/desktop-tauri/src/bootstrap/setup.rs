use tauri::{App, Manager};

use {
    crate::bootstrap::core::BootstrapCore,
    crate::desktop::listener,
    crate::desktop::shell::shortcut,
    crate::desktop::shell::menu,
    crate::desktop::shell::tray,
    crate::desktop::shell::window,
};


/// 执行 Tauri `.setup()` 阶段初始化。
///
/// 这个阶段可以拿到 `AppHandle`、窗口、托盘、插件 state 等 Tauri 运行期资源。
/// 因此这里负责：
/// - 把 `AppHandle` 和主窗口写入 `AppContext`；
/// - 初始化菜单、托盘、主窗口、全局快捷键；
/// - 接入剪贴板监听器；
/// - 启动依赖 `AppHandle` 的后台任务。
pub fn setup_app(app: &mut App, core: BootstrapCore, autostart: bool) -> tauri::Result<()> {
    // 登录后只挂在菜单栏运行，避免 macOS 把进程作为普通前台应用激活。
    #[cfg(target_os = "macos")]
    if autostart {
        app.set_activation_policy(tauri::ActivationPolicy::Accessory);
    }

    let app_handle = app.handle().clone();

    if let Err(e) = core.app_context.set_app_handle(app_handle) {
        log::warn!("设置 AppHandle 到 AppContext 失败: {}", e);
    }

    init_desktop_shell(app, &core, autostart)?;
    start_setup_tasks(app, &core);

    Ok(())
}

/// 初始化桌面外壳能力。
///
/// 菜单、托盘、主窗口、全局快捷键、剪贴板监听都属于 Tauri / 系统能力，
/// 所以放在 setup 阶段集中编排。
fn init_desktop_shell(app: &mut App, core: &BootstrapCore, autostart: bool) -> tauri::Result<()> {
    let _ = menu::init_menu(app);

    tray::create_tray(app.handle(), core.app_context.clone())?;

    window::init_main_window(
        app,
        core.app_context.window_focus_state(),
        core.app_context.window_hide_flag(),
    )?;
    let main_window = app.get_webview_window("main").ok_or(tauri::Error::FailedToReceiveMessage)?;
    core.app_context.set_main_window(main_window.clone())
        .map_err(|error| std::io::Error::other(error.to_string()))?;

    // 普通启动显示前捕获目标；自动启动保持后台运行。
    if autostart {
        main_window.hide()?;
        log::info!("登录自启，主窗口保持隐藏");
    } else {
        window::show_main_window(&core.app_context).map_err(|error| std::io::Error::other(error.to_string()))?;
    }

    if let Err(error) = shortcut::init_global_shortcut(app, core.app_context.clone()) { log::warn!("全局快捷键不可用: {}", error); }

    listener::init_clip_board_listener(app, core.clipboard_event_manager.clone())?;

    Ok(())
}

/// 旧同步运行时依赖旧数据库结构；新同步扩展完成前不启动旧任务。
fn start_setup_tasks(_app: &mut App, _core: &BootstrapCore) {}

use tauri::{App, Manager};

use crate::{
    biz::{
        cloud_sync_timer::start_cloud_sync_timer,
        download_cloud_file::start_cloud_file_download_timer,
        update_checker::check_update_on_startup,
    },
    bootstrap::core::BootstrapCore,
    clip_board_listener, global_shortcut, menu,
    services::clipboard_service::ClipboardService,
    tray, window,
};

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

    // 普通启动会直接显示主窗口，因此也要在显示前捕获当时的前台应用。
    if let Err(error) =
        ClipboardService::from_context(core.app_context.as_ref()).capture_auto_paste_target()
    {
        log::warn!("启动时保存自动粘贴目标窗口失败: {}", error);
    }

    let _ = window::init_main_window(
        app,
        core.app_context.window_focus_count(),
        core.app_context.window_hide_flag(),
    );
    if let Some(main_window) = app.get_webview_window("main") {
        if let Err(e) = core.app_context.set_main_window(main_window) {
            log::warn!("设置主窗口到 AppContext 失败: {}", e);
        }
    }

    let _ = global_shortcut::init_global_shortcut(app, core.app_context.clone());

    let _ =
        clip_board_listener::init_clip_board_listener(app, core.clipboard_event_manager.clone());

    Ok(())
}

/// 启动 setup 阶段即可运行的后台任务。
///
/// 这些任务需要 `AppHandle` 或数据库连接，但不依赖 `RunEvent::Ready`。
fn start_setup_tasks(app: &mut App, core: &BootstrapCore) {
    let app_handle = app.handle().clone();
    let db = core.db.clone();
    tokio::spawn(async move {
        start_cloud_sync_timer(app_handle, db).await;
    });

    let app_handle_download = app.handle().clone();
    tokio::spawn(async move {
        start_cloud_file_download_timer(app_handle_download).await;
    });

    let app_handle_update = app.handle().clone();
    tokio::spawn(async move {
        tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
        check_update_on_startup(app_handle_update).await;
    });
}

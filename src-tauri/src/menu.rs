#[cfg(target_os = "macos")]
use tauri::{Manager, menu::{MenuBuilder, SubmenuBuilder, PredefinedMenuItem}};

use tauri::App;

/// 初始化 macOS 菜单栏
///
/// 提供极简菜单：只保留"退出"功能，去掉所有不必要的菜单项
#[cfg(target_os = "macos")]
pub fn init_menu(app: &App) -> tauri::Result<()> {
    let app_handle = app.handle();

    // 创建退出菜单项（中文显示）
    let quit_item = PredefinedMenuItem::quit(app_handle, Some("退出 ClipPal"))?;

    // 创建应用子菜单（包含退出选项）
    let app_menu = SubmenuBuilder::new(app_handle, "ClipPal")
        .item(&quit_item)
        .build()?;

    // 创建主菜单栏
    let menu = MenuBuilder::new(app_handle)
        .item(&app_menu)
        .build()?;

    // 设置应用菜单
    app.set_menu(menu)?;

    log::info!("macOS 菜单栏已初始化（极简模式 - 仅退出功能）");

    Ok(())
}

/// Windows 平台不需要设置菜单
#[cfg(not(target_os = "macos"))]
pub fn init_menu(_app: &App) -> tauri::Result<()> {
    // Windows 上不设置菜单栏
    Ok(())
}

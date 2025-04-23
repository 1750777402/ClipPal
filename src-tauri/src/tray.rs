use tauri::image::Image;
use tauri::{tray::TrayIconBuilder, Manager, Runtime};
use tauri::menu::{Menu, MenuItem};
use tauri::tray::{MouseButton, TrayIconEvent};

pub fn create_tray<R: Runtime>(app: &tauri::AppHandle<R>) -> tauri::Result<()> {
    // 为系统创建托盘图标
    let icon = Image::from_bytes(include_bytes!("../icons/icon.png"))?;
    let quit_i = MenuItem::with_id(app, "quit", "退出", true, None::<&str>)?;
    let set_sys = MenuItem::with_id(app, "setSys", "设置", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&set_sys, &quit_i])?;
    let _ = TrayIconBuilder::with_id("tray")
        .tooltip("ClipPal")
        .icon(icon)
        // 设置托盘图标菜单
        .menu(&menu)
        // 防止菜单在鼠标左键单击时弹出
        .show_menu_on_left_click(false)
        // 托盘菜单点击事件
        .on_menu_event(|app, event| match event.id.as_ref() {
            "quit" => {
                app.exit(0);
            }
            _ => {
                println!("menu item {:?} not handled", event.id);
            }
        })
        // 托盘图标响应鼠标事件
        .on_tray_icon_event(|tray, event| match event {
            // 鼠标左键双击事件
            TrayIconEvent::DoubleClick {
                button: MouseButton::Left,
                ..
            } => {
                println!("left click pressed and released");
                // in this example, let's show and focus the main window when the tray is clicked
                let app = tray.app_handle();
                if let Some(window) = app.get_webview_window("main") {
                    let _ = window.show();
                    let _ = window.set_focus();
                }
            }
            _ => {}
        })
        .build(app);
    Ok(())
}

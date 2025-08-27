use tauri::image::Image;
use tauri::menu::{Menu, MenuItem};
use tauri::tray::{MouseButton, TrayIconEvent};
use tauri::{AppHandle, Emitter};
use tauri::{Manager, Runtime, tray::TrayIconBuilder};

use crate::{CONTEXT, auto_paste};

pub fn create_tray<R: Runtime>(app: &tauri::AppHandle<R>) -> tauri::Result<()> {
    // 为系统创建托盘图标
    let icon = Image::from_bytes(include_bytes!("../icons/icon_128x128.png"))?;
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
            "setSys" => {
                // 通知前端显示系统设置窗口
                let app_handle = CONTEXT.get::<AppHandle>();
                if let Some(window) = app.get_webview_window("main") {
                    let visible = window.is_visible().unwrap_or(false);
                    if !visible {
                        let _ = window.show();
                    }
                }
                let _ = app_handle.emit("open_settings_winodws", ());
            }
            _ => {
                log::warn!("菜单项 {:?} 未处理", event.id);
            }
        })
        // 托盘图标响应鼠标事件
        .on_tray_icon_event(|tray, event| match event {
            // 鼠标左键双击事件
            TrayIconEvent::DoubleClick {
                button: MouseButton::Left,
                ..
            } => {
                let app = tray.app_handle();

                // 先尝试保存当前焦点窗口（在显示我们的窗口之前）
                auto_paste::save_foreground_window();

                // 如果窗口已经可见，先隐藏它，让用户的应用重新获得焦点
                if let Some(window) = app.get_webview_window("main") {
                    let is_visible = window.is_visible().unwrap_or(false);
                    if is_visible {
                        let _ = window.hide();
                        // 等待一小段时间让用户应用获得焦点，然后重新保存
                        std::thread::sleep(std::time::Duration::from_millis(50));
                        auto_paste::save_foreground_window();
                        // 重新显示窗口
                        let _ = window.show();
                        let _ = window.set_focus();
                    } else {
                        let _ = window.show();
                        let _ = window.set_focus();
                    }
                }
            }
            _ => {}
        })
        .build(app);
    Ok(())
}

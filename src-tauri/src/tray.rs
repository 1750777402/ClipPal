use tauri::{
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    Emitter, Manager, PhysicalPosition, Runtime,
};
use tauri::image::Image;

pub fn create_tray<R: Runtime>(app: &tauri::AppHandle<R>) -> tauri::Result<()> {
    // 为系统创建托盘图标
    let icon = Image::from_bytes(include_bytes!("../icons/icon.png"))?;
    let _ = TrayIconBuilder::with_id("tray")
        .tooltip("ClipPal")
        .icon(icon)
        .on_tray_icon_event(|tray, event| match event {
            TrayIconEvent::Click {
                id: _,
                position,
                rect: _,
                button,
                button_state,
            } => match button {
                MouseButton::Left => {
                    let windows = tray.app_handle().webview_windows();
                    for (name, window) in windows {
                        if name == "login" || name == "home" {
                            window.show().unwrap();
                            window.unminimize().unwrap();
                            window.set_focus().unwrap();
                            break;
                        }
                    }
                }
                MouseButton::Right if MouseButtonState::Down == button_state => {
                    // 状态栏图标按下右键时显示状态栏菜单
                    let tray_window = tray.app_handle().get_webview_window("tray").unwrap();
                    if let Ok(outer_size) = tray_window.outer_size() {
                        tray_window
                            .set_position(PhysicalPosition::new(
                                position.x,
                                position.y - outer_size.height as f64,
                            ))
                            .unwrap();
                        tray_window.set_always_on_top(true).unwrap();
                        tray_window.show().unwrap();
                        tray_window.set_focus().unwrap();
                    }
                }
                _ => {}
            },
            #[cfg(target_os = "windows")]
            TrayIconEvent::Enter {
                id: _,
                position: _,
                rect: _,
            } => {
                tray.app_handle()
                    .emit_to("notify", "notify_enter", &tray.rect().unwrap())
                    .unwrap();
            }
            #[cfg(target_os = "windows")]
            TrayIconEvent::Leave {
                id: _,
                position: _,
                rect: _,
            } => {
                tray.app_handle()
                    .emit_to("notify", "notify_leave", ())
                    .unwrap();
            }
            _ => {}
        })
        .build(app);
    Ok(())
}
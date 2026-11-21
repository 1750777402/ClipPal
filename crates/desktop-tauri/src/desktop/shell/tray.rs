use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tauri::image::Image;
use tauri::menu::{Menu, MenuItem};
use tauri::tray::{MouseButton, TrayIconEvent};
#[cfg(target_os = "macos")]
use tauri::tray::MouseButtonState;
use tauri::{tray::TrayIconBuilder, Emitter};
use crate::app_context::AppContext;
use super::window::show_main_window;
#[cfg(target_os = "macos")]
use super::window::hide_main_window;

/// 防抖控制结构，使用单调时间，避免系统校时和重复鼠标事件的影响。
#[derive(Default)]
struct TrayClickDebounce(Mutex<Option<Instant>>);
impl TrayClickDebounce {
    /// 检查是否应该处理点击事件（防抖）。
    fn should_process_click(&self) -> bool {
        let Ok(mut last) = self.0.lock() else { return false; };
        let now = Instant::now();
        if last.is_some_and(|last| now.duration_since(last) < Duration::from_millis(300)) {
            return false;
        }
        *last = Some(now);
        true
    }
}

pub fn create_tray(app: &tauri::AppHandle, app_context: Arc<AppContext>) -> tauri::Result<()> {
    // 为系统创建托盘图标
    let icon = Image::from_bytes(crate::desktop::config::tray_icon_png().map_err(|error| std::io::Error::other(error.to_string()))?)?;
    let quit_i = MenuItem::with_id(app, "quit", "退出", true, None::<&str>)?;
    let set_sys = MenuItem::with_id(app, "setSys", "设置", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&set_sys, &quit_i])?;

    // 创建防抖控制器
    let debounce = TrayClickDebounce::default();
    #[cfg(target_os = "macos")]
    let click_visible = Mutex::new(None::<bool>);

    TrayIconBuilder::with_id("tray")
        .tooltip("ClipPal")
        .icon(icon)
        // 设置托盘图标菜单
        .menu(&menu)
        // 防止菜单在鼠标左键单击时弹出
        .show_menu_on_left_click(false)
        // 托盘菜单点击事件
        .on_menu_event({
            let context = app_context.clone();
            move |app, event| match event.id.as_ref() {
                "quit" => app.exit(0),
                "setSys" => {
                    // 通知前端显示系统设置窗口，显示和焦点处理与快捷键保持一致。
                    if let Err(error) = show_main_window(&context) {
                        log::error!("打开设置窗口失败: {}", error);
                        return;
                    }
                    if let Err(error) = app.emit("open_settings_windows", ()) {
                        log::error!("通知前端打开设置失败: {}", error);
                    }
                }
                _ => log::warn!("菜单项 {:?} 未处理", event.id),
            }
        })
        // 托盘图标响应鼠标事件；Windows 双击唤起，macOS 单击切换。
        .on_tray_icon_event(move |_tray, event| {
            #[cfg(target_os = "windows")]
            if matches!(event, TrayIconEvent::DoubleClick { button: MouseButton::Left, .. })
                && debounce.should_process_click() {
                // 可见时直接聚焦，不再隐藏、阻塞等待后重新捕获目标。
                if let Err(error) = show_main_window(&app_context) {
                    log::error!("托盘唤起窗口失败: {}", error);
                }
            }
            #[cfg(target_os = "macos")]
            if matches!(event, TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Down, .. }) {
                // 失焦隐藏可能先于释放事件发生，按下时保存本次点击意图。
                if let Ok(mut visible) = click_visible.lock() {
                    *visible = app_context.main_window().ok().and_then(|window| window.is_visible().ok());
                }
            }
            #[cfg(target_os = "macos")]
            if matches!(event, TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, .. })
                && debounce.should_process_click() {
                // 每次完整点击只处理鼠标释放，避免按下/释放两次切换。
                let result = app_context.main_window().and_then(|window| {
                    let visible = click_visible.lock().ok().and_then(|mut value| value.take())
                        .unwrap_or(window.is_visible().map_err(|error| crate::errors::AppError::Window(error.to_string()))?);
                    if visible {
                        hide_main_window(&app_context)
                    } else {
                        show_main_window(&app_context)
                    }
                });
                if let Err(error) = result { log::error!("托盘切换窗口失败: {}", error); }
            }
        })
        .build(app)?;
    Ok(())
}

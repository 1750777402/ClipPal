use tauri::image::Image;
use tauri::menu::{Menu, MenuItem};
use tauri::tray::{MouseButton, TrayIconEvent};
use tauri::{tray::TrayIconBuilder, Manager, Runtime};
use tauri::{AppHandle, Emitter};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;

use crate::{auto_paste, CONTEXT};

/// 防抖控制结构
#[derive(Debug)]
struct TrayClickDebounce {
    last_click_time: AtomicU64,
    is_processing: AtomicBool,
}

impl TrayClickDebounce {
    fn new() -> Arc<Self> {
        Arc::new(Self {
            last_click_time: AtomicU64::new(0),
            is_processing: AtomicBool::new(false),
        })
    }

    /// 检查是否应该处理点击事件（防抖）
    fn should_process_click(&self) -> bool {
        // 如果正在处理，直接返回 false
        if self.is_processing.load(Ordering::SeqCst) {
            log::debug!("正在处理上一个点击事件，忽略此次点击");
            return false;
        }

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;

        let last = self.last_click_time.load(Ordering::SeqCst);

        // 如果距离上次点击少于 300ms，认为是重复点击
        if now - last < 300 {
            log::debug!("距离上次点击时间过短 ({}ms)，忽略此次点击", now - last);
            return false;
        }

        // 标记为正在处理
        self.is_processing.store(true, Ordering::SeqCst);
        self.last_click_time.store(now, Ordering::SeqCst);
        true
    }

    /// 完成处理
    fn finish_processing(&self) {
        self.is_processing.store(false, Ordering::SeqCst);
    }
}

pub fn create_tray<R: Runtime>(app: &tauri::AppHandle<R>) -> tauri::Result<()> {
    // 为系统创建托盘图标
    let icon = Image::from_bytes(include_bytes!("../icons/icon_128x128.png"))?;
    let quit_i = MenuItem::with_id(app, "quit", "退出", true, None::<&str>)?;
    let set_sys = MenuItem::with_id(app, "setSys", "设置", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&set_sys, &quit_i])?;

    // 创建防抖控制器
    let debounce = TrayClickDebounce::new();

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
                let _ = app_handle.emit("open_settings_windows", ());
            }
            _ => {
                log::warn!("菜单项 {:?} 未处理", event.id);
            }
        })
        // 托盘图标响应鼠标事件
        .on_tray_icon_event({
            let debounce = Arc::clone(&debounce);
            move |tray, event| {
                log::debug!("托盘图标事件触发: {:?}", event);
                match event {
                // 鼠标左键双击事件
                TrayIconEvent::DoubleClick {
                    button: MouseButton::Left,
                    ..
                } => {
                    // 防抖检查
                    if !debounce.should_process_click() {
                        return;
                    }

                    log::info!("托盘图标双击事件");
                    let app = tray.app_handle();

                    // 如果窗口已经可见，先隐藏它，让用户的应用重新获得焦点
                    if let Some(window) = app.get_webview_window("main") {
                        use crate::CONTEXT;
                        use crate::window::{WindowFocusCount};

                        let is_visible = window.is_visible().unwrap_or(false);
                        log::debug!("窗口当前可见状态: {}", is_visible);

                        if is_visible {
                            let _ = window.hide();
                            // 等待一小段时间让用户应用获得焦点，然后重新保存
                            std::thread::sleep(std::time::Duration::from_millis(50));
                            auto_paste::save_foreground_window();

                            // 重新显示窗口
                            let _ = window.show();
                            let _ = window.set_focus();
                            log::debug!("窗口已重新显示并聚焦");

                            // 重置焦点计数器，确保第一次失去焦点不会隐藏窗口
                            let focus_count = CONTEXT.get::<WindowFocusCount>();
                            focus_count.reset();
                            log::debug!("已重置焦点丢失计数器");

                            // 完成防抖处理
                            let debounce_clone = Arc::clone(&debounce);
                            std::thread::spawn(move || {
                                std::thread::sleep(std::time::Duration::from_millis(100));
                                debounce_clone.finish_processing();
                            });
                        } else {
                            // 先尝试保存当前焦点窗口（在显示我们的窗口之前）
                            auto_paste::save_foreground_window();

                            // 显示并聚焦窗口
                            let _ = window.show();
                            let _ = window.set_focus();
                            log::debug!("窗口已显示并聚焦");

                            // 完成防抖处理
                            let debounce_clone = Arc::clone(&debounce);
                            std::thread::spawn(move || {
                                std::thread::sleep(std::time::Duration::from_millis(100));
                                debounce_clone.finish_processing();
                            });
                        }
                    } else {
                        debounce.finish_processing();
                    }
                }
                // macOS 上可能更习惯使用单击
                TrayIconEvent::Click {
                    button: MouseButton::Left,
                    ..
                } => {
                    #[cfg(target_os = "macos")]
                    {
                        // 防抖检查
                        if !debounce.should_process_click() {
                            return;
                        }

                        log::info!("托盘图标单击事件 (macOS)");
                        let app = tray.app_handle();

                        if let Some(window) = app.get_webview_window("main") {
                            let is_visible = window.is_visible().unwrap_or(false);
                            log::debug!("窗口当前可见状态: {}", is_visible);

                            if is_visible {
                                let _ = window.hide();
                                debounce.finish_processing();
                            } else {
                                // 保存前台窗口
                                auto_paste::save_foreground_window();

                                // 显示并聚焦窗口
                                let _ = window.show();
                                let _ = window.set_focus();
                                log::debug!("窗口已显示并聚焦");

                                // 完成防抖处理
                                let debounce_clone = Arc::clone(&debounce);
                                std::thread::spawn(move || {
                                    std::thread::sleep(std::time::Duration::from_millis(100));
                                    debounce_clone.finish_processing();
                                });
                            }
                        } else {
                            debounce.finish_processing();
                        }
                    }
                }
                _ => {}
            }
        }
        })
        .build(app);
    Ok(())
}

use std::sync::atomic::{AtomicU64, Ordering};

use tauri::{App, WindowEvent};
use tauri::{Manager, PhysicalPosition, PhysicalSize};

use crate::CONTEXT;
use tauri::{AppHandle, Emitter};

pub fn init_main_window(app: &App) -> tauri::Result<()> {
    // 获取主显示器
    let main_window = app.get_webview_window("main").unwrap();
    // 获取主显示器信息
    let monitor = main_window
        .primary_monitor()
        .expect("Failed to get primary monitor")
        .expect("No primary monitor found");

    // 获取显示器参数
    let screen_size = monitor.size();
    let screen_width = screen_size.width as i32;
    let screen_height = screen_size.height as i32;

    // 设置窗口参数 - 右侧贴边，高度与屏幕同高  宽度为屏幕宽度六分之一，最小400
    let window_width = (screen_width / 6).max(400);
    let x_position = (screen_width - window_width).max(0);

    // 设置窗口大小和位置
    main_window.set_size(PhysicalSize::new(window_width, screen_height))?;
    main_window.set_position(PhysicalPosition::new(x_position, 0))?;
    // 延迟显示
    std::thread::sleep(std::time::Duration::from_millis(100));
    main_window.show().unwrap();
    // 设置主窗口获取焦点
    let _ = main_window.set_focus();
    let main1 = main_window.clone();

    // 设置一个窗口计数器，用于记录窗口是否被聚焦或者失去焦点
    CONTEXT.set(WindowFocusCount::default());
    main_window.on_window_event(move |event| match event {
        WindowEvent::Focused(false) => {
            let window_focus_count = CONTEXT.get::<WindowFocusCount>();
            if window_focus_count.inc() > 1 {
                main1.hide().unwrap();
            }
        }
        WindowEvent::Focused(true) => {
            // 触发粘贴板变化事件通知前端
            let app_handle = CONTEXT.get::<AppHandle>();
            let _ = app_handle.emit("clip_record_change", ());
        }
        _ => {}
    });
    Ok(())
}

#[derive(Default, Debug)]
pub struct WindowFocusCount {
    pub lost_count: AtomicU64,
}

impl WindowFocusCount {
    pub fn inc(&self) -> u64 {
        self.lost_count.fetch_add(1, Ordering::SeqCst)
    }
}

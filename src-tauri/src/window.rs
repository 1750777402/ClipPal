use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

use tauri::{App, WindowEvent};
use tauri::{Manager, PhysicalPosition, PhysicalSize};

use crate::CONTEXT;

pub fn init_main_window(app: &App) -> tauri::Result<()> {
    // 检查是否为开机自启
    let is_autostart = std::env::args().any(|arg| arg == "--autostart");
    
    // 获取主显示器
    let main_window = app.get_webview_window("main")
        .ok_or_else(|| {
            log::error!("无法获取主窗口");
            tauri::Error::FailedToReceiveMessage
        })?;
    
    // 获取主显示器信息
    let monitor = main_window
        .primary_monitor()
        .map_err(|e| {
            log::error!("获取主显示器失败: {}", e);
            e
        })?
        .ok_or_else(|| {
            log::error!("未找到主显示器");
            tauri::Error::FailedToReceiveMessage
        })?;

    // 获取显示器参数
    let screen_size = monitor.size();
    let screen_width = screen_size.width as i32;
    let screen_height = screen_size.height as i32;

    // 设置窗口参数 - 右侧贴边，高度与屏幕同高  宽度为屏幕宽度六分之一，最小400
    let window_width = (screen_width / 6).max(400);
    // 设置x轴时在右侧贴边，但是留出一点距离防止边框溢出
    let x_position = (screen_width - window_width - 8).max(0);

    // 设置窗口大小和位置
    main_window.set_size(PhysicalSize::new(window_width, screen_height))?;
    main_window.set_position(PhysicalPosition::new(x_position, 0))?;
    
    // 只有在非开机启动时才显示窗口
    if !is_autostart {
        // 延迟显示
        std::thread::sleep(std::time::Duration::from_millis(100));
        if let Err(e) = main_window.show() {
            log::error!("显示主窗口失败: {}", e);
            return Err(e);
        }
        
        // 设置主窗口获取焦点
        if let Err(e) = main_window.set_focus() {
            log::error!("设置窗口焦点失败: {}", e);
            // 这个不是致命错误，继续执行
        }
        
        log::info!("主窗口初始化完成并显示");
    } else {
        log::info!("开机自启模式：主窗口初始化完成但不显示");
    }
    let main1 = main_window.clone();

    // 设置一个窗口失去焦点的计数器，用于记录窗口是否被聚焦或者失去焦点
    CONTEXT.set(WindowFocusCount::default());
    // 设置一个窗口隐藏标志，用于判断窗口是否被隐藏
    CONTEXT.set(WindowHideFlag::default());
    main_window.on_window_event(move |event| match event {
        WindowEvent::Focused(false) => {
            let window_focus_count = CONTEXT.get::<WindowFocusCount>();
            let window_hide_flag = CONTEXT.get::<WindowHideFlag>();
            if window_focus_count.inc() >= 1 && window_hide_flag.is_can_hide() {
                if let Err(e) = main1.hide() {
                    log::error!("隐藏窗口失败: {}", e);
                }
            }
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

#[derive(Debug)]
pub struct WindowHideFlag {
    /// 主窗口是否可以隐藏标识
    pub hide_flag: AtomicBool,
}

impl Default for WindowHideFlag {
    fn default() -> Self {
        Self {
            // 默认窗口可以隐藏
            hide_flag: AtomicBool::new(true),
        }
    }
}

impl WindowHideFlag {
    /// 窗口可以隐藏
    pub fn set_can_hide(&self) {
        self.hide_flag.store(true, Ordering::SeqCst);
    }
    /// 窗口不可以隐藏
    pub fn set_no_hide(&self) {
        self.hide_flag.store(false, Ordering::SeqCst);
    }
    /// 窗口是否可以隐藏
    pub fn is_can_hide(&self) -> bool {
        self.hide_flag.load(Ordering::SeqCst)
    }
}

/// 窗口隐藏保护  作用域守卫
pub struct WindowHideGuard<'a> {
    flag: &'a WindowHideFlag,
}

impl<'a> WindowHideGuard<'a> {
    pub fn new(flag: &'a WindowHideFlag) -> Self {
        flag.set_no_hide();
        Self { flag }
    }
}

/// 实现Drop  当作用域失效时，保证自动恢复主窗口可以隐藏
impl<'a> Drop for WindowHideGuard<'a> {
    fn drop(&mut self) {
        self.flag.set_can_hide();
    }
}

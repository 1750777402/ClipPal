// 抑制 cocoa crate 的弃用警告
#![allow(deprecated)]

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

use tauri::{App, WindowEvent};
use tauri::{Manager, PhysicalPosition, PhysicalSize};

use crate::CONTEXT;

// macOS系统API导入
#[cfg(target_os = "macos")]
use objc::{msg_send, sel, sel_impl};

pub fn init_main_window(app: &App) -> tauri::Result<()> {
    // 获取主显示器
    let main_window = app.get_webview_window("main").ok_or_else(|| {
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
    let scale_factor = monitor.scale_factor();

    log::info!(
        "显示器信息: {}x{}, 缩放比例: {}",
        screen_width,
        screen_height,
        scale_factor
    );

    // 智能窗口宽度计算
    let window_width = calculate_optimal_width(screen_width, scale_factor);

    // 获取系统工作区域 - 准确计算顶部偏移
    let (work_area_top, x_position) =
        get_accurate_work_area(screen_width, window_width, scale_factor);

    // 计算窗口高度 - macOS 和 Windows 不同的策略
    #[cfg(target_os = "macos")]
    let window_height = {
        // macOS: 从菜单栏下方到屏幕底部（Dock 会遮挡底部，但窗口置顶会显示在 Dock 上方）
        use cocoa::appkit::NSScreen;
        use cocoa::base::nil;
        use cocoa::foundation::NSRect;

        let height = unsafe {
            let main_screen = NSScreen::mainScreen(nil);
            if main_screen != nil {
                let screen_frame: NSRect = msg_send![main_screen, frame];
                let visible_frame: NSRect = msg_send![main_screen, visibleFrame];

                // NSScreen 返回的是逻辑像素，需要转换为物理像素
                // 物理像素 = 逻辑像素 × scale_factor
                let logical_height = visible_frame.size.height;
                let physical_height = (logical_height * scale_factor) as i32;

                log::info!(
                    "macOS 屏幕总高度(逻辑): {}px, 可见区域高度(逻辑): {}px, 物理高度: {}px",
                    screen_frame.size.height as i32,
                    logical_height as i32,
                    physical_height
                );

                physical_height
            } else {
                log::warn!("无法获取 macOS 可见区域，使用屏幕高度");
                screen_height - work_area_top
            }
        };
        height
    };

    #[cfg(target_os = "windows")]
    let window_height = {
        // Windows: 使用全屏高度，底部可以被遮挡
        screen_height
    };

    let y_position = work_area_top;

    log::info!(
        "计算得出窗口尺寸: {}x{}, 位置: ({}, {})",
        window_width,
        window_height,
        x_position,
        y_position
    );

    // 设置窗口大小和位置
    main_window.set_size(PhysicalSize::new(window_width, window_height))?;
    main_window.set_position(PhysicalPosition::new(x_position, y_position))?;

    // macOS 特定配置：设置窗口始终置顶，确保在菜单栏和 Dock 上方
    #[cfg(target_os = "macos")]
    {
        if let Err(e) = main_window.set_always_on_top(true) {
            log::error!("设置窗口置顶失败: {}", e);
        } else {
            log::info!("macOS 窗口已设置为始终置顶");
        }
    }

    // 延迟显示
    std::thread::sleep(std::time::Duration::from_millis(100));

    let args: Vec<String> = std::env::args().collect();
    if !args.contains(&"--autostart".to_string()) {
        if let Err(e) = main_window.show() {
            log::error!("显示主窗口失败: {}", e);
            return Err(e);
        }
        // 设置主窗口获取焦点
        if let Err(e) = main_window.set_focus() {
            log::error!("设置窗口焦点失败: {}", e);
        }
    }

    let main1 = main_window.clone();

    // 设置一个窗口失去焦点的计数器，用于记录窗口是否被聚焦或者失去焦点
    CONTEXT.set(WindowFocusCount::default());
    // 设置一个窗口隐藏标志，用于判断窗口是否被隐藏
    CONTEXT.set(WindowHideFlag::default());

    main_window.on_window_event(move |event| match event {
        WindowEvent::Focused(false) => {
            log::debug!("窗口失去焦点事件触发");

            let window_focus_count = CONTEXT.get::<WindowFocusCount>();
            let window_hide_flag = CONTEXT.get::<WindowHideFlag>();
            let count = window_focus_count.inc();
            let can_hide = window_hide_flag.is_can_hide();

            log::debug!("失去焦点计数: {}, 可以隐藏: {}", count, can_hide);

            // 判断是否应该隐藏窗口（平台特定逻辑）
            let should_hide = {
                #[cfg(target_os = "macos")]
                {
                    // macOS: 只要允许隐藏就隐藏（不检查计数）
                    // 通过 WindowHideFlag 的临时保护机制避免误操作
                    can_hide
                }

                #[cfg(target_os = "windows")]
                {
                    // Windows: 需要至少失去焦点一次（count >= 1）才隐藏
                    // 避免程序启动时窗口闪现后立即消失
                    count >= 1 && can_hide
                }
            };

            // 统一的窗口隐藏逻辑
            if should_hide {
                log::debug!("触发窗口隐藏");
                if let Err(e) = main1.hide() {
                    log::error!("隐藏窗口失败: {}", e);
                }
            }
        }
        WindowEvent::Focused(true) => {
            log::debug!("窗口获得焦点事件触发");
        }
        _ => {}
    });
    Ok(())
}

/// 计算最佳窗口宽度
///
/// 算法逻辑：
/// 1. 基础宽度：屏幕宽度的合理比例
/// 2. DPI适配：根据缩放比例调整
/// 3. 范围限制：设置最小和最大值
fn calculate_optimal_width(screen_width: i32, scale_factor: f64) -> i32 {
    // 基础比例计算 - 根据屏幕宽度采用不同策略
    let base_width = if screen_width <= 1366 {
        // 小屏幕：占用较小比例，避免过于拥挤
        (screen_width as f64 * 0.28).round() as i32
    } else if screen_width <= 1920 {
        // 标准屏幕：1/5 到 1/6 之间
        (screen_width as f64 * 0.18).round() as i32
    } else if screen_width <= 2560 {
        // 大屏幕：稍小比例
        (screen_width as f64 * 0.15).round() as i32
    } else {
        // 超大屏幕：更小比例，避免过宽
        (screen_width as f64 * 0.12).round() as i32
    };

    // DPI 适配调整
    let dpi_adjusted_width = (base_width as f64 * scale_factor.min(2.0)) as i32;

    // 设置合理的范围限制
    let min_width = (380.0 * scale_factor) as i32; // 最小380px（逻辑像素）
    let max_width = (600.0 * scale_factor) as i32; // 最大600px（逻辑像素）

    dpi_adjusted_width.clamp(min_width, max_width)
}

/// 获取准确的工作区域信息
///
/// 返回: (顶部偏移量, X位置)
fn get_accurate_work_area(screen_width: i32, window_width: i32, scale_factor: f64) -> (i32, i32) {
    #[cfg(target_os = "macos")]
    {
        get_macos_work_area(screen_width, window_width, scale_factor)
    }

    #[cfg(target_os = "windows")]
    {
        get_windows_work_area(screen_width, window_width, scale_factor)
    }
}

/// macOS准确获取工作区域
#[cfg(target_os = "macos")]
fn get_macos_work_area(screen_width: i32, window_width: i32, scale_factor: f64) -> (i32, i32) {
    // 在macOS上，使用系统API获取准确的菜单栏高度
    let menubar_height = unsafe {
        use cocoa::appkit::NSScreen;
        use cocoa::base::nil;
        use cocoa::foundation::NSRect;

        let main_screen = NSScreen::mainScreen(nil);
        if main_screen != nil {
            // 获取屏幕frame和visibleFrame的差值来计算菜单栏高度
            let screen_frame: NSRect = msg_send![main_screen, frame];
            let visible_frame: NSRect = msg_send![main_screen, visibleFrame];

            // 菜单栏高度 = 屏幕总高度 - 可见区域顶部位置 - 可见区域高度
            let calculated_height = (screen_frame.size.height
                - visible_frame.origin.y
                - visible_frame.size.height) as i32;

            log::info!("macOS菜单栏高度准确计算: {}px", calculated_height);
            calculated_height
        } else {
            // 如果API调用失败，回退到固定值
            log::warn!("无法获取macOS屏幕信息，使用默认菜单栏高度");
            if scale_factor >= 2.0 {
                28
            } else {
                24
            }
        }
    };

    // macOS: 窗口右侧紧贴屏幕边缘，不需要边距
    let x_position = (screen_width - window_width).max(0);

    log::info!("macOS 窗口X位置: {}, 窗口宽度: {}, 屏幕宽度: {}", x_position, window_width, screen_width);

    (menubar_height, x_position)
}

/// Windows准确获取工作区域  
#[cfg(target_os = "windows")]
fn get_windows_work_area(screen_width: i32, window_width: i32, scale_factor: f64) -> (i32, i32) {
    let work_area_top = unsafe {
        let mut work_area = windows::Win32::Foundation::RECT::default();
        let success = windows::Win32::UI::WindowsAndMessaging::SystemParametersInfoW(
            windows::Win32::UI::WindowsAndMessaging::SPI_GETWORKAREA,
            0,
            Some(&mut work_area as *mut windows::Win32::Foundation::RECT as *mut std::ffi::c_void),
            windows::Win32::UI::WindowsAndMessaging::SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS(0),
        );

        if success.is_ok() {
            log::info!(
                "Windows工作区域: top={}, left={}, bottom={}, right={}",
                work_area.top,
                work_area.left,
                work_area.bottom,
                work_area.right
            );
            work_area.top
        } else {
            log::warn!("无法获取Windows工作区域，假设无顶部偏移");
            0
        }
    };

    let edge_margin = (8.0 * scale_factor) as i32;
    let x_position = (screen_width - window_width - edge_margin).max(0);

    (work_area_top, x_position)
}

#[derive(Default, Debug)]
pub struct WindowFocusCount {
    pub lost_count: AtomicU64,
}

impl WindowFocusCount {
    pub fn inc(&self) -> u64 {
        self.lost_count.fetch_add(1, Ordering::SeqCst)
    }

    /// 重置焦点丢失计数器
    pub fn reset(&self) {
        self.lost_count.store(0, Ordering::SeqCst);
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

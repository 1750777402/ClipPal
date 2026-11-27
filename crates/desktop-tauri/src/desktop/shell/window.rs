// 抑制 cocoa crate 的弃用警告
#![allow(deprecated)]

use std::sync::{
    atomic::{AtomicUsize, AtomicBool, AtomicU64, Ordering},
    Arc,
};

use tauri::{App, Runtime, WebviewWindow, WindowEvent};
use tauri::{Manager, PhysicalPosition, PhysicalSize};

// macOS系统API导入
#[cfg(target_os = "macos")]
use objc::{msg_send, sel, sel_impl};

pub fn init_main_window(
    app: &App,
    window_focus_state: Arc<WindowFocusState>,
    window_hide_flag: Arc<WindowHideFlag>,
) -> tauri::Result<()> {
    // 获取主显示器
    let main_window = app.get_webview_window("main").ok_or_else(|| {
        log::error!("无法获取主窗口");
        tauri::Error::FailedToReceiveMessage
    })?;

    // 剪贴板弹窗由托盘和快捷键唤起，不参加 macOS 登录时的窗口恢复。
    #[cfg(target_os = "macos")]
    unsafe {
        let native_window = main_window.ns_window()? as *mut objc::runtime::Object;
        let _: () = msg_send![native_window, setRestorable: cocoa::base::NO];
    }

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

    // 先注册焦点监听，启动显示统一由 setup 在上下文准备完毕后执行。
    track_main_window_focus(&main_window, window_focus_state, window_hide_flag);
    Ok(())
}

fn track_main_window_focus<R: Runtime>(
    main: &WebviewWindow<R>,
    focus_state: Arc<WindowFocusState>,
    hide_flag: Arc<WindowHideFlag>,
) {
    let main_window = main.clone();
    main.on_window_event(move |event| match event {
        WindowEvent::Focused(false) => {
            log::debug!("窗口失去焦点事件触发");

            // 只有已获得焦点后的失焦才隐藏，避免启动时误隐藏；各平台策略一致。
            if focus_state.lost_focus() {
                schedule_focus_check(&main_window, &focus_state, &hide_flag);
            }
        }
        WindowEvent::Focused(true) => {
            focus_state.gained_focus();
            log::debug!("窗口获得焦点事件触发");
        }
        WindowEvent::CloseRequested { api, .. } => {
            // 关闭主窗口只收起到托盘，退出仍通过托盘菜单执行。
            api.prevent_close();
            if hide_flag.is_can_hide() {
                if let Err(error) = main_window.hide() {
                    log::error!("收起主窗口失败: {}", error);
                }
            }
        }
        _ => {}
    });
}

pub(super) fn track_image_viewer_focus<R: Runtime>(
    viewer: &WebviewWindow<R>,
    main: &WebviewWindow<R>,
    focus_state: Arc<WindowFocusState>,
    hide_flag: Arc<WindowHideFlag>,
) {
    let main_window = main.clone();
    viewer.on_window_event(move |event| match event {
        WindowEvent::Focused(true) => focus_state.cancel_pending_check(),
        WindowEvent::Focused(false) | WindowEvent::Destroyed => {
            schedule_focus_check(&main_window, &focus_state, &hide_flag);
        }
        _ => {}
    });
}

fn schedule_focus_check<R: Runtime>(
    main: &WebviewWindow<R>,
    focus_state: &Arc<WindowFocusState>,
    hide_flag: &Arc<WindowHideFlag>,
) {
    let revision = focus_state.next_check();
    if !hide_flag.is_can_hide() {
        return;
    }
    let state = focus_state.clone();
    let flag = hide_flag.clone();
    let window = main.clone();
    tauri::async_runtime::spawn(async move {
        // 短暂异步延后，让托盘点击先确定显隐意图，也给两个窗口之间的焦点切换留出时间。
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        let app = window.app_handle().clone();
        if let Err(error) = app.run_on_main_thread(move || {
            if !state.is_current_check(revision)
                || !flag.is_can_hide()
                || !window.is_visible().unwrap_or(false)
                || window.is_focused().unwrap_or(true)
            {
                return;
            }
            // 在 UI 线程读取实时焦点，避免依赖各平台发送焦点事件的先后顺序。
            if window
                .app_handle()
                .get_webview_window(super::image_viewer::LABEL)
                .is_some_and(|viewer| viewer.is_focused().unwrap_or(true))
            {
                return;
            }
            if let Err(error) = window.hide() {
                log::error!("隐藏窗口失败: {error}");
            }
        }) {
            log::error!("调度窗口失焦处理失败: {error}");
        }
    });
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

    log::info!(
        "macOS 窗口X位置: {}, 窗口宽度: {}, 屏幕宽度: {}",
        x_position,
        window_width,
        screen_width
    );

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

/// 记录主窗口的焦点变化和两个窗口共用的失焦检查序号。
#[derive(Default, Debug)]
pub struct WindowFocusState {
    focused: AtomicBool,
    revision: AtomicU64,
}

impl WindowFocusState {
    pub fn gained_focus(&self) {
        self.focused.store(true, Ordering::SeqCst);
        self.cancel_pending_check();
    }
    pub fn lost_focus(&self) -> bool {
        // 初始或重复的失焦事件不应触发或取消已安排的检查。
        self.focused.swap(false, Ordering::SeqCst)
    }

    fn next_check(&self) -> u64 {
        self.revision.fetch_add(1, Ordering::SeqCst) + 1
    }

    fn cancel_pending_check(&self) {
        self.next_check();
    }

    fn is_current_check(&self, revision: u64) -> bool {
        self.revision.load(Ordering::SeqCst) == revision
    }
}

/// 所有用户唤起入口共用：先保存外部目标，再恢复最小化、显示并聚焦。
pub fn show_main_window(context: &crate::app_context::AppContext) -> crate::errors::AppResult<()> {
    context.auto_paste_state().wake_generation.fetch_add(1, Ordering::SeqCst);
    if let Err(error) = crate::core::application::ClipboardService::from_context(context).capture_auto_paste_target() {
        log::warn!("保存自动粘贴目标窗口失败: {}", error);
    }
    let window = context.main_window()?;
    let _guard = WindowHideGuard::new(context.window_hide_flag());
    let error = |error: tauri::Error| crate::errors::AppError::Window(error.to_string());
    if window.is_minimized().map_err(error)? { window.unminimize().map_err(error)?; }
    window.show().map_err(error)?;
    window.set_focus().map_err(error)?;
    Ok(())
}

/// 托盘等主动隐藏入口也遵守对话框保护，避免丢失正在进行的用户交互。
pub fn hide_main_window(context: &crate::app_context::AppContext) -> crate::errors::AppResult<()> {
    if context.window_hide_flag().is_can_hide() { context.main_window()?.hide().map_err(|error| crate::errors::AppError::Window(error.to_string()))?; }
    Ok(())
}

#[derive(Debug)]
pub struct WindowHideFlag {
    /// 主窗口隐藏保护的持有数量，支持多个保存对话框交错结束。
    guards: AtomicUsize,
}

impl Default for WindowHideFlag {
    fn default() -> Self {
        Self {
            // 默认窗口可以隐藏
            guards: AtomicUsize::new(0),
        }
    }
}

impl WindowHideFlag {
    /// 窗口是否可以隐藏
    pub fn is_can_hide(&self) -> bool {
        self.guards.load(Ordering::SeqCst) == 0
    }
}

/// 窗口隐藏保护  作用域守卫
pub struct WindowHideGuard {
    flag: Arc<WindowHideFlag>,
}

impl WindowHideGuard {
    pub fn new(flag: Arc<WindowHideFlag>) -> Self {
        flag.guards.fetch_add(1, Ordering::SeqCst);
        Self { flag }
    }
}

/// 实现Drop  最后一个保护作用域失效时，自动恢复主窗口可以隐藏。
impl Drop for WindowHideGuard {
    fn drop(&mut self) {
        self.flag.guards.fetch_sub(1, Ordering::SeqCst);
    }
}

#[cfg(test)]
mod tests {
    use super::{WindowHideFlag, WindowHideGuard, WindowFocusState};
    use std::sync::Arc;

    #[test]
    fn initial_blur_is_ignored_but_every_real_blur_is_observed() {
        let state = WindowFocusState::default();
        assert!(!state.lost_focus());
        for _ in 0..3 {
            state.gained_focus();
            assert!(state.lost_focus());
            assert!(!state.lost_focus());
        }
    }

    #[test]
    fn delayed_blur_cannot_hide_a_refocused_window() {
        let state = WindowFocusState::default();
        state.gained_focus();
        state.lost_focus();
        let revision = state.next_check();
        assert!(state.is_current_check(revision));
        state.gained_focus();
        assert!(!state.is_current_check(revision));
        state.lost_focus();
        assert!(!state.is_current_check(revision));
    }

    #[test]
    fn viewer_focus_cancels_main_blur_and_viewer_blur_allows_a_new_check() {
        let state = WindowFocusState::default();
        state.gained_focus();
        assert!(state.lost_focus());
        let main_blur = state.next_check();
        assert!(!state.lost_focus());
        assert!(state.is_current_check(main_blur));
        state.cancel_pending_check();
        assert!(!state.is_current_check(main_blur));
        let viewer_blur = state.next_check();
        assert!(state.is_current_check(viewer_blur));
        state.gained_focus();
        assert!(!state.is_current_check(viewer_blur));
    }

    #[test]
    fn overlapping_dialogs_keep_window_visible_until_last_guard_finishes() {
        let flag = Arc::new(WindowHideFlag::default());
        assert!(flag.is_can_hide());
        let first = WindowHideGuard::new(flag.clone());
        let second = WindowHideGuard::new(flag.clone());
        assert!(!flag.is_can_hide());
        drop(first);
        assert!(!flag.is_can_hide());
        drop(second);
        assert!(flag.is_can_hide());
    }
}

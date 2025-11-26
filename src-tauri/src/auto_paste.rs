use crate::errors::{AppError, AppResult};
#[cfg(any(windows, target_os = "macos"))]
use once_cell::sync::Lazy;
#[cfg(any(windows, target_os = "macos"))]
use std::sync::{Arc, Mutex};

#[cfg(windows)]
use windows::Win32::{
    Foundation::HWND,
    System::Threading::GetCurrentProcessId,
    UI::{
        Input::KeyboardAndMouse::{
            INPUT, INPUT_KEYBOARD, KEYBDINPUT, KEYEVENTF_KEYUP, SendInput, VK_CONTROL, VK_V,
        },
        WindowsAndMessaging::{
            GetForegroundWindow, GetWindowTextW, GetWindowThreadProcessId, IsWindow,
            IsWindowVisible, SetForegroundWindow,
        },
    },
};

#[cfg(windows)]
static PREVIOUS_WINDOW: Lazy<Arc<Mutex<Option<WindowInfo>>>> =
    Lazy::new(|| Arc::new(Mutex::new(None)));

#[cfg(windows)]
#[derive(Debug, Clone)]
struct WindowInfo {
    hwnd: isize, // HWND as isize for thread safety
    title: String,
    process_id: u32,
}

/// 保存当前获得焦点的窗口信息
#[cfg(windows)]
pub fn save_foreground_window() {
    let hwnd = unsafe { GetForegroundWindow() };

    if hwnd.0.is_null() {
        return;
    }

    // 检查是否是我们自己的窗口
    let mut process_id = 0u32;
    unsafe {
        GetWindowThreadProcessId(hwnd, Some(&mut process_id));
    }

    let current_process_id = unsafe { GetCurrentProcessId() };

    // 如果是我们自己的窗口，不保存
    if process_id == current_process_id {
        return;
    }

    // 获取窗口标题
    let mut title_buffer = [0u16; 256];
    let title_len = unsafe { GetWindowTextW(hwnd, &mut title_buffer) };

    let title = if title_len > 0 {
        String::from_utf16_lossy(&title_buffer[..title_len as usize])
    } else {
        "Unknown".to_string()
    };

    let window_info = WindowInfo {
        hwnd: hwnd.0 as isize,
        title,
        process_id,
    };

    if let Ok(mut previous) = PREVIOUS_WINDOW.lock() {
        *previous = Some(window_info.clone());
        log::debug!(
            "保存焦点窗口: {} (PID: {})",
            window_info.title,
            window_info.process_id
        );
    }
}

/// 执行自动粘贴到之前的窗口 - Windows版本
#[cfg(windows)]
pub fn auto_paste_to_previous_window() -> AppResult<()> {
    let window_info = {
        let previous = PREVIOUS_WINDOW
            .lock()
            .map_err(|e| AppError::Lock(format!("获取窗口信息锁失败: {}", e)))?;

        match previous.as_ref() {
            Some(info) => info.clone(),
            None => {
                log::warn!("没有保存的目标窗口信息");
                return Err(AppError::AutoPaste("没有找到目标窗口".to_string()));
            }
        }
    };

    let hwnd = HWND(window_info.hwnd as *mut std::ffi::c_void);

    // 检查窗口是否仍然有效
    let is_valid = unsafe { IsWindow(hwnd) };
    if !is_valid.as_bool() {
        log::warn!("目标窗口已经无效");
        return Err(AppError::AutoPaste("目标窗口已经无效".to_string()));
    }

    // 检查窗口是否可见
    let is_visible = unsafe { IsWindowVisible(hwnd) };
    if !is_visible.as_bool() {
        log::warn!("目标窗口不可见");
        return Err(AppError::AutoPaste("目标窗口不可见".to_string()));
    }

    log::debug!("尝试自动粘贴到窗口: {}", window_info.title);

    // 将目标窗口设置为前台窗口
    let result = unsafe { SetForegroundWindow(hwnd) };
    if !result.as_bool() {
        log::warn!("无法将目标窗口设为前台窗口");
        // 继续尝试发送按键，有些情况下即使设置前台失败，按键仍然可以工作
    }

    // 等待一小段时间让窗口切换完成
    std::thread::sleep(std::time::Duration::from_millis(50));

    // 发送 Ctrl+V 按键组合
    send_ctrl_v_windows()?;

    log::debug!("自动粘贴完成");
    Ok(())
}

/// 发送 Ctrl+V 按键组合 - Windows版本
#[cfg(windows)]
fn send_ctrl_v_windows() -> AppResult<()> {
    let mut inputs = vec![
        // 按下 Ctrl
        INPUT {
            r#type: INPUT_KEYBOARD,
            Anonymous: windows::Win32::UI::Input::KeyboardAndMouse::INPUT_0 {
                ki: KEYBDINPUT {
                    wVk: VK_CONTROL,
                    wScan: 0,
                    dwFlags: Default::default(),
                    time: 0,
                    dwExtraInfo: 0,
                },
            },
        },
        // 按下 V
        INPUT {
            r#type: INPUT_KEYBOARD,
            Anonymous: windows::Win32::UI::Input::KeyboardAndMouse::INPUT_0 {
                ki: KEYBDINPUT {
                    wVk: VK_V,
                    wScan: 0,
                    dwFlags: Default::default(),
                    time: 0,
                    dwExtraInfo: 0,
                },
            },
        },
        // 释放 V
        INPUT {
            r#type: INPUT_KEYBOARD,
            Anonymous: windows::Win32::UI::Input::KeyboardAndMouse::INPUT_0 {
                ki: KEYBDINPUT {
                    wVk: VK_V,
                    wScan: 0,
                    dwFlags: KEYEVENTF_KEYUP,
                    time: 0,
                    dwExtraInfo: 0,
                },
            },
        },
        // 释放 Ctrl
        INPUT {
            r#type: INPUT_KEYBOARD,
            Anonymous: windows::Win32::UI::Input::KeyboardAndMouse::INPUT_0 {
                ki: KEYBDINPUT {
                    wVk: VK_CONTROL,
                    wScan: 0,
                    dwFlags: KEYEVENTF_KEYUP,
                    time: 0,
                    dwExtraInfo: 0,
                },
            },
        },
    ];

    let result = unsafe { SendInput(&mut inputs, std::mem::size_of::<INPUT>() as i32) };

    if result != inputs.len() as u32 {
        return Err(AppError::AutoPaste(format!(
            "发送按键失败，期望发送 {} 个事件，实际发送 {} 个",
            inputs.len(),
            result
        )));
    }

    log::debug!("成功发送 Ctrl+V 组合键");
    Ok(())
}

#[cfg(target_os = "macos")]
use core_graphics::{
    event::{CGEvent, CGEventFlags, CGKeyCode},
    event_source::{CGEventSource, CGEventSourceStateID},
};

/// 保存前台窗口信息 - macOS 简化版（不保存）
#[cfg(target_os = "macos")]
pub fn save_foreground_window() {
    // macOS 策略：不保存窗口，依赖系统自动切换
    // 当 ClipPal 窗口隐藏时，macOS 会自动激活之前活动的应用
    log::debug!("macOS 不需要手动保存窗口信息");
}

/// 执行自动粘贴 - macOS 简化版
#[cfg(target_os = "macos")]
pub fn auto_paste_to_previous_window() -> AppResult<()> {
    use crate::CONTEXT;
    use tauri::{AppHandle, Manager};

    log::debug!("开始 macOS 自动粘贴");

    // 获取窗口句柄
    let app_handle = CONTEXT.get::<AppHandle>();
    let window = app_handle
        .get_webview_window("main")
        .ok_or_else(|| AppError::AutoPaste("无法获取主窗口".to_string()))?;

    // 确保窗口已隐藏，让系统自动切换到之前的应用
    if window.is_visible().unwrap_or(true) {
        log::debug!("隐藏 ClipPal 窗口");
        window
            .hide()
            .map_err(|e| AppError::AutoPaste(format!("隐藏窗口失败: {}", e)))?;

        // 等待窗口隐藏和系统切换到上一个应用
        std::thread::sleep(std::time::Duration::from_millis(350));
    } else {
        log::debug!("窗口已隐藏，稍等系统切换");
        // 窗口已隐藏，稍等片刻确保系统完成切换
        std::thread::sleep(std::time::Duration::from_millis(150));
    }

    // 发送 Cmd+V 到当前前台应用
    send_cmd_v()?;

    log::debug!("macOS 自动粘贴完成");
    Ok(())
}

/// 模拟 Cmd+V
#[cfg(target_os = "macos")]
fn send_cmd_v() -> AppResult<()> {
    unsafe {
        let source = CGEventSource::new(CGEventSourceStateID::HIDSystemState)
            .map_err(|e| AppError::AutoPaste(format!("创建事件源失败: {:?}", e)))?;

        let v_key: CGKeyCode = 9; // V 键的键码

        // 按下 Cmd+V
        let key_down = CGEvent::new_keyboard_event(source.clone(), v_key, true)
            .map_err(|e| AppError::AutoPaste(format!("创建按键按下事件失败: {:?}", e)))?;
        key_down.set_flags(CGEventFlags::CGEventFlagCommand);
        key_down.post(core_graphics::event::CGEventTapLocation::HID);

        log::debug!("已发送 Cmd+V 按下事件");

        // 短暂延迟，模拟真实按键
        std::thread::sleep(std::time::Duration::from_millis(20));

        // 释放 Cmd+V
        let key_up = CGEvent::new_keyboard_event(source, v_key, false)
            .map_err(|e| AppError::AutoPaste(format!("创建按键释放事件失败: {:?}", e)))?;
        key_up.set_flags(CGEventFlags::CGEventFlagCommand);
        key_up.post(core_graphics::event::CGEventTapLocation::HID);

        log::debug!("已发送 Cmd+V 释放事件");
    }

    Ok(())
}

/// 不支持平台的占位实现
#[cfg(not(any(windows, target_os = "macos")))]
pub fn save_foreground_window() {
    log::warn!("自动粘贴功能仅支持 Windows 和 macOS 平台");
}

/// 不支持平台的占位实现
#[cfg(not(any(windows, target_os = "macos")))]
pub fn auto_paste_to_previous_window() -> AppResult<()> {
    Err(AppError::AutoPaste(
        "自动粘贴功能仅在Windows和macOS平台支持".to_string(),
    ))
}

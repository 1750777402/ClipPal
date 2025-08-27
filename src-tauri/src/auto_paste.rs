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

#[cfg(target_os = "macos")]
use cocoa::{
    appkit::{NSApp, NSApplication, NSWindow, NSWorkspace},
    base::{NO, YES, id, nil},
    foundation::{NSArray, NSNumber, NSString},
};
#[cfg(target_os = "macos")]
use core_graphics::{
    event::{CGEvent, CGEventFlags, CGEventType, CGKeyCode},
    event_source::{CGEventSource, CGEventSourceStateID},
    window::{CGWindowListCopyWindowInfo, CGWindowListOption, kCGWindowNumber, kCGWindowOwnerPID},
};
#[cfg(target_os = "macos")]
use objc::{msg_send, runtime::Object, sel, sel_impl};

#[cfg(any(windows, target_os = "macos"))]
static PREVIOUS_WINDOW: Lazy<Arc<Mutex<Option<WindowInfo>>>> =
    Lazy::new(|| Arc::new(Mutex::new(None)));

#[cfg(any(windows, target_os = "macos"))]
#[derive(Debug, Clone)]
struct WindowInfo {
    #[cfg(windows)]
    hwnd: isize, // HWND as isize for thread safety
    #[cfg(target_os = "macos")]
    window_number: u32, // CGWindowID for macOS
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

/// Mac系统保存当前获得焦点的窗口信息
#[cfg(target_os = "macos")]
pub fn save_foreground_window() {
    unsafe {
        // 获取当前前台应用
        let workspace = NSWorkspace::sharedWorkspace(nil);
        let running_apps: id = msg_send![workspace, runningApplications];
        let app_count: usize = msg_send![running_apps, count];

        let mut frontmost_app_pid = 0u32;

        // 找到前台应用
        for i in 0..app_count {
            let app: id = msg_send![running_apps, objectAtIndex: i];
            let is_active: bool = msg_send![app, isActive];

            if is_active {
                let pid_number: id = msg_send![app, processIdentifier];
                frontmost_app_pid = msg_send![pid_number, intValue];
                break;
            }
        }

        // 获取当前进程PID
        let current_pid = std::process::id();

        // 如果是我们自己的窗口，不保存
        if frontmost_app_pid == current_pid {
            return;
        }

        // 获取窗口列表信息
        let window_list_info = CGWindowListCopyWindowInfo(
            CGWindowListOption::kCGWindowListOptionOnScreenOnly
                | CGWindowListOption::kCGWindowListExcludeDesktopElements,
            0,
        );

        if window_list_info.is_null() {
            return;
        }

        let window_count: usize = msg_send![window_list_info, count];

        for i in 0..window_count {
            let window_info: id = msg_send![window_list_info, objectAtIndex: i];

            // 获取窗口的进程ID
            let pid_key = NSString::alloc(nil).init_str("kCGWindowOwnerPID");
            let pid_obj: id = msg_send![window_info, objectForKey: pid_key];

            if pid_obj != nil {
                let window_pid: u32 = msg_send![pid_obj, intValue];

                if window_pid == frontmost_app_pid {
                    // 获取窗口标题
                    let title_key = NSString::alloc(nil).init_str("kCGWindowName");
                    let title_obj: id = msg_send![window_info, objectForKey: title_key];

                    let title = if title_obj != nil {
                        let title_ptr: *const i8 = msg_send![title_obj, UTF8String];
                        if !title_ptr.is_null() {
                            std::ffi::CStr::from_ptr(title_ptr)
                                .to_string_lossy()
                                .to_string()
                        } else {
                            "Unknown".to_string()
                        }
                    } else {
                        "Unknown".to_string()
                    };

                    // 获取窗口编号
                    let window_number_key = NSString::alloc(nil).init_str("kCGWindowNumber");
                    let window_number_obj: id =
                        msg_send![window_info, objectForKey: window_number_key];
                    let window_number: u32 = if window_number_obj != nil {
                        msg_send![window_number_obj, intValue]
                    } else {
                        0
                    };

                    let window_info = WindowInfo {
                        window_number,
                        title,
                        process_id: window_pid,
                    };

                    if let Ok(mut previous) = PREVIOUS_WINDOW.lock() {
                        *previous = Some(window_info.clone());
                        log::debug!(
                            "保存焦点窗口: {} (PID: {})",
                            window_info.title,
                            window_info.process_id
                        );
                    }
                    break;
                }
            }
        }
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

/// 执行自动粘贴到之前的窗口 - macOS版本
#[cfg(target_os = "macos")]
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

    log::debug!(
        "尝试自动粘贴到窗口: {} (PID: {})",
        window_info.title,
        window_info.process_id
    );

    unsafe {
        // 激活目标应用
        let workspace = NSWorkspace::sharedWorkspace(nil);
        let running_apps: id = msg_send![workspace, runningApplications];
        let app_count: usize = msg_send![running_apps, count];

        let mut target_app: id = nil;

        // 找到目标应用
        for i in 0..app_count {
            let app: id = msg_send![running_apps, objectAtIndex: i];
            let pid_number: id = msg_send![app, processIdentifier];
            let app_pid: u32 = msg_send![pid_number, intValue];

            if app_pid == window_info.process_id {
                target_app = app;
                break;
            }
        }

        if target_app == nil {
            log::warn!("无法找到目标应用");
            return Err("无法找到目标应用".to_string());
        }

        // 激活目标应用
        let activated: bool = msg_send![target_app, activateWithOptions: 0];
        if !activated {
            log::warn!("无法激活目标应用");
            // 继续尝试发送按键
        }
    }

    // 等待一小段时间让应用切换完成
    std::thread::sleep(std::time::Duration::from_millis(100));

    // 发送 Cmd+V 按键组合
    send_cmd_v_macos()?;

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

    log::debug!("成功发送 Ctrl+V 按键事件");
    Ok(())
}

/// 发送 Cmd+V 按键组合 - macOS版本  
#[cfg(target_os = "macos")]
fn send_cmd_v_macos() -> AppResult<()> {
    unsafe {
        // 创建事件源
        let event_source = CGEventSource::new(CGEventSourceStateID::CombinedSessionState)
            .map_err(|e| AppError::AutoPaste(format!("创建事件源失败: {:?}", e)))?;

        // V键的虚拟键码 (基于US键盘布局)
        let v_keycode: CGKeyCode = 9;

        // 按下 Cmd+V
        let key_down_event = CGEvent::new_keyboard_event(event_source.clone(), v_keycode, true)
            .map_err(|e| AppError::AutoPaste(format!("创建按键按下事件失败: {:?}", e)))?;

        key_down_event.set_flags(CGEventFlags::CGEventFlagCommand);
        key_down_event.post(core_graphics::event::CGEventTapLocation::HID);

        // 短暂延迟
        std::thread::sleep(std::time::Duration::from_millis(10));

        // 释放 Cmd+V
        let key_up_event = CGEvent::new_keyboard_event(event_source, v_keycode, false)
            .map_err(|e| AppError::AutoPaste(format!("创建按键释放事件失败: {:?}", e)))?;

        key_up_event.set_flags(CGEventFlags::CGEventFlagCommand);
        key_up_event.post(core_graphics::event::CGEventTapLocation::HID);
    }

    log::debug!("成功发送 Cmd+V 按键事件");
    Ok(())
}

/// 不支持平台的占位实现
#[cfg(not(any(windows, target_os = "macos")))]
pub fn save_foreground_window() {
    log::warn!("自动粘贴功能仅在Windows和macOS平台支持");
}

/// 不支持平台的占位实现
#[cfg(not(any(windows, target_os = "macos")))]
pub fn auto_paste_to_previous_window() -> AppResult<()> {
    Err(AppError::AutoPaste(
        "自动粘贴功能仅在Windows和macOS平台支持".to_string(),
    ))
}

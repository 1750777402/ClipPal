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
use cocoa::{
    appkit::{NSApp, NSApplication},
    base::{id, nil},
    foundation::NSString,
};
#[cfg(target_os = "macos")]
use core_graphics::{
    event::{CGEvent, CGEventFlags, CGKeyCode},
    event_source::{CGEventSource, CGEventSourceStateID},
};
#[cfg(target_os = "macos")]
use objc::{class, msg_send, sel, sel_impl}; // 添加 class 宏导入

/// 保存前台窗口信息
#[cfg(target_os = "macos")]
pub fn save_foreground_window() {
    unsafe {
        let workspace: id = msg_send![class!(NSWorkspace), sharedWorkspace];
        let front_app: id = msg_send![workspace, frontmostApplication];
        if front_app == nil {
            return;
        }

        let pid: u32 = msg_send![front_app, processIdentifier];
        if pid == std::process::id() {
            return;
        }

        let name_obj: id = msg_send![front_app, localizedName];
        let cname: *const i8 = msg_send![name_obj, UTF8String];
        let app_name = if !cname.is_null() {
            std::ffi::CStr::from_ptr(cname)
                .to_string_lossy()
                .into_owned()
        } else {
            "Unknown".into()
        };

        let info = WindowInfo {
            window_number: 0,
            title: app_name.clone(),
            process_id: pid,
        };

        if let Ok(mut prev) = PREVIOUS_WINDOW.lock() {
            *prev = Some(info.clone());
        }
        log::debug!("保存前台窗口: {} (PID: {})", app_name, pid);
    }
}

/// 执行自动粘贴
#[cfg(target_os = "macos")]
pub fn auto_paste_to_previous_window() -> AppResult<()> {
    let window_info = {
        let prev = PREVIOUS_WINDOW
            .lock()
            .map_err(|e| AppError::Lock(format!("锁失败: {}", e)))?;
        match prev.as_ref() {
            Some(info) => info.clone(),
            None => return Err(AppError::AutoPaste("没有找到目标窗口".into())),
        }
    };

    log::debug!(
        "自动粘贴到: {} (PID: {})",
        window_info.title,
        window_info.process_id
    );

    unsafe {
        let workspace: id = msg_send![class!(NSWorkspace), sharedWorkspace];
        let running_apps: id = msg_send![workspace, runningApplications];
        let count: usize = msg_send![running_apps, count];
        let mut target_app: id = nil;

        for i in 0..count {
            let app: id = msg_send![running_apps, objectAtIndex: i];
            let pid: u32 = msg_send![app, processIdentifier];
            if pid == window_info.process_id {
                target_app = app;
                break;
            }
        }

        if target_app == nil {
            return Err(AppError::AutoPaste("目标应用未找到".into()));
        }

        // 强制激活 App
        let _: () = msg_send![target_app, activateWithOptions: 1 << 1];
    }

    std::thread::sleep(std::time::Duration::from_millis(120));

    // 尝试菜单 Paste 优先
    if !try_menu_paste() {
        send_cmd_v()?;
    }

    log::debug!("自动粘贴完成");
    Ok(())
}

/// 尝试菜单 Paste
#[cfg(target_os = "macos")]
fn try_menu_paste() -> bool {
    unsafe {
        let app = NSApp();
        if app == nil {
            return false;
        }
        let main_menu: id = msg_send![app, mainMenu];
        if main_menu == nil {
            return false;
        }

        let item_count: usize = msg_send![main_menu, numberOfItems];
        for i in 0..item_count {
            let item: id = msg_send![main_menu, itemAtIndex: i];
            if item == nil {
                continue;
            }
            let submenu: id = msg_send![item, submenu];
            if submenu == nil {
                continue;
            }
            let sub_count: usize = msg_send![submenu, numberOfItems];
            for j in 0..sub_count {
                let sub_item: id = msg_send![submenu, itemAtIndex: j];
                if sub_item == nil {
                    continue;
                }
                let title: id = msg_send![sub_item, title];
                let title_str = nsstring_to_rust(title).to_lowercase();
                if title_str == "paste" || title_str == "粘贴" || title_str == "вставить"
                {
                    let _: () = msg_send![sub_item, performClick: nil];
                    return true;
                }
            }
        }
    }
    false
}

#[cfg(target_os = "macos")]
unsafe fn nsstring_to_rust(ns: id) -> String {
    if ns == nil {
        return "".into();
    }
    let c: *const i8 = msg_send![ns, UTF8String];
    if c.is_null() {
        return "".into();
    }
    std::ffi::CStr::from_ptr(c).to_string_lossy().into_owned()
}

/// 模拟 Cmd+V
#[cfg(target_os = "macos")]
fn send_cmd_v() -> AppResult<()> {
    unsafe {
        let source = CGEventSource::new(CGEventSourceStateID::HIDSystemState)
            .map_err(|e| AppError::AutoPaste(format!("事件源失败: {:?}", e)))?;
        let v_key: CGKeyCode = 9;

        let down = CGEvent::new_keyboard_event(source.clone(), v_key, true)
            .map_err(|e| AppError::AutoPaste(format!("按下失败: {:?}", e)))?;
        down.set_flags(CGEventFlags::CGEventFlagCommand);
        down.post(core_graphics::event::CGEventTapLocation::HID);

        std::thread::sleep(std::time::Duration::from_millis(10));

        let up = CGEvent::new_keyboard_event(source, v_key, false)
            .map_err(|e| AppError::AutoPaste(format!("释放失败: {:?}", e)))?;
        up.set_flags(CGEventFlags::CGEventFlagCommand);
        up.post(core_graphics::event::CGEventTapLocation::HID);
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

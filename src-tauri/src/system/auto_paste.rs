use crate::{
    app_context::AutoPasteTarget,
    errors::{AppError, AppResult},
};

#[cfg(windows)]
use windows::Win32::{
    Foundation::HWND,
    System::Threading::GetCurrentProcessId,
    UI::{
        Input::KeyboardAndMouse::{
            SendInput, INPUT, INPUT_KEYBOARD, KEYBDINPUT, KEYEVENTF_KEYUP, VK_CONTROL, VK_V,
        },
        WindowsAndMessaging::{
            GetForegroundWindow, GetWindowTextW, GetWindowThreadProcessId, IsWindow,
            IsWindowVisible, SetForegroundWindow,
        },
    },
};

/// 捕获当前前台窗口；当前台窗口属于 ClipPal 自身时返回空目标。
#[cfg(windows)]
pub fn capture_target() -> AppResult<Option<AutoPasteTarget>> {
    let hwnd = unsafe { GetForegroundWindow() };
    if hwnd.0.is_null() {
        return Ok(None);
    }

    let mut process_id = 0u32;
    unsafe {
        GetWindowThreadProcessId(hwnd, Some(&mut process_id));
    }
    if process_id == unsafe { GetCurrentProcessId() } {
        return Ok(None);
    }

    let mut title_buffer = [0u16; 256];
    let title_len = unsafe { GetWindowTextW(hwnd, &mut title_buffer) };
    let title = if title_len > 0 {
        String::from_utf16_lossy(&title_buffer[..title_len as usize])
    } else {
        "Unknown".to_string()
    };

    log::debug!("捕获自动粘贴目标窗口: {} (PID: {})", title, process_id);
    Ok(Some(AutoPasteTarget::Windows {
        hwnd: hwnd.0 as isize,
        process_id,
        title,
    }))
}

/// 恢复 Windows 目标窗口，并确认实际前台窗口与捕获目标完全一致。
#[cfg(windows)]
pub fn restore_and_verify(target: &AutoPasteTarget) -> AppResult<()> {
    let AutoPasteTarget::Windows {
        hwnd,
        process_id,
        title,
    } = target;
    let target_hwnd = HWND(*hwnd as *mut std::ffi::c_void);

    if !unsafe { IsWindow(target_hwnd) }.as_bool() {
        return Err(AppError::AutoPaste(
            "目标窗口已经失效，内容已复制".to_string(),
        ));
    }
    if !unsafe { IsWindowVisible(target_hwnd) }.as_bool() {
        return Err(AppError::AutoPaste(
            "目标窗口不可见，内容已复制".to_string(),
        ));
    }

    let mut current_process_id = 0u32;
    unsafe {
        GetWindowThreadProcessId(target_hwnd, Some(&mut current_process_id));
    }
    if current_process_id != *process_id {
        return Err(AppError::AutoPaste(
            "目标窗口句柄已被其他进程复用，内容已复制".to_string(),
        ));
    }

    if unsafe { GetForegroundWindow() }.0 != target_hwnd.0 {
        let activated = unsafe { SetForegroundWindow(target_hwnd) };
        if !activated.as_bool() {
            log::warn!("Windows 拒绝激活自动粘贴目标窗口: {}", title);
        }
    }

    for _ in 0..10 {
        if unsafe { GetForegroundWindow() }.0 == target_hwnd.0 {
            log::debug!("自动粘贴目标窗口已激活: {}", title);
            return Ok(());
        }
        std::thread::sleep(std::time::Duration::from_millis(30));
    }

    Err(AppError::AutoPaste(
        "无法确认目标窗口已获得焦点，内容已复制".to_string(),
    ))
}

/// 向已经验证的 Windows 前台窗口发送 Ctrl+V。
#[cfg(windows)]
pub fn send_paste_shortcut() -> AppResult<()> {
    let inputs = vec![
        keyboard_input(VK_CONTROL, Default::default()),
        keyboard_input(VK_V, Default::default()),
        keyboard_input(VK_V, KEYEVENTF_KEYUP),
        keyboard_input(VK_CONTROL, KEYEVENTF_KEYUP),
    ];
    let sent = unsafe { SendInput(&inputs, std::mem::size_of::<INPUT>() as i32) };
    if sent != inputs.len() as u32 {
        return Err(AppError::AutoPaste(format!(
            "发送 Ctrl+V 失败，期望 {} 个事件，实际发送 {} 个；内容已复制",
            inputs.len(),
            sent
        )));
    }
    Ok(())
}

#[cfg(windows)]
fn keyboard_input(
    key: windows::Win32::UI::Input::KeyboardAndMouse::VIRTUAL_KEY,
    flags: windows::Win32::UI::Input::KeyboardAndMouse::KEYBD_EVENT_FLAGS,
) -> INPUT {
    INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: windows::Win32::UI::Input::KeyboardAndMouse::INPUT_0 {
            ki: KEYBDINPUT {
                wVk: key,
                wScan: 0,
                dwFlags: flags,
                time: 0,
                dwExtraInfo: 0,
            },
        },
    }
}

#[cfg(target_os = "macos")]
use cocoa::base::{id, nil};
#[cfg(target_os = "macos")]
use core_graphics::{
    event::{CGEvent, CGEventFlags, CGEventTapLocation, CGKeyCode},
    event_source::{CGEventSource, CGEventSourceStateID},
};
#[cfg(target_os = "macos")]
use objc::{msg_send, sel, sel_impl};

/// 捕获当前 macOS 前台应用；当前台应用属于 ClipPal 自身时返回空目标。
#[cfg(target_os = "macos")]
pub fn capture_target() -> AppResult<Option<AutoPasteTarget>> {
    unsafe {
        let workspace: id = msg_send![objc::class!(NSWorkspace), sharedWorkspace];
        if workspace == nil {
            return Err(AppError::AutoPaste("无法获取 NSWorkspace".to_string()));
        }

        let app: id = msg_send![workspace, frontmostApplication];
        if app == nil {
            return Ok(None);
        }

        let process_id: i32 = msg_send![app, processIdentifier];
        if process_id == std::process::id() as i32 {
            return Ok(None);
        }

        let app_name =
            ns_string(msg_send![app, localizedName]).unwrap_or_else(|| "Unknown".to_string());
        let bundle_id = ns_string(msg_send![app, bundleIdentifier]);
        log::debug!(
            "捕获自动粘贴目标应用: {} (PID: {}, bundle: {:?})",
            app_name,
            process_id,
            bundle_id
        );

        Ok(Some(AutoPasteTarget::MacOS {
            process_id,
            bundle_id,
            app_name,
        }))
    }
}

/// 激活 macOS 目标应用，并按 PID 验证它确实成为前台应用。
#[cfg(target_os = "macos")]
pub fn restore_and_verify(target: &AutoPasteTarget) -> AppResult<()> {
    let AutoPasteTarget::MacOS {
        process_id,
        bundle_id,
        app_name,
    } = target;

    unsafe {
        let app: id = msg_send![objc::class!(NSRunningApplication),
            runningApplicationWithProcessIdentifier: *process_id];
        if app == nil {
            return Err(AppError::AutoPaste(
                "目标应用已经退出，内容已复制".to_string(),
            ));
        }

        if let Some(expected_bundle_id) = bundle_id {
            let current_bundle_id = ns_string(msg_send![app, bundleIdentifier]);
            if current_bundle_id.as_deref() != Some(expected_bundle_id.as_str()) {
                return Err(AppError::AutoPaste(
                    "目标应用进程已发生变化，内容已复制".to_string(),
                ));
            }
        }

        let options: usize = 1 << 1; // NSApplicationActivateIgnoringOtherApps
        let activated: bool = msg_send![app, activateWithOptions: options];
        if !activated {
            return Err(AppError::AutoPaste(format!(
                "无法激活目标应用 {}，内容已复制",
                app_name
            )));
        }
    }

    for _ in 0..10 {
        if frontmost_process_id() == Some(*process_id) {
            log::debug!("自动粘贴目标应用已激活: {}", app_name);
            return Ok(());
        }
        std::thread::sleep(std::time::Duration::from_millis(30));
    }

    Err(AppError::AutoPaste(
        "无法确认目标应用已获得焦点，内容已复制".to_string(),
    ))
}

/// 向已经验证的 macOS 前台应用发送 Cmd+V。
#[cfg(target_os = "macos")]
pub fn send_paste_shortcut() -> AppResult<()> {
    if !accessibility_permission_granted() {
        return Err(AppError::AutoPaste(
            "需要辅助功能权限才能自动粘贴，内容已复制".to_string(),
        ));
    }

    let source = CGEventSource::new(CGEventSourceStateID::CombinedSessionState)
        .map_err(|error| AppError::AutoPaste(format!("创建按键事件源失败: {:?}", error)))?;
    let command_flags =
        CGEventFlags::from_bits_truncate(CGEventFlags::CGEventFlagCommand.bits() | 0x00000008);
    let v_key: CGKeyCode = 9;

    let v_down = CGEvent::new_keyboard_event(source.clone(), v_key, true)
        .map_err(|error| AppError::AutoPaste(format!("创建 V 按下事件失败: {:?}", error)))?;
    v_down.set_flags(command_flags);
    v_down.post(CGEventTapLocation::AnnotatedSession);

    std::thread::sleep(std::time::Duration::from_millis(20));

    let v_up = CGEvent::new_keyboard_event(source, v_key, false)
        .map_err(|error| AppError::AutoPaste(format!("创建 V 释放事件失败: {:?}", error)))?;
    v_up.set_flags(command_flags);
    v_up.post(CGEventTapLocation::AnnotatedSession);
    Ok(())
}

#[cfg(target_os = "macos")]
fn accessibility_permission_granted() -> bool {
    extern "C" {
        fn AXIsProcessTrusted() -> bool;
    }
    unsafe { AXIsProcessTrusted() }
}

#[cfg(target_os = "macos")]
fn frontmost_process_id() -> Option<i32> {
    unsafe {
        let workspace: id = msg_send![objc::class!(NSWorkspace), sharedWorkspace];
        if workspace == nil {
            return None;
        }
        let app: id = msg_send![workspace, frontmostApplication];
        if app == nil {
            return None;
        }
        Some(msg_send![app, processIdentifier])
    }
}

#[cfg(target_os = "macos")]
unsafe fn ns_string(value: id) -> Option<String> {
    if value == nil {
        return None;
    }
    let pointer: *const i8 = msg_send![value, UTF8String];
    if pointer.is_null() {
        return None;
    }
    Some(
        std::ffi::CStr::from_ptr(pointer)
            .to_string_lossy()
            .to_string(),
    )
}

#[cfg(not(any(windows, target_os = "macos")))]
pub fn capture_target() -> AppResult<Option<AutoPasteTarget>> {
    Ok(None)
}

#[cfg(not(any(windows, target_os = "macos")))]
pub fn restore_and_verify(_target: &AutoPasteTarget) -> AppResult<()> {
    Err(AppError::AutoPaste(
        "自动粘贴功能仅支持 Windows 和 macOS".to_string(),
    ))
}

#[cfg(not(any(windows, target_os = "macos")))]
pub fn send_paste_shortcut() -> AppResult<()> {
    Err(AppError::AutoPaste(
        "自动粘贴功能仅支持 Windows 和 macOS".to_string(),
    ))
}

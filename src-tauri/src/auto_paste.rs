use crate::errors::{AppError, AppResult};

#[cfg(windows)]
use once_cell::sync::Lazy;
#[cfg(windows)]
use std::sync::{Arc, Mutex};

#[cfg(target_os = "macos")]
use once_cell::sync::Lazy;
#[cfg(target_os = "macos")]
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

#[cfg(target_os = "macos")]
use cocoa::base::nil;
#[cfg(target_os = "macos")]
use objc::{msg_send, sel, sel_impl};

#[cfg(target_os = "macos")]
static PREVIOUS_APP_PID: Lazy<Arc<Mutex<Option<i32>>>> =
    Lazy::new(|| Arc::new(Mutex::new(None)));

/// 保存前台窗口信息 - macOS版本
#[cfg(target_os = "macos")]
pub fn save_foreground_window() {
    use cocoa::base::id;

    unsafe {
        let cls = objc::class!(NSWorkspace);
        let workspace: id = msg_send![cls, sharedWorkspace];

        if workspace != nil {
            let front_app: id = msg_send![workspace, frontmostApplication];

            if front_app != nil {
                // 获取应用名称
                let app_name: id = msg_send![front_app, localizedName];
                let name_ptr: *const i8 = msg_send![app_name, UTF8String];
                let name = if !name_ptr.is_null() {
                    std::ffi::CStr::from_ptr(name_ptr)
                        .to_string_lossy()
                        .to_string()
                } else {
                    "Unknown".to_string()
                };

                // 跳过ClipPal自己
                if name == "ClipPal" {
                    log::debug!("当前前台是ClipPal，不保存");
                    return;
                }

                // 获取进程ID
                let pid: i32 = msg_send![front_app, processIdentifier];

                if let Ok(mut previous) = PREVIOUS_APP_PID.lock() {
                    *previous = Some(pid);
                    log::info!("保存前台应用: {} (PID: {})", name, pid);
                }
            }
        }
    }
}

/// 执行自动粘贴 - macOS 简化版
#[cfg(target_os = "macos")]
pub fn auto_paste_to_previous_window() -> AppResult<()> {
    use crate::CONTEXT;
    use tauri::{AppHandle, Manager};

    log::info!("macOS 自动粘贴开始");

    // 获取窗口句柄
    let app_handle = CONTEXT.get::<AppHandle>();
    let window = app_handle
        .get_webview_window("main")
        .ok_or_else(|| {
            log::error!("无法获取主窗口");
            AppError::AutoPaste("无法获取主窗口".to_string())
        })?;

    log::debug!("已获取主窗口句柄");

    // 获取保存的前台应用PID
    let saved_pid = {
        let previous = PREVIOUS_APP_PID
            .lock()
            .map_err(|e| AppError::AutoPaste(format!("获取保存的应用PID失败: {}", e)))?;
        *previous
    };

    if let Some(pid) = saved_pid {
        log::info!("尝试激活保存的应用 (PID: {})", pid);
        activate_app_by_pid(pid)?;

        // 优化：缩短等待时间到 100ms
        std::thread::sleep(std::time::Duration::from_millis(100));
    } else {
        log::warn!("没有保存的前台应用，尝试激活任意应用");
        activate_previous_app()?;

        // 优化：缩短等待时间到 100ms
        std::thread::sleep(std::time::Duration::from_millis(100));
    }

    // 确保窗口已隐藏
    let is_visible = window.is_visible().unwrap_or(true);
    log::debug!("当前窗口可见状态: {}", is_visible);

    if is_visible {
        log::info!("准备隐藏窗口");
        window
            .hide()
            .map_err(|e| {
                log::error!("隐藏窗口失败: {}", e);
                AppError::AutoPaste(format!("隐藏窗口失败: {}", e))
            })?;

        log::debug!("窗口已隐藏");
    }

    // 验证前台应用是否正确（优化：减少验证次数和间隔）
    let mut verified = false;
    for i in 0..3 {
        if let Some(app_name) = get_frontmost_app_name() {
            log::debug!("第{}次验证，当前前台应用: {}", i + 1, app_name);
            if app_name != "ClipPal" {
                log::info!("前台应用验证成功: {}", app_name);
                verified = true;
                break;
            }
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    }

    if !verified {
        log::warn!("前台应用验证失败，但仍然尝试发送按键");
    }

    // 优化：缩短最后等待时间到 50ms
    std::thread::sleep(std::time::Duration::from_millis(50));

    log::info!("开始执行粘贴操作 (使用 CGEvent)");
    // 使用 CGEvent 发送 Cmd+V
    send_cmd_v()?;

    log::info!("macOS 自动粘贴完成");
    Ok(())
}

/// 使用菜单栏触发粘贴（配合重试机制）
#[cfg(target_os = "macos")]
fn trigger_paste_menu_item(pid: i32) -> AppResult<()> {
    use core_foundation::base::TCFType;
    use core_foundation::string::CFString;

    log::info!("使用 Accessibility API 触发粘贴菜单，目标应用 PID: {}", pid);

    unsafe {
        // 直接使用 PID 创建应用的 AXUIElement
        let app_element = accessibility_sys::AXUIElementCreateApplication(pid);

        if app_element.is_null() {
            log::error!("无法创建应用的 AXUIElement，PID: {}", pid);
            return Err(AppError::AutoPaste(format!("无法创建应用的 AXUIElement，PID: {}", pid)));
        }

        // 获取菜单栏
        let menu_bar_key = CFString::from_static_string("AXMenuBar");
        let mut menu_bar_value: core_foundation::base::CFTypeRef = std::ptr::null_mut();

        let result = accessibility_sys::AXUIElementCopyAttributeValue(
            app_element,
            menu_bar_key.as_CFTypeRef() as core_foundation::string::CFStringRef,
            &mut menu_bar_value
        );

        if result != 0 {
            core_foundation::base::CFRelease(app_element as *const std::ffi::c_void);
            log::error!("无法获取菜单栏: {}", result);
            return Err(AppError::AutoPaste("无法获取菜单栏".to_string()));
        }

        let menu_bar = menu_bar_value as accessibility_sys::AXUIElementRef;

        // 查找编辑菜单（支持中英文）
        let mut paste_item: Option<accessibility_sys::AXUIElementRef> = None;

        if let Some(edit_menu) = find_menu_by_title(menu_bar, "编辑") {
            log::debug!("找到编辑菜单");
            if let Some(item) = find_menu_by_title(edit_menu, "粘贴") {
                log::debug!("找到粘贴菜单项");
                paste_item = Some(item);
            }
            core_foundation::base::CFRelease(edit_menu as *const std::ffi::c_void);
        }

        if paste_item.is_none() {
            if let Some(edit_menu) = find_menu_by_title(menu_bar, "Edit") {
                log::debug!("找到 Edit 菜单");
                if let Some(item) = find_menu_by_title(edit_menu, "Paste") {
                    log::debug!("找到 Paste 菜单项");
                    paste_item = Some(item);
                }
                core_foundation::base::CFRelease(edit_menu as *const std::ffi::c_void);
            }
        }

        core_foundation::base::CFRelease(menu_bar as *const std::ffi::c_void);
        core_foundation::base::CFRelease(app_element as *const std::ffi::c_void);

        if let Some(paste) = paste_item {
            log::info!("找到粘贴菜单项，准备点击");

            let press_action = CFString::from_static_string("AXPress");
            let result = accessibility_sys::AXUIElementPerformAction(
                paste,
                press_action.as_CFTypeRef() as core_foundation::string::CFStringRef,
            );

            core_foundation::base::CFRelease(paste as *const std::ffi::c_void);

            if result != 0 {
                log::error!("触发粘贴菜单项失败: {}", result);
                return Err(AppError::AutoPaste(format!("触发粘贴菜单项失败: {}", result)));
            }

            log::info!("成功触发粘贴菜单项");
            Ok(())
        } else {
            log::error!("未找到粘贴菜单项");
            Err(AppError::AutoPaste("未找到粘贴菜单项".to_string()))
        }
    }
}

/// 在 UI 元素中查找指定标题的菜单项
#[cfg(target_os = "macos")]
fn find_menu_by_title(
    element: accessibility_sys::AXUIElementRef,
    title: &str,
) -> Option<accessibility_sys::AXUIElementRef> {
    use core_foundation::base::TCFType;
    use core_foundation::string::CFString;

    unsafe {
        // 获取子元素
        let children_key = CFString::from_static_string("AXChildren");
        let mut children_value: core_foundation::base::CFTypeRef = std::ptr::null_mut();

        let result = accessibility_sys::AXUIElementCopyAttributeValue(
            element,
            children_key.as_CFTypeRef() as core_foundation::string::CFStringRef,
            &mut children_value
        );

        if result != 0 {
            return None;
        }

        // children_value 是一个 CFArray
        let children_array = children_value as core_foundation::array::CFArrayRef;
        let count = core_foundation::array::CFArrayGetCount(children_array);

        for i in 0..count {
            let child = core_foundation::array::CFArrayGetValueAtIndex(children_array, i);
            if child.is_null() {
                continue;
            }

            let child_element = child as accessibility_sys::AXUIElementRef;

            // 获取标题
            let title_key = CFString::from_static_string("AXTitle");
            let mut title_value: core_foundation::base::CFTypeRef = std::ptr::null_mut();

            let result = accessibility_sys::AXUIElementCopyAttributeValue(
                child_element,
                title_key.as_CFTypeRef() as core_foundation::string::CFStringRef,
                &mut title_value
            );

            if result == 0 && !title_value.is_null() {
                let cf_title = CFString::wrap_under_get_rule(
                    title_value as core_foundation::string::CFStringRef
                );

                if cf_title.to_string() == title {
                    core_foundation::base::CFRelease(children_value as *const std::ffi::c_void);
                    // 增加引用计数，因为我们要返回这个元素
                    core_foundation::base::CFRetain(child);
                    return Some(child_element);
                }

                core_foundation::base::CFRelease(title_value as *const std::ffi::c_void);
            }
        }

        core_foundation::base::CFRelease(children_value as *const std::ffi::c_void);
        None
    }
}

/// 使用AppleScript执行粘贴操作（已废弃，保留用于参考）
#[cfg(target_os = "macos")]
#[allow(dead_code)]
fn send_cmd_v_via_applescript() -> AppResult<()> {
    use cocoa::base::{id, nil};
    use objc::{msg_send, sel, sel_impl};
    use std::ffi::CString;

    log::info!("使用NSAppleScript执行粘贴");

    unsafe {
        // AppleScript代码 - 使用key code方式
        // 9是V键的key code
        let script_str = "tell application \"System Events\"\nkey code 9 using {command down}\nend tell";

        // 使用CString确保null终止符
        let c_script = CString::new(script_str).map_err(|e| {
            log::error!("创建CString失败: {}", e);
            AppError::AutoPaste(format!("创建CString失败: {}", e))
        })?;

        // 创建NSString
        let ns_string_class = objc::class!(NSString);
        let ns_script: id = msg_send![ns_string_class, stringWithUTF8String: c_script.as_ptr()];

        if ns_script == nil {
            log::error!("创建NSString失败");
            return Err(AppError::AutoPaste("创建NSString失败".to_string()));
        }

        // 创建NSAppleScript对象
        let apple_script_class = objc::class!(NSAppleScript);
        let apple_script: id = msg_send![apple_script_class, alloc];
        let apple_script: id = msg_send![apple_script, initWithSource: ns_script];

        if apple_script == nil {
            log::error!("创建NSAppleScript失败");
            return Err(AppError::AutoPaste("创建NSAppleScript失败".to_string()));
        }

        // 执行脚本
        let mut error: id = nil;
        let _result: id = msg_send![apple_script, executeAndReturnError: &mut error];

        if error != nil {
            // 获取错误信息
            let error_desc: id = msg_send![error, description];
            let error_cstr: *const i8 = msg_send![error_desc, UTF8String];
            let error_msg = if !error_cstr.is_null() {
                std::ffi::CStr::from_ptr(error_cstr)
                    .to_string_lossy()
                    .to_string()
            } else {
                "Unknown error".to_string()
            };

            log::error!("NSAppleScript执行失败: {}", error_msg);
            return Err(AppError::AutoPaste(format!("NSAppleScript执行失败: {}", error_msg)));
        }

        log::info!("NSAppleScript执行成功");
    }

    Ok(())
}

/// 根据PID激活应用
#[cfg(target_os = "macos")]
fn activate_app_by_pid(pid: i32) -> AppResult<()> {
    use cocoa::base::id;

    unsafe {
        let cls = objc::class!(NSRunningApplication);
        let app: id = msg_send![cls, runningApplicationWithProcessIdentifier: pid];

        if app == nil {
            log::warn!("无法找到PID为{}的应用", pid);
            return Err(AppError::AutoPaste(format!("无法找到PID为{}的应用", pid)));
        }

        // 获取应用名称用于日志
        let app_name: id = msg_send![app, localizedName];
        let name_ptr: *const i8 = msg_send![app_name, UTF8String];
        let name = if !name_ptr.is_null() {
            std::ffi::CStr::from_ptr(name_ptr)
                .to_string_lossy()
                .to_string()
        } else {
            format!("PID:{}", pid)
        };

        log::info!("激活应用: {}", name);

        let options: usize = 1 << 1; // NSApplicationActivateIgnoringOtherApps
        let result: bool = msg_send![app, activateWithOptions: options];

        if result {
            log::info!("成功激活应用: {}", name);
            Ok(())
        } else {
            log::error!("激活应用失败: {}", name);
            Err(AppError::AutoPaste(format!("激活应用失败: {}", name)))
        }
    }
}

/// 激活上一个活动的应用（非ClipPal）
#[cfg(target_os = "macos")]
fn activate_previous_app() -> AppResult<()> {
    use cocoa::base::id;

    unsafe {
        let cls = objc::class!(NSWorkspace);
        let workspace: id = msg_send![cls, sharedWorkspace];

        if workspace == nil {
            return Err(AppError::AutoPaste("无法获取NSWorkspace".to_string()));
        }

        // 获取所有运行的应用
        let running_apps: id = msg_send![workspace, runningApplications];
        if running_apps == nil {
            return Err(AppError::AutoPaste("无法获取运行中的应用列表".to_string()));
        }

        let count: usize = msg_send![running_apps, count];
        log::debug!("共有{}个运行中的应用", count);

        // 遍历查找最近活动的非ClipPal应用
        for i in 0..count {
            let app: id = msg_send![running_apps, objectAtIndex: i];
            if app == nil {
                continue;
            }

            // 获取应用名称
            let app_name: id = msg_send![app, localizedName];
            if app_name != nil {
                let name_ptr: *const i8 = msg_send![app_name, UTF8String];
                if !name_ptr.is_null() {
                    let name = std::ffi::CStr::from_ptr(name_ptr)
                        .to_string_lossy()
                        .to_string();

                    // 跳过ClipPal自己
                    if name == "ClipPal" {
                        log::debug!("跳过ClipPal");
                        continue;
                    }

                    // 检查是否是常规应用（不是系统服务等）
                    let activation_policy: i64 = msg_send![app, activationPolicy];
                    // NSApplicationActivationPolicyRegular = 0
                    if activation_policy != 0 {
                        log::debug!("跳过非常规应用: {} (policy: {})", name, activation_policy);
                        continue;
                    }

                    // 尝试激活这个应用
                    log::info!("尝试激活应用: {}", name);
                    let options: usize = 1 << 1; // NSApplicationActivateIgnoringOtherApps
                    let result: bool = msg_send![app, activateWithOptions: options];

                    if result {
                        log::info!("成功激活应用: {}", name);
                        return Ok(());
                    } else {
                        log::warn!("激活应用失败: {}", name);
                    }
                }
            }
        }

        Err(AppError::AutoPaste("未找到可激活的应用".to_string()))
    }
}

/// 获取当前前台应用名称
#[cfg(target_os = "macos")]
fn get_frontmost_app_name() -> Option<String> {
    use cocoa::base::id;
    use std::ffi::CStr;

    unsafe {
        let cls = objc::class!(NSWorkspace);
        let workspace: id = msg_send![cls, sharedWorkspace];

        if workspace != nil {
            let front_app: id = msg_send![workspace, frontmostApplication];

            if front_app != nil {
                let app_name: id = msg_send![front_app, localizedName];

                if app_name != nil {
                    let name_ptr: *const i8 = msg_send![app_name, UTF8String];

                    if !name_ptr.is_null() {
                        let name = CStr::from_ptr(name_ptr)
                            .to_string_lossy()
                            .to_string();
                        return Some(name);
                    }
                }
            }
        }
    }

    None
}

/// 记录当前前台应用名称（用于调试）
#[cfg(target_os = "macos")]
fn log_frontmost_app() {
    if let Some(name) = get_frontmost_app_name() {
        log::info!("当前前台应用: {}", name);
    } else {
        log::warn!("无法获取前台应用名称");
    }
}

/// 检查辅助功能权限
#[cfg(target_os = "macos")]
fn check_accessibility_permissions() -> bool {
    // 使用 accessibility-sys 提供的底层 C API
    // 这个函数检查当前进程是否被信任可以使用辅助功能 API
    extern "C" {
        fn AXIsProcessTrusted() -> bool;
    }

    unsafe { AXIsProcessTrusted() }
}

/// 检查系统剪贴板内容
#[cfg(target_os = "macos")]
fn check_clipboard_content() -> Option<String> {
    use cocoa::appkit::NSPasteboard;
    use cocoa::base::{id, nil};
    use cocoa::foundation::NSString;

    unsafe {
        let pasteboard: id = NSPasteboard::generalPasteboard(nil);
        let available_types: id = msg_send![pasteboard, types];

        if available_types == nil {
            log::warn!("剪贴板没有可用类型");
            return None;
        }

        let string_type = NSString::alloc(nil).init_str("public.utf8-plain-text");
        let contains: bool = msg_send![available_types, containsObject: string_type];

        if contains {
            let content: id = msg_send![pasteboard, stringForType: string_type];
            if content != nil {
                let c_str: *const i8 = msg_send![content, UTF8String];
                if !c_str.is_null() {
                    let content_str = std::ffi::CStr::from_ptr(c_str)
                        .to_string_lossy()
                        .to_string();
                    return Some(content_str);
                }
            }
        }

        None
    }
}

/// 模拟 Cmd+V - 基于 Maccy 的实现方式
#[cfg(target_os = "macos")]
fn send_cmd_v() -> AppResult<()> {
    log::info!("使用 CGEvent 发送 Cmd+V (Maccy 方式)");

    // 检查辅助功能权限
    let has_permission = check_accessibility_permissions();
    log::info!("辅助功能权限状态: {}", if has_permission { "已授予" } else { "未授予 ⚠️" });

    if !has_permission {
        log::error!("❌ 未授予辅助功能权限！请在系统设置 > 隐私与安全性 > 辅助功能中授予 ClipPal 权限");
        return Err(AppError::AutoPaste(
            "需要辅助功能权限才能执行自动粘贴。请在系统设置中授予权限。".to_string()
        ));
    }

    // 检查剪贴板内容
    if let Some(content) = check_clipboard_content() {
        let preview = if content.len() > 50 {
            format!("{}...", &content[..50])
        } else {
            content.clone()
        };
        log::info!("剪贴板内容预览: '{}'", preview);
    } else {
        log::warn!("⚠️ 剪贴板为空或无法读取内容");
    }

    unsafe {
        // 使用 CombinedSessionState 而不是 HIDSystemState
        let source = CGEventSource::new(CGEventSourceStateID::CombinedSessionState)
            .map_err(|e| {
                log::error!("创建事件源失败: {:?}", e);
                AppError::AutoPaste(format!("创建事件源失败: {:?}", e))
            })?;

        log::debug!("CGEventSource 创建成功 (CombinedSessionState)");

        let v_key: CGKeyCode = 9; // V 键的键码

        // 设置 Command 标志，包括设备特定的左 Command 键标志
        // CGEventFlagCommand = 0x100000 (general command flag)
        // NX_DEVICELCMDKEYMASK = 0x00000008 (device-specific left command key)
        let command_flags = CGEventFlags::from_bits_truncate(
            CGEventFlags::CGEventFlagCommand.bits() | 0x00000008
        );

        log::debug!("创建 V 键按下事件，标志: 0x{:x}", command_flags.bits());

        // 按下 V 键（带 Command 标志）
        let v_down = CGEvent::new_keyboard_event(source.clone(), v_key, true)
            .map_err(|e| {
                log::error!("创建 V 按下事件失败: {:?}", e);
                AppError::AutoPaste(format!("创建 V 按下事件失败: {:?}", e))
            })?;
        v_down.set_flags(command_flags);
        // 使用 AnnotatedSession 而不是 HID
        v_down.post(core_graphics::event::CGEventTapLocation::AnnotatedSession);

        log::debug!("已发送 V 键按下事件");

        // 短暂延迟
        std::thread::sleep(std::time::Duration::from_millis(20));

        // 释放 V 键
        let v_up = CGEvent::new_keyboard_event(source, v_key, false)
            .map_err(|e| {
                log::error!("创建 V 释放事件失败: {:?}", e);
                AppError::AutoPaste(format!("创建 V 释放事件失败: {:?}", e))
            })?;
        v_up.set_flags(command_flags);
        v_up.post(core_graphics::event::CGEventTapLocation::AnnotatedSession);

        log::debug!("已发送 V 键释放事件");
    }

    log::info!("Cmd+V 按键事件发送完成");
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

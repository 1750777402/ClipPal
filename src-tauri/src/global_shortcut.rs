use crate::services::clipboard_service::ClipboardService;
use crate::{app_context::AppContext, domain::settings::Settings};
use std::sync::Arc;
use tauri::{App, AppHandle, Manager};
use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Modifiers, Shortcut};

pub fn init_global_shortcut(app: &App, app_context: Arc<AppContext>) -> tauri::Result<()> {
    #[cfg(desktop)]
    {
        // 首先注册插件
        app.handle()
            .plugin(tauri_plugin_global_shortcut::Builder::new().build())?;

        // 从设置中获取快捷键
        let settings = app_context.with_settings(Clone::clone).map_err(|e| {
            log::error!("获取设置失败: {}", e);
            tauri::Error::FailedToReceiveMessage
        })?;
        let shortcut_str = settings.shortcut_key.clone();

        // 注册快捷键并设置处理器
        let shortcut_obj = match parse_shortcut_strict(&shortcut_str) {
            Ok(shortcut) => shortcut,
            Err(error) => {
                log::warn!("配置中的快捷键无效，回退到默认值: {}", error);
                parse_shortcut(&Settings::default().shortcut_key)
            }
        };
        register_shortcut_handler(app.handle(), app_context, shortcut_obj).map_err(|e| {
            log::error!("快捷键注册失败: {}", e);
            tauri::Error::FailedToReceiveMessage
        })?;

        log::info!("全局快捷键初始化成功: {}", shortcut_str);
    }
    Ok(())
}

/// 注册统一的快捷键处理器，初始化和设置变更必须共用同一条唤起链路。
pub fn register_shortcut_handler(
    app_handle: &AppHandle,
    app_context: Arc<AppContext>,
    shortcut: Shortcut,
) -> Result<(), String> {
    app_handle
        .global_shortcut()
        .on_shortcut(shortcut, {
            let app_handle = app_handle.clone();
            move |_app, shortcut, event| {
                log::debug!("快捷键触发: {:?}, 状态: {:?}", shortcut, event.state());
                if event.state() == tauri_plugin_global_shortcut::ShortcutState::Pressed {
                    if let Some(window) = app_handle.get_webview_window("main") {
                        if let Err(error) = ClipboardService::from_context(app_context.as_ref())
                            .capture_auto_paste_target()
                        {
                            log::warn!("保存自动粘贴目标窗口失败: {}", error);
                        }

                        let _ = window.show();
                        let _ = window.set_focus();
                        log::debug!("窗口已显示并聚焦");
                    }
                }
            }
        })
        .map_err(|error| error.to_string())
}

fn parse_modifier(part: &str) -> Option<Modifiers> {
    match part {
        "Ctrl" => Some(Modifiers::CONTROL),
        "Shift" => Some(Modifiers::SHIFT),
        "Alt" => Some(Modifiers::ALT),
        "Meta" | "Cmd" => Some(Modifiers::META),
        _ => None,
    }
}

fn parse_key_code(part: &str) -> Option<Code> {
    match part {
        "`" => Some(Code::Backquote),
        "-" => Some(Code::Minus),
        "=" => Some(Code::Equal),
        "[" => Some(Code::BracketLeft),
        "]" => Some(Code::BracketRight),
        "\\" => Some(Code::Backslash),
        ";" => Some(Code::Semicolon),
        "'" => Some(Code::Quote),
        "," => Some(Code::Comma),
        "." => Some(Code::Period),
        "/" => Some(Code::Slash),
        "Space" => Some(Code::Space),
        "Enter" => Some(Code::Enter),
        "Tab" => Some(Code::Tab),
        "Escape" => Some(Code::Escape),
        "Backspace" => Some(Code::Backspace),
        "Delete" => Some(Code::Delete),
        "ArrowUp" => Some(Code::ArrowUp),
        "ArrowDown" => Some(Code::ArrowDown),
        "ArrowLeft" => Some(Code::ArrowLeft),
        "ArrowRight" => Some(Code::ArrowRight),
        "Home" => Some(Code::Home),
        "End" => Some(Code::End),
        "PageUp" => Some(Code::PageUp),
        "PageDown" => Some(Code::PageDown),
        "Insert" => Some(Code::Insert),
        "F1" => Some(Code::F1),
        "F2" => Some(Code::F2),
        "F3" => Some(Code::F3),
        "F4" => Some(Code::F4),
        "F5" => Some(Code::F5),
        "F6" => Some(Code::F6),
        "F7" => Some(Code::F7),
        "F8" => Some(Code::F8),
        "F9" => Some(Code::F9),
        "F10" => Some(Code::F10),
        "F11" => Some(Code::F11),
        "F12" => Some(Code::F12),
        c if c.len() == 1 => {
            let ch = c.chars().next()?;
            if ch.is_ascii_alphabetic() {
                Some(match ch.to_ascii_uppercase() {
                    'A' => Code::KeyA,
                    'B' => Code::KeyB,
                    'C' => Code::KeyC,
                    'D' => Code::KeyD,
                    'E' => Code::KeyE,
                    'F' => Code::KeyF,
                    'G' => Code::KeyG,
                    'H' => Code::KeyH,
                    'I' => Code::KeyI,
                    'J' => Code::KeyJ,
                    'K' => Code::KeyK,
                    'L' => Code::KeyL,
                    'M' => Code::KeyM,
                    'N' => Code::KeyN,
                    'O' => Code::KeyO,
                    'P' => Code::KeyP,
                    'Q' => Code::KeyQ,
                    'R' => Code::KeyR,
                    'S' => Code::KeyS,
                    'T' => Code::KeyT,
                    'U' => Code::KeyU,
                    'V' => Code::KeyV,
                    'W' => Code::KeyW,
                    'X' => Code::KeyX,
                    'Y' => Code::KeyY,
                    'Z' => Code::KeyZ,
                    _ => return None,
                })
            } else if ch.is_ascii_digit() {
                Some(match ch {
                    '0' => Code::Digit0,
                    '1' => Code::Digit1,
                    '2' => Code::Digit2,
                    '3' => Code::Digit3,
                    '4' => Code::Digit4,
                    '5' => Code::Digit5,
                    '6' => Code::Digit6,
                    '7' => Code::Digit7,
                    '8' => Code::Digit8,
                    '9' => Code::Digit9,
                    _ => return None,
                })
            } else {
                None
            }
        }
        _ => None,
    }
}

pub fn parse_shortcut_strict(shortcut_str: &str) -> Result<Shortcut, String> {
    let parts: Vec<&str> = shortcut_str
        .split('+')
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .collect();

    if parts.len() < 2 || parts.len() > 4 {
        return Err("快捷键必须包含 1-3 个修饰键和 1 个主键".to_string());
    }

    let mut modifiers = Modifiers::empty();
    let mut key_code: Option<Code> = None;

    for part in parts {
        if let Some(modifier) = parse_modifier(part) {
            if modifiers.contains(modifier) {
                return Err(format!("快捷键中包含重复的修饰键: {}", part));
            }
            modifiers |= modifier;
            continue;
        }

        if key_code.is_some() {
            return Err("快捷键只能包含一个主键".to_string());
        }

        key_code = parse_key_code(part);
        if key_code.is_none() {
            return Err(format!("当前不支持该按键: {}", part));
        }
    }

    if modifiers.is_empty() {
        return Err("快捷键至少需要一个修饰键".to_string());
    }

    let code = key_code.ok_or_else(|| "快捷键缺少主键".to_string())?;
    Ok(Shortcut::new(Some(modifiers), code))
}

// 解析快捷键字符串（保持向后兼容）
pub fn parse_shortcut(shortcut_str: &str) -> Shortcut {
    parse_shortcut_strict(shortcut_str).unwrap_or_else(|error| {
        log::warn!("快捷键解析失败，回退默认值: {}", error);
        Shortcut::new(Some(Modifiers::CONTROL), Code::Backquote)
    })
}

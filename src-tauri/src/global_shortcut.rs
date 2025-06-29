use crate::auto_paste;
use crate::errors::lock_utils::safe_lock;
use tauri::App;

pub fn init_global_shortcut(app: &App) -> tauri::Result<()> {
    #[cfg(desktop)]
    {
        use crate::{CONTEXT, biz::system_setting::Settings};
        use tauri_plugin_global_shortcut::GlobalShortcutExt;

        // 首先注册插件
        app.handle()
            .plugin(tauri_plugin_global_shortcut::Builder::new().build())?;

        // 从设置中获取快捷键
        let settings = {
            use std::sync::{Arc, Mutex};

            let lock = CONTEXT.get::<Arc<Mutex<Settings>>>().clone();
            let result = match safe_lock(&lock) {
                Ok(current) => current.clone(),
                Err(e) => {
                    log::error!("获取设置锁失败: {}", e);
                    return Err(tauri::Error::FailedToReceiveMessage);
                }
            };
            result
        };
        let shortcut_str = settings.shortcut_key.clone();

        // 注册快捷键并设置处理器
        let shortcut_obj = parse_shortcut(&shortcut_str);
        app.handle()
            .global_shortcut()
            .on_shortcut(shortcut_obj, {
                let app_handle = app.handle().clone();
                move |_app, shortcut, event| {
                    log::debug!("快捷键触发: {:?}, 状态: {:?}", shortcut, event.state());
                    if event.state() == tauri_plugin_global_shortcut::ShortcutState::Pressed {
                        // 在显示粘贴板窗口之前，先保存当前获得焦点的窗口
                        auto_paste::save_foreground_window();

                        use tauri::Manager;
                        if let Some(window) = app_handle.get_webview_window("main") {
                            let _ = window.show();
                            let _ = window.set_focus();
                        }
                    }
                }
            })
            .map_err(|e| {
                log::error!("快捷键注册失败: {}", e);
                tauri::Error::FailedToReceiveMessage
            })?;

        log::info!("全局快捷键初始化成功: {}", shortcut_str);
    }
    Ok(())
}

// 解析快捷键字符串（保持向后兼容）
pub fn parse_shortcut(shortcut_str: &str) -> tauri_plugin_global_shortcut::Shortcut {
    use tauri_plugin_global_shortcut::{Code, Modifiers};

    let parts: Vec<&str> = shortcut_str.split('+').collect();
    let mut modifiers = Modifiers::empty();
    let mut code = Code::KeyA; // 默认值

    for part in parts {
        match part {
            "Ctrl" => modifiers |= Modifiers::CONTROL,
            "Shift" => modifiers |= Modifiers::SHIFT,
            "Alt" => modifiers |= Modifiers::ALT,
            "Meta" => modifiers |= Modifiers::META,
            "`" => code = Code::Backquote,
            "Space" => code = Code::Space,
            "Enter" => code = Code::Enter,
            "Tab" => code = Code::Tab,
            "Escape" => code = Code::Escape,
            "Backspace" => code = Code::Backspace,
            "Delete" => code = Code::Delete,
            "ArrowUp" => code = Code::ArrowUp,
            "ArrowDown" => code = Code::ArrowDown,
            "ArrowLeft" => code = Code::ArrowLeft,
            "ArrowRight" => code = Code::ArrowRight,
            "Home" => code = Code::Home,
            "End" => code = Code::End,
            "PageUp" => code = Code::PageUp,
            "PageDown" => code = Code::PageDown,
            "Insert" => code = Code::Insert,
            "F1" => code = Code::F1,
            "F2" => code = Code::F2,
            "F3" => code = Code::F3,
            "F4" => code = Code::F4,
            "F5" => code = Code::F5,
            "F6" => code = Code::F6,
            "F7" => code = Code::F7,
            "F8" => code = Code::F8,
            "F9" => code = Code::F9,
            "F10" => code = Code::F10,
            "F11" => code = Code::F11,
            "F12" => code = Code::F12,
            // 处理单个字符
            c if c.len() == 1 => {
                if let Some(ch) = c.chars().next() {
                    if ch.is_ascii_alphabetic() {
                        code = match ch.to_ascii_uppercase() {
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
                            _ => Code::KeyA,
                        };
                    } else if ch.is_ascii_digit() {
                        code = match ch {
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
                            _ => Code::Digit0,
                        };
                    }
                }
            }
            _ => {}
        }
    }

    tauri_plugin_global_shortcut::Shortcut::new(Some(modifiers), code)
}

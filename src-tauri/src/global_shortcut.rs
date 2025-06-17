use tauri::App;

pub fn init_global_shortcut(app: &App) -> tauri::Result<()> {
    #[cfg(desktop)]
    {
        use crate::{CONTEXT, biz::system_setting::Settings};
        use tauri_plugin_global_shortcut::GlobalShortcutExt;

        // 从设置中获取快捷键
        let settings = {
            use std::sync::{Arc, Mutex};

            let lock = CONTEXT.get::<Arc<Mutex<Settings>>>().clone();
            let current = lock.lock().unwrap();
            current.clone()
        };
        let shortcut_str = settings.shortcut_key.clone();

        // 解析快捷键字符串
        let shortcut = parse_shortcut(&shortcut_str);

        app.handle().plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(move |app, registered_shortcut, _| {
                    let current_shortcut_str ={
                        use std::sync::{Arc, Mutex};
            
                        let lock = CONTEXT.get::<Arc<Mutex<Settings>>>().clone();
                        let current = lock.lock().unwrap();
                        current.clone()
                    }.shortcut_key.clone();
                    // 解析快捷键字符串
                    let current_shortcut = parse_shortcut(&current_shortcut_str);
                    println!("Received shortcut: {:?}", registered_shortcut);
                    println!("Received shortcut1111: {:?}", current_shortcut);
                    if registered_shortcut == &current_shortcut {
                        use tauri::Manager;
                        if let Some(window) = app.get_webview_window("main") {
                            let _ = window.show();
                            let _ = window.set_focus();
                        }
                    }
                })
                .build(),
        )?;

        app.global_shortcut()
            .register(shortcut)
            .unwrap_or_else(|e| println!("快捷键设置失败:{}", e));
    }
    Ok(())
}

// 解析快捷键字符串
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
                let ch = c.chars().next().unwrap();
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
            _ => {}
        }
    }

    tauri_plugin_global_shortcut::Shortcut::new(Some(modifiers), code)
}

use tauri::App;

pub fn init_global_shortcut(app: &App) -> tauri::Result<()> {
    #[cfg(desktop)]
    {
        use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Modifiers, Shortcut};

        let ctrl_n_shortcut = Shortcut::new(Some(Modifiers::CONTROL), Code::Backquote);
        app.handle().plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(move |app, shortcut, _| {
                    if shortcut == &ctrl_n_shortcut {
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
            .register(ctrl_n_shortcut)
            .unwrap_or_else(|e| println!("快捷键设置失败:{}", e));
    }
    Ok(())
}

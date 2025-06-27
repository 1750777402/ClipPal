use tauri::App;

pub fn init_single_instance(app: &App) -> tauri::Result<()> {
    #[cfg(desktop)]
    {
        app.handle()
            .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
                // 当用户尝试第二次启动程序时，会触发这个回调

                use tauri::Manager;
                log::info!("检测到已有实例，激活已有主窗口...");
                if let Some(window) = app.get_webview_window("main") {
                    // 显示并聚焦已有主窗口
                    let _ = window.show();
                    let _ = window.set_focus();
                }
            }))?;
    }
    Ok(())
}

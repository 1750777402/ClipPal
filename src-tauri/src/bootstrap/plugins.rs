use tauri::{Manager, Runtime};
use tauri_plugin_autostart::MacosLauncher;

/// 注册 Tauri 插件。
///
/// 这里只负责插件装配，不处理业务初始化。
pub fn register_plugins<R>(builder: tauri::Builder<R>) -> tauri::Builder<R>
where
    R: Runtime,
{
    builder
        // 软件自动更新
        .plugin(tauri_plugin_updater::Builder::new().build())
        // 本机系统对话框，用于打开、保存文件和展示消息弹窗
        .plugin(tauri_plugin_dialog::init())
        // 保存窗口位置和大小，并在应用重新打开时恢复
        .plugin(tauri_plugin_window_state::Builder::new().build())
        // 使用系统默认应用打开文件或 URL
        .plugin(tauri_plugin_opener::init())
        // ClipPal 自定义剪贴板插件
        .plugin(tauri_plugin_clipboard_pal::init())
        // 开机自启插件
        .plugin(tauri_plugin_autostart::init(
            MacosLauncher::LaunchAgent,
            Some(vec!["--autostart"]),
        ))
        // HTTP 请求插件
        .plugin(tauri_plugin_http::init())
        // 单实例插件：第二次启动时显示并聚焦已有主窗口
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.show();
                let _ = window.set_focus();
            }
        }))
}

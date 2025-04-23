use tauri::Runtime;
use tauri_plugin_autostart::MacosLauncher;

pub trait CustomInit {
    fn init_plugin(self) -> Self;
}

impl<R: Runtime> CustomInit for tauri::Builder<R> {
    fn init_plugin(self) -> Self {
        self.plugin(tauri_plugin_opener::init())
            // 粘贴板插件
            .plugin(tauri_plugin_clipboard_manager::init())
            // 开机自启插件
            .plugin(tauri_plugin_autostart::init(
                MacosLauncher::LaunchAgent,
                Some(vec!["--flag1", "--flag2"]),
            ))
            // http请求插件
            .plugin(tauri_plugin_http::init())
    }
}

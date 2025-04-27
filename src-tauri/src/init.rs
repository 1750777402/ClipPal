use tauri::Runtime;
use tauri_plugin_autostart::MacosLauncher;

pub trait CustomInit {
    fn init_plugin(self) -> Self;
}

impl<R: Runtime> CustomInit for tauri::Builder<R> {
    fn init_plugin(self) -> Self {
        self
            // opener 插件  用于在程序中打开文件和URL 还支持在系统的文件资源管理器中“显示”文件
            .plugin(tauri_plugin_opener::init())
            // 粘贴板插件
            .plugin(tauri_plugin_clipboard_pal::init())
            // 开机自启插件
            .plugin(tauri_plugin_autostart::init(
                MacosLauncher::LaunchAgent,
                Some(vec!["--flag1", "--flag2"]),
            ))
            // http请求插件
            .plugin(tauri_plugin_http::init())
            // 全局快捷键设置插件
            .plugin(tauri_plugin_global_shortcut::Builder::new().build())
            // sql功能插件，比如使用SQLite等
            .plugin(tauri_plugin_sql::Builder::default().build())
    }
}

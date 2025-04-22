use tauri::{Manager, PhysicalPosition, PhysicalSize};
use tauri::tray::TrayIconBuilder;

// Learn more about Tauri commands at https://tauri.app/develop/calling-rust/
#[tauri::command]
fn greet(name: &str) -> String {
    format!("Hello, {}! You've been greeted from Rust!", name)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            // 设置图标栏图标
            let tray = TrayIconBuilder::new()
                .icon(app.default_window_icon().unwrap().clone())
                .build(app)?;
            // 获取主显示器
            let main_window = app.get_webview_window("main").unwrap();
            // 获取主显示器信息
            let monitor = main_window
                .primary_monitor()
                .expect("Failed to get primary monitor")
                .expect("No primary monitor found");

            // 获取显示器参数
            let screen_size = monitor.size();
            let screen_height = screen_size.height as i32;
            let y_position = (screen_height - 490).max(0);

            // 设置窗口参数
            main_window.set_size(PhysicalSize::new(screen_size.width, 500))?;
            main_window.set_position(PhysicalPosition::new(0, y_position))?;
            // 延迟显示
            std::thread::sleep(std::time::Duration::from_millis(100));
            main_window.show().unwrap();

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![greet])
        .run(tauri::generate_context!())
        .expect("error running tauri");
}

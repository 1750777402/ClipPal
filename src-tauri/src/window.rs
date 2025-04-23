use tauri::App;
use tauri::{Manager, PhysicalPosition, PhysicalSize};

pub fn init_main_window(app: &App) -> tauri::Result<()> {
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
}

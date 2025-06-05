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
    let screen_width = screen_size.width as i32;
    let screen_height = screen_size.height as i32;

    // 设置窗口参数 - 右侧贴边，高度与屏幕同高
    let window_width = 400; // 可以根据需要调整窗口宽度
    let x_position = (screen_width - window_width).max(0);

    // 设置窗口大小和位置
    main_window.set_size(PhysicalSize::new(window_width, screen_height))?;
    main_window.set_position(PhysicalPosition::new(x_position, 0))?;
    // 延迟显示
    std::thread::sleep(std::time::Duration::from_millis(100));
    main_window.show().unwrap();
    Ok(())
}

//! Tauri 打包壳：生成宿主 context，并向桌面适配包注入静态资产。
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub async fn run() -> Result<(), Box<dyn std::error::Error>> {
    desktop_tauri::run(
        tauri::generate_context!(),
        desktop_tauri::PackagedAssets {
            config_json: include_str!("../config.json"),
            tray_icon_png: include_bytes!("../icons/icon_128x128.png"),
        },
    ).await
}

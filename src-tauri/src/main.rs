// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = windows)]

#[tokio::main]
async fn main() {
    if let Err(e) = clip_pal::run().await {
        log::error!("应用程序启动失败: {}", e);
        std::process::exit(1);
    }
}

use tauri::App;
use tauri::{plugin::Plugin, Manager};
use tauri_plugin_clipboard_pal::desktop::ClipboardPal;

pub fn init_clip_board_listener(app: &App) -> tauri::Result<()> {
    let clipboard = app.handle().state::<ClipboardPal>();
    let _ = clipboard.start_monitor(app.handle().clone());
    Ok(())
}

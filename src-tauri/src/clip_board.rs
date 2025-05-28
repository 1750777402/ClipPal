use std::sync::Arc;

use clipboard_listener::ClipboardEvent;
use clipboard_listener::EventManager;
use tauri::App;
use tauri::Manager;
use tauri_plugin_clipboard_pal::desktop::ClipboardPal;

pub fn init_clip_board_listener(
    app: &App,
    manager: Arc<EventManager<ClipboardEvent>>,
) -> tauri::Result<()> {
    let clipboard = app.handle().state::<ClipboardPal>();
    let _ = clipboard.start_monitor(manager);
    Ok(())
}

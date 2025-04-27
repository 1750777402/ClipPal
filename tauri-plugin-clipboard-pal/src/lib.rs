use tauri::{
    plugin::{Builder, TauriPlugin},
    Manager, Runtime,
};

#[cfg(desktop)]
pub mod desktop;

mod error;

pub use error::{Error, Result};

/// Initializes the plugin.
pub fn init<R: Runtime>() -> TauriPlugin<R> {
    Builder::new("clipboard-pal")
        .invoke_handler(tauri::generate_handler![])
        .setup(|app, api| {
            #[cfg(desktop)]
            let clipboard_pal = desktop::init(api)?;
            app.manage(clipboard_pal);
            Ok(())
        })
        .build()
}

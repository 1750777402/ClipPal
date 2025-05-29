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
    // 初始化插件
    Builder::new("clipboard-pal")
        .invoke_handler(tauri::generate_handler![])
        .setup(move |app, _api| {
            #[cfg(desktop)]
            let clipboard_pal = desktop::init()?;
            app.manage(clipboard_pal);
            Ok(())
        })
        .build()
}

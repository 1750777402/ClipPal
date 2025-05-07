use clipboard_rs::{
    Clipboard as ClipboardRS, ClipboardContext as ClipboardRsContext, ClipboardHandler,
    ClipboardWatcher, ClipboardWatcherContext, ContentFormat, WatcherShutdown,
};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use std::sync::{Arc, Mutex};
use tauri::{plugin::PluginApi, Runtime};

pub fn init<R: Runtime, C: DeserializeOwned>(_api: PluginApi<R, C>) -> crate::Result<ClipboardPal> {
    Ok(ClipboardPal {
        clipboard: Arc::new(Mutex::new(ClipboardRsContext::new().unwrap())),
        watcher_shutdown: Arc::default(),
    })
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AvailableTypes {
    pub text: bool,
    pub html: bool,
    pub rtf: bool,
    pub image: bool,
    pub files: bool,
}

pub struct ClipboardPal {
    pub clipboard: Arc<Mutex<ClipboardRsContext>>,
    pub watcher_shutdown: Arc<Mutex<Option<WatcherShutdown>>>,
}

impl ClipboardPal {
    pub fn start_monitor(&self) -> Result<(), String> {
        let clipboard = ClipboardMonitor::new(self.clipboard.clone());
        let mut watcher: ClipboardWatcherContext<ClipboardMonitor> =
            ClipboardWatcherContext::new().unwrap();
        let watcher_shutdown = watcher.add_handler(clipboard).get_shutdown_channel();
        let mut watcher_shutdown_state = self.watcher_shutdown.lock().unwrap();
        if (*watcher_shutdown_state).is_some() {
            return Ok(());
        }
        *watcher_shutdown_state = Some(watcher_shutdown);
        std::thread::spawn(move || {
            watcher.start_watch();
        });
        Ok(())
    }

    pub fn stop_monitor(&self) -> Result<(), String> {
        let mut watcher_shutdown_state = self.watcher_shutdown.lock().unwrap();
        if let Some(watcher_shutdown) = (*watcher_shutdown_state).take() {
            watcher_shutdown.stop();
        }
        *watcher_shutdown_state = None;
        Ok(())
    }

    pub fn is_monitor_running(&self) -> bool {
        (*self.watcher_shutdown.lock().unwrap()).is_some()
    }
}

pub struct ClipboardMonitor {
    pub clipboard: Arc<Mutex<ClipboardRsContext>>,
}

impl ClipboardMonitor {
    pub fn new(clipboard: Arc<Mutex<ClipboardRsContext>>) -> Self {
        Self { clipboard }
    }
}

impl ClipboardHandler for ClipboardMonitor {
    fn on_clipboard_change(&mut self) {
        println!("");
        let clipboard_context = self
            .clipboard
            .lock()
            .map_err(|err| err.to_string())
            .unwrap();

        if clipboard_context.has(ContentFormat::Text) {
            let text_context = clipboard_context.get_text().map_err(|err| err.to_string());
            if let Ok(text) = text_context {
                println!("复制了文本{:?}", text);
            }
        }
        if clipboard_context.has(ContentFormat::Image) {
            let text_context = clipboard_context.get_image().map_err(|err| err.to_string());
            if let Ok(image) = text_context {
                println!("复制了图片");
            }
        }
        if clipboard_context.has(ContentFormat::Rtf) {
            let text_context = clipboard_context
                .get_rich_text()
                .map_err(|err| err.to_string());
            if let Ok(content) = text_context {
                println!("复制了富文本内容:{}", content);
            }
        }
        if clipboard_context.has(ContentFormat::Html) {
            let text_context = clipboard_context.get_html().map_err(|err| err.to_string());
            if let Ok(content) = text_context {
                println!("复制了html内容:{}", content);
            }
        }
        if clipboard_context.has(ContentFormat::Files) {
            let text_context = clipboard_context.get_files().map_err(|err| err.to_string());
            if let Ok(content) = text_context {
                println!("复制了文件:{:?}", content);
            }
        }
    }
}

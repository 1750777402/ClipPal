use clipboard_listener::{ClipBoardEventListener, ClipType, ClipboardEvent, EventManager};
use clipboard_rs::{
    common::RustImage, Clipboard as ClipboardRS, ClipboardContext as ClipboardRsContext,
    ClipboardHandler, ClipboardWatcher, ClipboardWatcherContext, ContentFormat, WatcherShutdown,
};
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};

pub fn init() -> crate::Result<ClipboardPal> {
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
    pub fn start_monitor(&self, manager: Arc<EventManager<ClipboardEvent>>) -> Result<(), String> {
        let clipboard = ClipboardMonitor::new(self.clipboard.clone(), manager);
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
    pub manager: Arc<EventManager<ClipboardEvent>>,
    pub clipboard: Arc<Mutex<ClipboardRsContext>>,
}

impl ClipboardMonitor {
    pub fn new(
        clipboard: Arc<Mutex<ClipboardRsContext>>,
        manager: Arc<EventManager<ClipboardEvent>>,
    ) -> Self {
        Self { clipboard, manager }
    }
}

impl ClipboardHandler for ClipboardMonitor {
    fn on_clipboard_change(&mut self) {
        let clipboard_context = self
            .clipboard
            .lock()
            .map_err(|err| err.to_string())
            .unwrap();
        // 先判断是不是图片   这里的图片特指PNG
        if clipboard_context.has(ContentFormat::Image) {
            let text_context = clipboard_context.get_image().map_err(|err| err.to_string());
            if let Ok(image) = text_context {
                if let Ok(png) = image.to_png() {
                    self.manager.emit(ClipboardEvent {
                        r#type: ClipType::Img,
                        content: "".to_string(),
                        file: png.get_bytes().to_vec(),
                    });
                } else {
                    self.manager.emit(ClipboardEvent {
                        r#type: ClipType::Unknown,
                        content: "".to_string(),
                        file: vec![],
                    });
                }
            }
        }
        // 再判断是不是文件
        if clipboard_context.has(ContentFormat::Files) {
            let text_context = clipboard_context.get_files().map_err(|err| err.to_string());
            if let Ok(content) = text_context {
                println!("复制了文件:{:?}", content);
            }
        }
        // 文件类型的就判断完了

        // 再判断是不是富文本内容
        if clipboard_context.has(ContentFormat::Rtf) {
            let text_context = clipboard_context
                .get_rich_text()
                .map_err(|err| err.to_string());
            if let Ok(content) = text_context {
                println!("复制了富文本内容:{}", content);
            }
        }
        // 再判断是不是html
        if clipboard_context.has(ContentFormat::Html) {
            let text_context = clipboard_context.get_html().map_err(|err| err.to_string());
            if let Ok(content) = text_context {
                println!("复制了html内容:{}", content);
            }
        }
        // 最后判断是不是普通文本
        if clipboard_context.has(ContentFormat::Text) {
            let text_context = clipboard_context.get_text().map_err(|err| err.to_string());
            if let Ok(text) = text_context {
                // self.manager.emit(data);
                println!("复制了文本{:?}", text);
            }
        }
    }
}

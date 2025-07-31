use base64::{Engine, engine::general_purpose};
use clipboard_listener::{ClipType, ClipboardEvent, EventManager};
use clipboard_rs::{
    Clipboard as ClipboardRS, ClipboardContent, ClipboardContext as ClipboardRsContext,
    ClipboardHandler, ClipboardWatcher, ClipboardWatcherContext, ContentFormat, RustImageData,
    WatcherShutdown, common::RustImage,
};
use image::EncodableLayout;
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};

pub fn init() -> crate::Result<ClipboardPal> {
    let clipboard_context = ClipboardRsContext::new().map_err(|e| {
        crate::Error::Io(std::io::Error::new(
            std::io::ErrorKind::Other,
            format!("Failed to create clipboard context: {}", e),
        ))
    })?;

    Ok(ClipboardPal {
        clipboard: Arc::new(Mutex::new(clipboard_context)),
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
    /// Write files uris to clipboard. The files should be in uri format: `file:///path/to/file` on Mac and Linux. File path is absolute path.
    /// On Windows, the path should be in the format `C:\\path\\to\\file`.
    pub fn write_files_uris(&self, files: Vec<String>) -> Result<(), String> {
        // iterate through files, check if it starts with files://, if not throw error (only linux and mac)
        #[cfg(any(target_os = "linux", target_os = "macos"))]
        {
            for file in &files {
                if !file.starts_with("file://") {
                    return Err(format!(
                        "Invalid file uri: {}. File uri should start with file://",
                        file
                    ));
                }
            }
        }
        // On Windows, we don't need the file:// prefix, so we remove it if it's there
        #[cfg(target_os = "windows")]
        {
            for file in &files {
                if file.starts_with("file://") {
                    return Err(format!(
                        "Invalid file uri: {}. File uri on Windows should not start with file://",
                        file
                    ));
                }
            }
        }

        self.clipboard
            .lock()
            .map_err(|err| err.to_string())?
            .set_files(files)
            .map_err(|err| err.to_string())
    }

    // Write to Clipboard APIs
    pub fn write_text(&self, text: String) -> Result<(), String> {
        self.clipboard
            .lock()
            .map_err(|err| err.to_string())?
            .set_text(text)
            .map_err(|err| err.to_string())
    }

    pub fn write_html(&self, html: String) -> Result<(), String> {
        self.clipboard
            .lock()
            .map_err(|err| err.to_string())?
            .set_html(html)
            .map_err(|err| err.to_string())
    }

    pub fn write_html_and_text(&self, html: String, text: String) -> Result<(), String> {
        self.clipboard
            .lock()
            .map_err(|err| err.to_string())?
            .set(vec![
                ClipboardContent::Text(text),
                ClipboardContent::Html(html),
            ])
            .map_err(|err| err.to_string())
    }

    pub fn write_rtf(&self, rtf: String) -> Result<(), String> {
        self.clipboard
            .lock()
            .map_err(|err| err.to_string())?
            .set_rich_text(rtf)
            .map_err(|err| err.to_string())
    }

    /// write base64 png image to clipboard
    pub fn write_image_base64(&self, base64_image: String) -> Result<(), String> {
        let decoded = general_purpose::STANDARD
            .decode(base64_image)
            .map_err(|err| err.to_string())?;
        self.write_image_binary(decoded)
            .map_err(|err| err.to_string())?;
        Ok(())
    }

    pub fn write_image_binary(&self, bytes: Vec<u8>) -> Result<(), String> {
        let img = RustImageData::from_bytes(bytes.as_bytes()).map_err(|err| err.to_string())?;
        self.clipboard
            .lock()
            .map_err(|err| err.to_string())?
            .set_image(img)
            .map_err(|err| err.to_string())?;
        Ok(())
    }

    pub fn start_monitor(&self, manager: Arc<EventManager<ClipboardEvent>>) -> Result<(), String> {
        let clipboard = ClipboardMonitor::new(self.clipboard.clone(), manager);
        let mut watcher = ClipboardWatcherContext::new()
            .map_err(|e| format!("Failed to create clipboard watcher: {}", e))?;
        let watcher_shutdown = watcher.add_handler(clipboard).get_shutdown_channel();
        let mut watcher_shutdown_state = self
            .watcher_shutdown
            .lock()
            .map_err(|e| format!("Failed to acquire lock: {}", e))?;
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
        let mut watcher_shutdown_state = self
            .watcher_shutdown
            .lock()
            .map_err(|e| format!("Failed to acquire lock: {}", e))?;
        if let Some(watcher_shutdown) = (*watcher_shutdown_state).take() {
            watcher_shutdown.stop();
        }
        *watcher_shutdown_state = None;
        Ok(())
    }

    pub fn is_monitor_running(&self) -> bool {
        self.watcher_shutdown
            .lock()
            .map(|guard| guard.is_some())
            .unwrap_or(false)
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
        let clipboard_context = match self.clipboard.lock() {
            Ok(context) => context,
            Err(e) => {
                log::error!("Failed to acquire clipboard lock: {}", e);
                return;
            }
        };

        // 先判断是不是图片   不管clipboard_context.get_image()得到的是什么类型的图片，统一使用image.to_png()转为png格式
        // 其实大多数情况是针对截图软件的截图功能，截图软件截取的图片是没有形成实际的图片文件的，只有图片二进制数据
        if clipboard_context.has(ContentFormat::Image) {
            let img_context = clipboard_context.get_image().map_err(|err| err.to_string());
            if let Ok(image) = img_context {
                if let Ok(png) = image.to_png() {
                    self.manager.emit(ClipboardEvent {
                        r#type: ClipType::Image,
                        content: "".to_string(),
                        file: Some(png.get_bytes().to_vec()),
                        file_path_vec: None,
                    });
                }
                return;
            }
        }
        // 再判断是不是文件   这个文件包含了各种类型的文件，比如图片、视频、文件夹等等，是实际存在于我们硬盘中的文件
        if clipboard_context.has(ContentFormat::Files) {
            let file_context = clipboard_context.get_files().map_err(|err| err.to_string());
            if let Ok(content) = file_context {
                self.manager.emit(ClipboardEvent {
                    r#type: ClipType::File,
                    content: "".to_string(),
                    file: None,
                    file_path_vec: Some(content),
                });
                return;
            }
        }
        // 文件类型的就判断完了

        // 再判断是不是富文本内容
        // if clipboard_context.has(ContentFormat::Rtf) {
        //     let text_context = clipboard_context
        //         .get_rich_text()
        //         .map_err(|err| err.to_string());
        //     if let Ok(content) = text_context {
        //         self.manager.emit(ClipboardEvent {
        //             r#type: ClipType::Rtf,
        //             content: content,
        //             file: None,
        //         });
        //         return;
        //     }
        // }
        // // 再判断是不是html
        // if clipboard_context.has(ContentFormat::Html) {
        //     let text_context = clipboard_context.get_html().map_err(|err| err.to_string());
        //     if let Ok(content) = text_context {
        //         self.manager.emit(ClipboardEvent {
        //             r#type: ClipType::Html,
        //             content: content,
        //             file: None,
        //         });
        //         return;
        //     }
        // }
        // 最后判断是不是普通文本
        if clipboard_context.has(ContentFormat::Text) {
            let text_context = clipboard_context.get_text().map_err(|err| err.to_string());
            if let Ok(text) = text_context {
                self.manager.emit(ClipboardEvent {
                    r#type: ClipType::Text,
                    content: text,
                    file: None,
                    file_path_vec: None,
                });
                return;
            }
        }
    }
}

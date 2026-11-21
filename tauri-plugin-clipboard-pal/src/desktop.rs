use base64::{engine::general_purpose, Engine};
use clipboard_listener::{ClipType, ClipboardEvent, EventManager};
use clipboard_rs::{
    common::RustImage, Clipboard as ClipboardRS, ClipboardContent,
    ClipboardContext as ClipboardRsContext, ClipboardHandler, ClipboardWatcher,
    ClipboardWatcherContext, ContentFormat, RustImageData, WatcherShutdown,
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

    let retained_files = clipboard_context.get_files().ok()
        .and_then(|files| crate::file_paths::local_paths(files).ok()).unwrap_or_default();
    Ok(ClipboardPal {
        clipboard: Arc::new(Mutex::new(clipboard_context)),
        watcher_shutdown: Arc::default(),
        retained_files: Arc::new(Mutex::new(retained_files)),
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
    retained_files: Arc<Mutex<Vec<String>>>,
}

impl ClipboardPal {
    /// 接受本地绝对路径或 file URI，由适配器转换为平台要求的格式。
    pub fn write_files_uris(&self, files: Vec<String>) -> Result<(), String> {
        let local = crate::file_paths::local_paths(files)?;
        let files = crate::file_paths::clipboard_paths(local.clone())?;
        let clipboard = self.clipboard.lock().map_err(|err| err.to_string())?;
        let mut retained = self.retained_files.lock().map_err(|err| err.to_string())?;
        if let Err(error) = clipboard.set_files(files) {
            crate::file_paths::remove_inactive_temp_dirs(&local, &retained);
            return Err(error.to_string());
        }
        crate::file_paths::remove_inactive_temp_dirs(&retained, &local);
        *retained = local;
        Ok(())
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
        let clipboard = ClipboardMonitor::new(self.clipboard.clone(), manager, self.retained_files.clone());
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
    retained_files: Arc<Mutex<Vec<String>>>,
}

impl ClipboardMonitor {
    pub fn new(
        clipboard: Arc<Mutex<ClipboardRsContext>>,
        manager: Arc<EventManager<ClipboardEvent>>,
        retained_files: Arc<Mutex<Vec<String>>>,
    ) -> Self {
        Self { clipboard, manager, retained_files }
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

        let snapshot = (|| -> Option<ClipboardEvent> {
            // 优先判断文件，Finder 可能同时提供文件路径和预览图片。
            // 判断是不是文件   这个文件包含了各种类型的文件，比如图片、视频、文件夹等等，是实际存在于我们硬盘中的文件
            if clipboard_context.has(ContentFormat::Files) {
                let file_context = clipboard_context.get_files().map_err(|err| err.to_string());
                if let Ok(content) = file_context.and_then(crate::file_paths::local_paths) {
                    return Some(ClipboardEvent {
                        r#type: ClipType::File,
                        content: "".to_string(),
                        file: None,
                        file_path_vec: Some(content),
                    });
                }
            }
            // 再判断是不是图片   不管clipboard_context.get_image()得到的是什么类型的图片，统一使用image.to_png()转为png格式
            // 其实大多数情况是针对截图软件的截图功能，截图软件截取的图片是没有形成实际的图片文件的，只有图片二进制数据
            if clipboard_context.has(ContentFormat::Image) {
                let img_context = clipboard_context.get_image().map_err(|err| err.to_string());
                if let Ok(image) = img_context {
                    if let Ok(png) = image.to_png() {
                        return Some(ClipboardEvent {
                            r#type: ClipType::Image,
                            content: "".to_string(),
                            file: Some(png.get_bytes().to_vec()),
                            file_path_vec: None,
                        });
                    }
                    return None;
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
                    return Some(ClipboardEvent {
                        r#type: ClipType::Text,
                        content: text,
                        file: None,
                        file_path_vec: None,
                    });
                }
            }
            None
        })();
        // 剪贴板内容替换后才释放旧临时文件，当前文件不会因固定超时而失效。
        if let Ok(mut retained) = self.retained_files.lock() {
            let current = clipboard_context.get_files().ok()
                .and_then(|files| crate::file_paths::local_paths(files).ok()).unwrap_or_default();
            crate::file_paths::remove_inactive_temp_dirs(&retained, &current);
            *retained = current;
        }
        // 先释放系统剪贴板锁，再等待有界事件队列，避免复制操作相互阻塞。
        drop(clipboard_context);
        if let Some(event) = snapshot { self.manager.emit(event); }
    }
}

#[cfg(test)]
mod native_tests {
    use super::*;
    use std::path::PathBuf;

    struct RestoreClipboard {
        clipboard: Arc<Mutex<ClipboardRsContext>>,
        snapshot: Option<Vec<ClipboardContent>>,
        fixture_dir: PathBuf,
    }
    impl Drop for RestoreClipboard {
        fn drop(&mut self) {
            if let Some(snapshot) = self.snapshot.take() {
                if let Ok(clipboard) = self.clipboard.lock() { let _ = clipboard.set(snapshot); }
            }
            let _ = std::fs::remove_dir_all(&self.fixture_dir);
        }
    }

    /// 显式运行的原生集成验证；常规测试不读取或修改系统剪贴板。
    #[test]
    #[ignore = "需要原生桌面会话，临时写入系统剪贴板并恢复原内容"]
    fn native_clipboard_roundtrip_preserves_files_and_prioritizes_them_over_previews() {
        let clipboard = init().unwrap();
        // 测试不得回收测试开始前的临时副本，保证最终可恢复原文件引用。
        clipboard.retained_files.lock().unwrap().clear();
        let snapshot = {
            let system = clipboard.clipboard.lock().unwrap();
            let selected = [ContentFormat::Text, ContentFormat::Files, ContentFormat::Image, ContentFormat::Html, ContentFormat::Rtf]
                .into_iter().filter(|format| system.has(format.clone())).collect::<Vec<_>>();
            system.get(&selected).unwrap()
        };
        let dir = std::env::temp_dir().join(format!("clippal-native-test-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let _restore = RestoreClipboard { clipboard: clipboard.clipboard.clone(), snapshot: Some(snapshot), fixture_dir: dir.clone() };
        let file = dir.join("中文 空格%#文件.txt");
        std::fs::write(&file, "native clipboard fixture").unwrap();
        let native = file.to_string_lossy().into_owned();
        let uri = url::Url::from_file_path(&file).unwrap().to_string();
        clipboard.write_files_uris(vec![uri]).unwrap();
        let returned = clipboard.clipboard.lock().unwrap().get_files().unwrap();
        assert_eq!(crate::file_paths::local_paths(returned).unwrap(), vec![native.clone()]);

        let mut png = std::io::Cursor::new(Vec::new());
        image::DynamicImage::new_rgba8(2, 2).write_to(&mut png, image::ImageFormat::Png).unwrap();
        let preview = RustImageData::from_bytes(png.get_ref()).unwrap();
        clipboard.clipboard.lock().unwrap().set(vec![ClipboardContent::Files(vec![native]), ClipboardContent::Image(preview)]).unwrap();
        let manager = Arc::new(EventManager::default());
        let events = manager.subscribe();
        let mut monitor = ClipboardMonitor::new(clipboard.clipboard.clone(), manager, clipboard.retained_files.clone());
        monitor.on_clipboard_change();
        let event = events.recv_blocking().unwrap();
        assert!(matches!(event.r#type, ClipType::File));
        assert_eq!(event.file_path_vec.unwrap().len(), 1);

        clipboard.write_text("ClipPal 本地文本🙂".into()).unwrap();
        monitor.on_clipboard_change();
        let event = events.recv_blocking().unwrap();
        assert!(matches!(event.r#type, ClipType::Text));
        assert_eq!(event.content, "ClipPal 本地文本🙂");
        clipboard.write_image_binary(png.into_inner()).unwrap();
        monitor.on_clipboard_change();
        let event = events.recv_blocking().unwrap();
        assert!(matches!(event.r#type, ClipType::Image));
        assert!(!event.file.unwrap().is_empty());
    }
}

use std::sync::{Arc, Mutex};

use serde::{Deserialize, Serialize};

// 事件监听器Trait
pub trait ClipBoardEventListener<T>: Send + Sync {
    fn handle_event(&self, event_data: &T);
}

// 线程安全的事件管理器
#[derive(Default)]
pub struct EventManager<T> {
    listeners: Arc<Mutex<Vec<Arc<dyn ClipBoardEventListener<T>>>>>, // 单一监听器列表
}

impl<T> EventManager<T>
where
    T: Clone + Send + 'static,
{
    // 注册监听器
    pub fn subscribe(&self, listener: Arc<dyn ClipBoardEventListener<T>>) {
        self.listeners.lock().unwrap().push(listener);
    }

    // 触发事件
    pub fn emit(&self, data: T) {
        let listeners = self.listeners.lock().unwrap();
        for listener in listeners.iter() {
            listener.handle_event(&data);
        }
    }

    // 异步触发（需Tokio运行时）
    #[cfg(feature = "async")]
    pub async fn emit_async(&self, data: T) {
        let listeners = self.listeners.lock().unwrap().clone();
        tokio::spawn(async move {
            for listener in listeners {
                listener.handle_event(&data);
            }
        });
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub enum ClipType {
    Text,
    Img,
    File,
    Rtf,
    Html,
    #[default]
    Unknown,
}

#[derive(Debug, Clone, Default)]
pub struct ClipboardEvent {
    // 类型
    pub r#type: ClipType,
    // 内容
    pub content: String,
    // 文件内容
    pub file: Vec<u8>,
}

#[derive(Debug, Clone)]
pub struct ClipboardEventTigger;
impl ClipBoardEventListener<ClipboardEvent> for ClipboardEventTigger {
    fn handle_event(&self, event: &ClipboardEvent) {
        dbg!(event);
    }
}

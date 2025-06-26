use log::error;
use std::{
    fmt,
    str::FromStr,
    sync::{Arc, RwLock},
};

use async_channel::{Receiver, Sender};
use serde::{Deserialize, Serialize};
use tokio::signal;

// 事件监听器Trait
#[async_trait::async_trait]
pub trait ClipBoardEventListener<T>: Send + Sync {
    async fn handle_event(&self, event_data: &T);
}

// 线程安全的事件管理器

pub struct EventManager<T> {
    tx: Sender<T>,
    rt: Receiver<T>,
    listeners: Arc<RwLock<Vec<Arc<dyn ClipBoardEventListener<T>>>>>, // 单一监听器列表
    pub shutdown: (Sender<()>, Receiver<()>),
}

impl<T> EventManager<T>
where
    T: Clone + Send + 'static,
{
    pub fn default() -> Self {
        let (tx, rt) = async_channel::bounded(100);
        let (shutdown_tx, shutdown_rt) = async_channel::bounded(1);
        Self {
            tx,
            rt,
            listeners: Default::default(),
            shutdown: (shutdown_tx, shutdown_rt),
        }
    }
    pub fn add_event_listener(&self, event_listener: Arc<dyn ClipBoardEventListener<T>>) {
        match self.listeners.write() {
            Ok(mut write) => {
                write.push(event_listener);
            }
            Err(e) => {
                log::error!("添加事件监听器失败: {}", e);
            }
        }
    }
    // 注册监听器
    pub fn subscribe(&self) -> Receiver<T> {
        self.rt.clone()
    }

    // 触发事件
    pub fn emit(&self, data: T) {
        let _ = self.tx.send_blocking(data);
    }
    pub fn start_event_loop(&self) {
        let rx: async_channel::Receiver<T> = self.subscribe();
        let listeners = self.listeners.clone();
        let shutdown_rt = self.shutdown.1.clone();
        tokio::spawn(async move {
            let mut join_set = tokio::task::JoinSet::new();
            loop {
                tokio::select! {
                    event = rx.recv() => match event {
                        Ok(event) => {
                            // 并发处理所有handler
                            match listeners.read() {
                                Ok(readers) => {
                                    let handlers_clone = readers.clone();
                                    join_set.spawn(async move {
                                        // 并发处理所有handler
                                        for handler in &handlers_clone {
                                            handler.handle_event(&event).await;
                                        }
                                    });
                                }
                                Err(e) => {
                                    log::error!("获取事件监听器读锁失败: {}", e);
                                }
                            }
                        },
                        Err(e) => {
                            error!("rx.recv Error: {}", e);
                            break;
                        },
                    },
                    _ = shutdown_rt.recv() => {
                        let _ = join_set.shutdown().await;
                        break;
                    },
                    shutdown_signal = Box::pin(signal::ctrl_c()) => {
                        match shutdown_signal {
                            Ok(()) => {
                                let _ = join_set.shutdown().await;
                                break;
                            },
                            Err(e) => {
                                error!("shutdown_signal Error: {}", e);
                                let _ = join_set.shutdown().await;
                                break;
                            }
                        }
                    },
                }
            }
        });
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub enum ClipType {
    Text,
    Image,
    File,
    Rtf,
    Html,
    #[default]
    Unknown,
}

// 实现枚举值到字符串的转换
impl fmt::Display for ClipType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            ClipType::Text => "Text",
            ClipType::Image => "Image",
            ClipType::File => "File",
            ClipType::Rtf => "Rtf",
            ClipType::Html => "Html",
            ClipType::Unknown => "Unknown",
        };
        write!(f, "{}", s)
    }
}

// 实现字符串到枚举值的转换
impl FromStr for ClipType {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "Text" => Ok(ClipType::Text),
            "Image" => Ok(ClipType::Image),
            "File" => Ok(ClipType::File),
            "Rtf" => Ok(ClipType::Rtf),
            "Html" => Ok(ClipType::Html),
            _ => Ok(ClipType::Unknown),
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct ClipboardEvent {
    // 类型
    pub r#type: ClipType,
    // 内容  文本类型使用
    pub content: String,
    // 文件内容  png截图类型图片使用
    pub file: Option<Vec<u8>>,
    // 文件路径   文件类型使用
    pub file_path_vec: Option<Vec<String>>,
}

use std::sync::{Arc, Mutex, RwLock};

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
        let mut write = self.listeners.write().unwrap();
        write.push(event_listener);
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
                            let handlers_clone = listeners.read().unwrap().clone();
                            join_set.spawn(async move {
                                // 并发处理所有handler
                                for handler in &handlers_clone {
                                    let _ = handler.handle_event(&event).await;
                                }
                            });
                        },
                        Err(e) => {
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
#[async_trait::async_trait]
impl ClipBoardEventListener<ClipboardEvent> for ClipboardEventTigger {
    async fn handle_event(&self, event: &ClipboardEvent) {
        println!("触发了粘贴板监听器:{}", event.content);
    }
}

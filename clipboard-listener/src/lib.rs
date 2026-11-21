use std::{
    fmt,
    str::FromStr,
    sync::{Arc, RwLock, atomic::{AtomicBool, Ordering}},
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
    shutdown: (Sender<()>, Receiver<()>),
    running: Arc<AtomicBool>,
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
            running: Arc::new(AtomicBool::new(false)),
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
    /// 退出请求不阻塞桌面事件线程，多次请求可合并。
    pub fn request_shutdown(&self) {
        let _ = self.shutdown.0.try_send(());
    }

    pub fn start_event_loop(&self) {
        if self.running.swap(true, Ordering::AcqRel) { return; }
        let rx = self.subscribe();
        let listeners = self.listeners.clone();
        let shutdown = self.shutdown.1.clone();
        let running = self.running.clone();
        tokio::spawn(async move {
            struct RunningGuard(Arc<AtomicBool>);
            impl Drop for RunningGuard {
                fn drop(&mut self) { self.0.store(false, Ordering::Release); }
            }
            let _guard = RunningGuard(running);
            let mut handlers = tokio::task::JoinSet::new();
            let consume = async {
                while let Ok(event) = rx.recv().await {
                    let snapshot = match listeners.read() {
                        Ok(readers) => readers.clone(),
                        Err(error) => {
                            log::error!("获取事件监听器读锁失败: {}", error);
                            continue;
                        }
                    };
                    // 按采集顺序处理事件，每次回收任务，避免已完成任务和大图片持续积压。
                    handlers.spawn(async move {
                        for handler in snapshot { handler.handle_event(&event).await; }
                    });
                    if let Some(Err(error)) = handlers.join_next().await {
                        log::error!("剪贴板事件处理任务异常: {}", error);
                    }
                }
            };
            tokio::select! {
                _ = consume => {},
                _ = shutdown.recv() => {},
                result = signal::ctrl_c() => {
                    if let Err(error) = result { log::error!("监听退出信号失败: {}", error); }
                }
            }
            handlers.shutdown().await;
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;
    use tokio::{sync::Notify, time::{timeout, Duration}};
    struct Recorder { values: Arc<Mutex<Vec<u32>>>, finished: Arc<Notify> }
    #[async_trait::async_trait]
    impl ClipBoardEventListener<u32> for Recorder {
        async fn handle_event(&self, value: &u32) {
            tokio::task::yield_now().await;
            self.values.lock().unwrap().push(*value);
            self.finished.notify_one();
        }
    }
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn processes_bursts_once_in_order_and_shutdown_is_nonblocking() {
        let manager = Arc::new(EventManager::default());
        let values = Arc::new(Mutex::new(Vec::new()));
        let finished = Arc::new(Notify::new());
        manager.add_event_listener(Arc::new(Recorder { values: values.clone(), finished: finished.clone() }));
        manager.start_event_loop();
        manager.start_event_loop();
        let sender = manager.clone();
        let producer = tokio::task::spawn_blocking(move || {
            for value in 0..300 { sender.emit(value); }
        });
        timeout(Duration::from_secs(5), async {
            loop {
                if values.lock().unwrap().len() == 300 { break; }
                finished.notified().await;
            }
        }).await.unwrap();
        producer.await.unwrap();
        assert_eq!(*values.lock().unwrap(), (0..300).collect::<Vec<_>>());
        manager.request_shutdown();
        manager.request_shutdown();
        timeout(Duration::from_secs(1), async {
            while manager.running.load(Ordering::Acquire) { tokio::task::yield_now().await; }
        }).await.unwrap();
    }
}

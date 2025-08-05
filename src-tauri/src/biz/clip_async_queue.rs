#![allow(dead_code)]

use async_channel::{Receiver, Sender, TryRecvError, bounded};
use rbatis::RBatis;
use std::sync::Arc;
use tauri::{AppHandle, Emitter};
use tokio::task;
use tokio::time::{Duration, sleep};

use crate::CONTEXT;
use crate::api::cloud_sync_api::{SingleCloudSyncParam, sync_single_clip_record};
use crate::biz::clip_record::ClipRecord;
use crate::errors::AppError;
use crate::utils::lock_utils::GlobalSyncLock;

#[derive(Clone, Debug)]
pub enum QueueEvent<T> {
    Add(T),
    Delete(T),
}

#[derive(Clone)]
pub struct AsyncQueue<T> {
    sender: Arc<Sender<QueueEvent<T>>>,
    receiver: Arc<Receiver<QueueEvent<T>>>,
}

impl<T: Clone + Send + 'static> AsyncQueue<T> {
    pub fn new(capacity: usize) -> Self {
        let (sender, receiver) = bounded(capacity);
        AsyncQueue {
            sender: Arc::new(sender),
            receiver: Arc::new(receiver),
        }
    }

    pub async fn send_add(&self, item: T) -> Result<(), async_channel::SendError<QueueEvent<T>>> {
        self.sender.send(QueueEvent::Add(item)).await
    }

    pub async fn send_delete(
        &self,
        item: T,
    ) -> Result<(), async_channel::SendError<QueueEvent<T>>> {
        self.sender.send(QueueEvent::Delete(item)).await
    }

    pub async fn recv(&self) -> Result<QueueEvent<T>, async_channel::RecvError> {
        self.receiver.recv().await
    }

    pub fn try_recv(&self) -> Result<QueueEvent<T>, TryRecvError> {
        self.receiver.try_recv()
    }

    pub fn capacity(&self) -> Option<usize> {
        self.sender.capacity()
    }

    pub fn len(&self) -> usize {
        self.sender.len()
    }

    pub fn is_empty(&self) -> bool {
        self.sender.is_empty()
    }

    pub fn is_full(&self) -> bool {
        self.sender.is_full()
    }
}

pub fn consume_clip_record_queue(queue: AsyncQueue<ClipRecord>) {
    task::spawn(async move {
        let sync_lock: &GlobalSyncLock = CONTEXT.get::<GlobalSyncLock>();

        loop {
            // 先尝试拿锁，拿不到就等待一会儿再重试
            if let Some(_guard) = sync_lock.try_lock() {
                log::debug!("获取到同步锁，开始处理队列数据...");

                // 循环接收并处理队列数据
                loop {
                    match queue.try_recv() {
                        Ok(event) => {
                            // 处理数据
                            let param = match event {
                                QueueEvent::Add(item) => SingleCloudSyncParam {
                                    r#type: 1,
                                    clip: item.clone(),
                                },
                                QueueEvent::Delete(item) => SingleCloudSyncParam {
                                    r#type: 2,
                                    clip: item.clone(),
                                },
                            };
                            handle_sync_inner(param).await;
                        }
                        Err(TryRecvError::Empty) => {
                            // 队列空了，跳出内层循环，释放锁
                            log::debug!("队列已空，释放锁，等待下一轮处理");
                            break;
                        }
                        Err(e) => {
                            log::error!("接收队列消息错误: {}", e);
                            break;
                        }
                    }
                }
            } else {
                // 锁被占用，短暂休眠避免忙等
                log::debug!("同步锁被占用，等待后重试...");
            }
            sleep(Duration::from_millis(500)).await;
        }
    });
}

async fn handle_sync_inner(param: SingleCloudSyncParam) {
    let res = sync_single_clip_record(&param).await;
    log::info!(
        "同步单个剪贴板记录，粘贴记录：{}，结果：{:?}",
        param.clip.md5_str,
        res
    );
    match res {
        Ok(response) => {
            if let Some(success) = response {
                let ids = vec![param.clip.id];
                let rb: &RBatis = CONTEXT.get::<RBatis>();
                let _ = ClipRecord::update_sync_flag(rb, &ids, 2, success.timestamp);
                let payload = serde_json::json!({
                    "clip_ids": ids,
                    "sync_flag": 2
                });
                let app_handle = CONTEXT.get::<AppHandle>();
                let _ = app_handle
                    .emit("sync_status_update_batch", payload)
                    .map_err(|e| AppError::General(format!("批量通知前端失败: {}", e)));
            }
        }
        Err(e) => log::error!(
            "同步单个剪贴板记录失败，粘贴记录：{}，错误：{}",
            param.clip.id,
            e
        ),
    }
}

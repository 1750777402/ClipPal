use async_channel::{Receiver, Sender, TryRecvError, bounded};
use rbatis::RBatis;
use std::fmt::Debug;
use std::sync::Arc;
use tokio::task;
use tokio::time::{Duration, sleep};

use crate::CONTEXT;
use crate::api::cloud_sync_api::{SingleCloudSyncParam, sync_single_clip_record};
use crate::biz::clip_record::ClipRecord;
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
            // 优先尝试获取锁
            if let Some(_guard) = sync_lock.try_lock() {
                log::debug!("获取到同步锁，准备从队列接收数据...");

                match queue.recv().await {
                    Ok(QueueEvent::Add(item)) => {
                        let param = SingleCloudSyncParam {
                            r#type: 1,
                            clip: item.clone(),
                        };
                        handle_sync_inner(param).await;
                    }
                    Ok(QueueEvent::Delete(item)) => {
                        let param = SingleCloudSyncParam {
                            r#type: 2,
                            clip: item.clone(),
                        };
                        handle_sync_inner(param).await;
                    }
                    Err(e) => {
                        log::error!("接收async_queue消息错误: {}", e);
                    }
                }
            } else {
                log::debug!("当前有同步任务在进行，等待中...");
                sleep(Duration::from_millis(100)).await;
            }
        }
    });
}

async fn handle_sync_inner(param: SingleCloudSyncParam) {
    let res = sync_single_clip_record(&param).await;
    match res {
        Ok(response) => {
            if let Some(success) = response {
                let rb: &RBatis = CONTEXT.get::<RBatis>();
                let _ =
                    ClipRecord::update_sync_flag(rb, &vec![param.clip.id], 2, success.timestamp);
            }
        }
        Err(e) => log::error!(
            "同步单个剪贴板记录失败，粘贴记录：{}，错误：{}",
            param.clip.id,
            e
        ),
    }
}

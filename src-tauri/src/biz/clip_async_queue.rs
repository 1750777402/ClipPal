#![allow(dead_code)]

use async_channel::{Receiver, Sender, TryRecvError, bounded};
use std::fmt::Debug;
use std::sync::Arc;
use tokio::task;

use crate::api::cloud_sync_api::{SingleCloudSyncParam, sync_single_clip_record};
use crate::biz::clip_record::ClipRecord;

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
    /// 创建一个指定容量的队列
    pub fn new(capacity: usize) -> Self {
        let (sender, receiver) = bounded(capacity);
        AsyncQueue {
            sender: Arc::new(sender),
            receiver: Arc::new(receiver),
        }
    }

    /// 发送 Add 事件
    pub async fn send_add(&self, item: T) -> Result<(), async_channel::SendError<QueueEvent<T>>> {
        self.sender.send(QueueEvent::Add(item)).await
    }

    /// 发送 Delete 事件
    pub async fn send_delete(
        &self,
        item: T,
    ) -> Result<(), async_channel::SendError<QueueEvent<T>>> {
        self.sender.send(QueueEvent::Delete(item)).await
    }

    /// 异步阻塞接收
    pub async fn recv(&self) -> Result<QueueEvent<T>, async_channel::RecvError> {
        self.receiver.recv().await
    }

    /// 非阻塞尝试接收
    pub fn try_recv(&self) -> Result<QueueEvent<T>, TryRecvError> {
        self.receiver.try_recv()
    }

    /// 获取剩余容量
    pub fn capacity(&self) -> Option<usize> {
        self.sender.capacity()
    }

    /// 获取当前队列长度
    pub fn len(&self) -> usize {
        self.sender.len()
    }

    /// 是否为空
    pub fn is_empty(&self) -> bool {
        self.sender.is_empty()
    }

    /// 是否已满
    pub fn is_full(&self) -> bool {
        self.sender.is_full()
    }
}

pub fn consume_clip_record_queue(queue: AsyncQueue<ClipRecord>) {
    task::spawn(async move {
        loop {
            match queue.recv().await {
                Ok(QueueEvent::Add(item)) => {
                    // 处理添加逻辑
                    let param = SingleCloudSyncParam {
                        r#type: 1,
                        clip: item.clone(),
                    };
                    let res = sync_single_clip_record(&param).await;
                    if let Err(e) = res {
                        log::error!(
                            "同步新增单个剪贴板记录失败，粘贴记录：{}，错误：{}",
                            item.id,
                            e
                        );
                    }
                }
                Ok(QueueEvent::Delete(item)) => {
                    // 处理删除逻辑
                    let param = SingleCloudSyncParam {
                        r#type: 2,
                        clip: item.clone(),
                    };
                    let res = sync_single_clip_record(&param).await;
                    if let Err(e) = res {
                        log::error!(
                            "同步删除单个剪贴板记录失败，粘贴记录：{}，错误：{}",
                            item.id,
                            e
                        );
                    }
                }
                Err(e) => {
                    log::error!("接收async_queue消息错误: {}", e);
                }
            }
        }
    });
}

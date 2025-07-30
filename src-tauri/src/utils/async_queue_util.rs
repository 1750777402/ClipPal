use async_channel::{Receiver, Sender, TryRecvError, TrySendError, bounded};
use std::sync::Arc;

#[derive(Clone)]
pub struct AsyncQueue<T> {
    sender: Arc<Sender<T>>,
    receiver: Arc<Receiver<T>>,
}

impl<T> AsyncQueue<T> {
    /// 创建一个指定容量的队列
    pub fn new(capacity: usize) -> Self {
        let (sender, receiver) = bounded(capacity);
        AsyncQueue {
            sender: Arc::new(sender),
            receiver: Arc::new(receiver),
        }
    }

    /// 异步阻塞发送
    pub async fn send(&self, item: T) -> Result<(), async_channel::SendError<T>> {
        self.sender.send(item).await
    }

    /// 异步阻塞接收
    pub async fn recv(&self) -> Result<T, async_channel::RecvError> {
        self.receiver.recv().await
    }

    /// 非阻塞尝试发送
    pub fn try_send(&self, item: T) -> Result<(), TrySendError<T>> {
        self.sender.try_send(item)
    }

    /// 非阻塞尝试接收
    pub fn try_recv(&self) -> Result<T, TryRecvError> {
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

//! 旧版内存同步队列，尚不是持久化 Outbox。
use async_channel::{bounded, Receiver, Sender, TryRecvError};
use std::sync::Arc;

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

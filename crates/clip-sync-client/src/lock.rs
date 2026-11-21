use std::sync::Arc;
use tokio::sync::{Mutex, MutexGuard};

/// 非阻塞互斥锁
pub struct NonblockMutex<T> {
    inner: Arc<Mutex<T>>,
}

impl<T> NonblockMutex<T> {
    pub fn new(val: T) -> Self {
        Self {
            inner: Arc::new(Mutex::new(val)),
        }
    }

    pub fn try_lock(&self) -> Option<MutexGuard<'_, T>> {
        match self.inner.try_lock() {
            Ok(guard) => Some(guard),
            Err(_) => None,
        }
    }

    pub fn is_locked(&self) -> bool {
        self.try_lock().is_none()
    }

    pub fn inner(&self) -> &Arc<Mutex<T>> {
        &self.inner
    }
}

impl<T> Clone for NonblockMutex<T> {
    fn clone(&self) -> Self {
        Self {
            inner: Arc::clone(&self.inner),
        }
    }
}

pub type GlobalSyncLock = NonblockMutex<()>;

pub fn create_global_sync_lock() -> GlobalSyncLock {
    NonblockMutex::new(())
}

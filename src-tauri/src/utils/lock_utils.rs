#![allow(dead_code)]

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

    pub fn try_lock(&self) -> Option<MutexGuard<T>> {
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

// ----------------------------------------------------------------------------------------------------------------------------------------

/// 同步阻塞锁
pub mod lock_utils {
    use std::sync::{Mutex, MutexGuard, RwLock, RwLockReadGuard, RwLockWriteGuard};

    use crate::errors::{AppError, AppResult};

    pub fn safe_lock<T>(mutex: &Mutex<T>) -> AppResult<MutexGuard<T>> {
        mutex
            .lock()
            .map_err(|e| AppError::Lock(format!("无法获取锁: {}", e)))
    }

    pub fn safe_read_lock<T>(rwlock: &RwLock<T>) -> AppResult<RwLockReadGuard<T>> {
        rwlock
            .read()
            .map_err(|e| AppError::Lock(format!("无法获取读锁: {}", e)))
    }

    pub fn safe_write_lock<T>(rwlock: &RwLock<T>) -> AppResult<RwLockWriteGuard<T>> {
        rwlock
            .write()
            .map_err(|e| AppError::Lock(format!("无法获取写锁: {}", e)))
    }
}

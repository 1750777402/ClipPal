//! 上传和下载共用同一个附件队列。并发限制不影响操作日志与补拉页的提交顺序。
use clip_sync_client::{protocol::Attachment, SyncError, TransferProgress};
use serde::Serialize;
use std::{collections::{BTreeMap, HashSet}, sync::{Arc, Mutex}, future::Future};
use tokio::sync::Semaphore;

/// 一个进程同时传输的附件数。上下行共用两个名额。
pub(crate) const CONCURRENT_FILES: usize = 2;

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Direction { Upload, Download }

#[derive(Clone, Copy, Serialize)]
#[serde(rename_all = "snake_case")]
enum State { Queued, Transferring, Retrying, Failed, Transferred }

/// 卡片状态。上传完成以云端确认和本机队列提交为准。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CardSyncStatus { PendingUpload, Uploading, Uploaded, PendingDownload, Downloading }

/// 下载完成前，列表用本地内存里的附件描述显示临时卡片。
#[derive(Clone, Serialize)]
pub struct IncomingAttachment {
    pub content_key: String,
    pub name: String,
    pub created: i64,
    pub status: CardSyncStatus,
}

struct FileTransfer {
    state: State,
    bytes: u64,
    incoming: Option<IncomingAttachment>,
}

#[derive(Default)]
struct TransferState {
    files: BTreeMap<(Direction, String), FileTransfer>,
    submitting: HashSet<String>,
}

#[derive(Clone)]
pub(crate) struct Transfers {
    permits: Arc<Semaphore>,
    state: Arc<Mutex<TransferState>>,
}

impl Default for Transfers {
    fn default() -> Self {
        Self {
            permits: Arc::new(Semaphore::new(CONCURRENT_FILES)),
            state: Arc::new(Mutex::new(TransferState::default())),
        }
    }
}

impl Transfers {
    /// 将数据库里的状态与在途传输合并，附上本页正在下载的记录。
    pub(crate) fn card_states(
        &self, uploads: BTreeMap<String, Option<String>>,
    ) -> (BTreeMap<String, CardSyncStatus>, Vec<IncomingAttachment>) {
        let state = self.state.lock().unwrap_or_else(|error| error.into_inner());
        let cards = uploads.into_iter().map(|(key, operation_id)| {
            let status = if let Some(id) = operation_id {
                let submitting = state.submitting.contains(&id);
                let sending_file = state.files.get(&(Direction::Upload, id))
                    .is_some_and(|file| matches!(file.state, State::Transferring));
                if sending_file || submitting {
                    CardSyncStatus::Uploading
                } else {
                    CardSyncStatus::PendingUpload
                }
            } else {
                CardSyncStatus::Uploaded
            };
            (key, status)
        }).collect();
        let incoming = state.files.values().filter_map(|file| {
            let mut card = file.incoming.clone()?;
            if matches!(file.state, State::Transferring | State::Transferred) {
                card.status = CardSyncStatus::Downloading;
            }
            Some(card)
        }).collect();
        (cards, incoming)
    }

    /// 暂停同步时释放本轮内存中的传输状态。
    pub(crate) fn clear_in_flight(&self) {
        let mut state = self.state.lock().unwrap_or_else(|error| error.into_inner());
        state.files.clear();
        state.submitting.clear();
    }

    /// 页面开始下载前登记附件，等待名额的项目也有自己的卡片状态。
    pub(crate) fn queue_download(&self, id: String, attachment: IncomingAttachment) {
        self.state.lock().unwrap_or_else(|error| error.into_inner()).files.insert(
            (Direction::Download, id),
            FileTransfer { state: State::Queued, bytes: 0, incoming: Some(attachment) },
        );
    }

    /// 补拉页提交后移除临时卡片。下一次重试会重新登记整页。
    pub(crate) fn clear_downloads(&self) {
        self.state.lock().unwrap_or_else(|error| error.into_inner()).files
            .retain(|(direction, _), _| *direction != Direction::Download);
    }

    /// 标记正在通过 HTTP 提交的操作，含文本和已经传完文件本体的附件。
    pub(crate) fn submitting(&self, operations: &[crate::clip_sync_client::protocol::Operation], active: bool) {
        let mut state = self.state.lock().unwrap_or_else(|error| error.into_inner());
        if active {
            state.submitting.extend(operations.iter().map(|operation| operation.operation_id.clone()));
        } else {
            for operation in operations { state.submitting.remove(&operation.operation_id); }
        }
    }

    pub(crate) fn forget_upload(&self, id: &str) {
        self.state.lock().unwrap_or_else(|error| error.into_inner()).files
            .remove(&(Direction::Upload, id.to_owned()));
    }

    #[cfg(test)]
    pub(crate) fn in_progress_count(&self) -> usize {
        self.state.lock().unwrap_or_else(|error| error.into_inner()).files.len()
    }

    /// 等待名额、记录真实字节进度，并在出错后保留状态供界面显示。
    pub(crate) async fn run<T, F, Work>(&self, direction: Direction, id: String, item: &Attachment, work: Work) -> Result<T, SyncError>
    where
        Work: FnOnce(Arc<TransferProgress>) -> F,
        F: Future<Output = Result<T, SyncError>>,
    {
        let key = (direction, id.clone());
        {
            let mut state = self.state.lock().unwrap_or_else(|error| error.into_inner());
            let row = state.files.entry(key.clone()).or_insert(FileTransfer {
                state: State::Queued, bytes: 0, incoming: None,
            });
            row.state = State::Queued;
            row.bytes = 0;
        }
        let _permit = self.permits.acquire().await.expect("共享的附件传输队列不会被关闭");
        self.update(&key, |row| row.state = State::Transferring);
        let transfers = self.clone();
        let progress_key = key.clone();
        let total = item.size as u64;
        let progress: Arc<TransferProgress> = Arc::new(move |bytes| {
            transfers.update(&progress_key, |row| row.bytes = bytes.min(total));
        });
        let result = work(progress).await;
        match &result {
            Ok(_) if direction == Direction::Upload => self.forget_upload(&key.1),
            Ok(_) => self.update(&key, |row| row.state = State::Transferred),
            Err(error) => self.update(&key, |row| row.state = match error {
                SyncError::Retry(_) => State::Retrying,
                SyncError::Blocked(_) => State::Failed,
            }),
        }
        result
    }

    fn update(&self, key: &(Direction, String), change: impl FnOnce(&mut FileTransfer)) {
        if let Some(row) = self.state.lock().unwrap_or_else(|error| error.into_inner()).files.get_mut(key) {
            change(row);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    /// 同时发出两个上传和两个下载，等待名额的任务没有机会启动传输。
    #[tokio::test]
    async fn uploads_and_downloads_share_two_slots() {
        let transfers = Transfers::default();
        let started = Arc::new(Semaphore::new(0));
        let finish = Arc::new(Semaphore::new(0));
        let mut tasks = Vec::new();
        for (index, direction) in [Direction::Upload, Direction::Download].into_iter().cycle().take(4).enumerate() {
            let (transfers, started, finish) = (transfers.clone(), started.clone(), finish.clone());
            tasks.push(tokio::spawn(async move {
                let id = index.to_string();
                let item = Attachment { object_id: id.clone(), name: "test".into(), sha256: String::new(), size: 2 };
                transfers.run(direction, id, &item, |progress| async move {
                    progress(1);
                    started.add_permits(1);
                    finish.acquire().await.unwrap().forget();
                    Ok(())
                }).await.unwrap();
            }));
        }
        started.acquire_many(2).await.unwrap().forget();
        assert!(tokio::time::timeout(Duration::from_millis(50), started.acquire()).await.is_err());
        let state = transfers.state.lock().unwrap_or_else(|error| error.into_inner());
        assert_eq!(state.files.values()
            .filter(|row| matches!(row.state, State::Transferring) && row.bytes == 1).count(), 2);
        drop(state);
        finish.add_permits(4);
        for task in tasks { task.await.unwrap(); }
    }
}

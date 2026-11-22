//! 同一个会话只有一个补拉任务和一个上传任务。两个任务独立运行。
use super::{protocol::*, StreamEvent, SyncError, SyncStore, SyncTransport};
use std::{collections::HashSet, future::Future, sync::Arc, time::Duration};
use tokio::{sync::{mpsc, watch}, task::JoinHandle, time::{self, Instant}};

const MIN_INTERVAL: Duration = Duration::from_millis(400);
const RECHECK: Duration = Duration::from_secs(60);
const MAX_RETRY: Duration = Duration::from_secs(30);
const ROUND_BUDGET: Duration = Duration::from_secs(3);
const MAX_BATCHES: usize = 8;

/// 单个任务的最近状态。失败状态始终保留待上传动作和已提交游标。
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionStatus {
    #[default]
    Idle,
    Working,
    Retrying,
    Blocked,
}

/// 会话的轻量唤醒入口。通道只合并信号，业务数据始终保存在仓储中。
#[derive(Clone)]
pub struct SyncTriggers {
    pull: mpsc::Sender<()>,
    upload: mpsc::Sender<()>,
}

impl SyncTriggers {
    /// 首次启用、SSE 和网络恢复统一调用这里。已有待处理信号时无需再发。
    pub fn request_pull(&self) {
        let _ = self.pull.try_send(());
    }

    /// 本地提交后或网络恢复时唤醒持久化队列的处理任务。
    pub fn request_upload(&self) {
        let _ = self.upload.try_send(());
    }

    /// 恢复连接后同时补拉和补传，两条任务各自遵守重试节奏。
    pub fn network_restored(&self) {
        self.request_pull();
        self.request_upload();
    }
}

/// 持有一个范围内的任务。换范围时先停止旧会话，再创建新会话。
pub struct SyncSession {
    triggers: SyncTriggers,
    stop: watch::Sender<bool>,
    task: JoinHandle<()>,
    status: watch::Receiver<(SessionStatus, SessionStatus)>,
}

impl SyncSession {
    /// 调用方已完成本地范围初始化。退出时先完成在途 SQLite 提交，再结束任务。
    pub fn start(scope: Scope, transport: Arc<dyn SyncTransport>, store: Arc<dyn SyncStore>) -> Self {
        let (pull, pull_rx) = mpsc::channel(1);
        let (upload, upload_rx) = mpsc::channel(1);
        let triggers = SyncTriggers { pull, upload };
        let (stop, stop_rx) = watch::channel(false);
        let (status_tx, status) = watch::channel((SessionStatus::Idle, SessionStatus::Idle));
        let workers = Workers { scope, transport, store, triggers: triggers.clone() };
        let task = tokio::spawn(async move {
            tokio::join!(
                run_worker("补拉", pull_rx, stop_rx.clone(),
                    |status| status_tx.send_modify(|state| state.1 = status),
                    || workers.pull_round(stop_rx.clone())),
                run_worker("上传", upload_rx, stop_rx.clone(),
                    |status| status_tx.send_modify(|state| state.0 = status),
                    || workers.upload_round(stop_rx.clone())),
                workers.listen(stop_rx.clone()),
                workers.fallback(stop_rx.clone()),
            );
        });
        triggers.network_restored();
        Self { triggers, stop, task, status }
    }

    /// 返回唤醒句柄，事件监听器不需要持有整个会话。
    pub fn triggers(&self) -> &SyncTriggers { &self.triggers }

    /// 订阅上传和补拉各自的状态。元组顺序是上传、补拉。
    pub fn subscribe_status(&self) -> watch::Receiver<(SessionStatus, SessionStatus)> {
        self.status.clone()
    }

    /// 等待旧任务结束，返回之后不会再提交旧范围的数据。
    pub async fn stop(mut self) {
        let _ = self.stop.send(true);
        let _ = (&mut self.task).await;
    }
}

impl Drop for SyncSession {
    fn drop(&mut self) {
        // 忘记显式关闭时仍通知后台退出。正常切换范围调用 stop 并等待。
        let _ = self.stop.send(true);
    }
}

struct Workers {
    scope: Scope,
    transport: Arc<dyn SyncTransport>,
    store: Arc<dyn SyncStore>,
    triggers: SyncTriggers,
}

impl Workers {
    /// 连续处理有积压的批次，每批收到完整确认后才继续。
    async fn upload_round(&self, mut stop: watch::Receiver<bool>) -> Result<bool, SyncError> {
        let started = Instant::now();
        for _ in 0..MAX_BATCHES {
            if *stop.borrow() { return Ok(false); }
            let operations = self.store.pending().await?;
            if operations.is_empty() { return Ok(false); }

            let request = UploadRequest { scope: self.scope.clone(), operations };
            // 网络请求可取消。即使服务器已提交，持久化队列仍能按原 ID 重试。
            let response = tokio::select! {
                biased;
                _ = stop.changed() => return Ok(false),
                response = self.transport.upload(&request) => response?,
            };
            let ids: HashSet<_> = response.acknowledgements.iter()
                .map(|ack| ack.operation_id.as_str()).collect();
            let sequences: HashSet<_> = response.acknowledgements.iter()
                .map(|ack| ack.sync_seq).collect();
            // 即使云端曾经提交，缺失确认时也保留整个批次，下一次仍发送原操作 ID。
            if response.scope != self.scope
                || response.acknowledgements.len() != request.operations.len()
                || ids.len() != request.operations.len()
                || sequences.len() != request.operations.len()
                || sequences.iter().any(|seq| *seq <= 0)
                || request.operations.iter().any(|operation| !ids.contains(operation.operation_id.as_str()))
            {
                return Err(SyncError::Blocked("服务端没有完整确认当前上传批次".into()));
            }
            self.store.confirm(&request.operations).await?;
            self.triggers.request_pull();
            if started.elapsed() >= ROUND_BUDGET { break; }
        }
        Ok(true)
    }

    /// 每页从仓储读取最新已提交的游标。请求或回放失败会重试这一页。
    async fn pull_round(&self, mut stop: watch::Receiver<bool>) -> Result<bool, SyncError> {
        let started = Instant::now();
        for _ in 0..MAX_BATCHES {
            if *stop.borrow() { return Ok(false); }
            let after = self.store.cursor().await?;
            let request = PullRequest { scope: self.scope.clone(), after, limit: BATCH_LIMIT };
            let page = tokio::select! {
                biased;
                _ = stop.changed() => return Ok(false),
                page = self.transport.pull(&request) => page?,
            };
            let mut previous = after;
            if page.scope != self.scope || page.changes.len() > request.limit
                || (page.has_more && page.changes.is_empty())
                || page.changes.iter().any(|change| {
                    let valid = change.sync_seq > previous;
                    previous = change.sync_seq;
                    !valid
                })
            {
                return Err(SyncError::Blocked("服务端返回的同步范围或分页顺序无效".into()));
            }
            let has_more = page.has_more;
            if !page.changes.is_empty() { self.store.apply(after, page).await?; }
            if !has_more { return Ok(false); }
            if started.elapsed() >= ROUND_BUDGET { break; }
        }
        Ok(true)
    }

    /// SSE 生命周期独立于补拉。通知只产生唤醒，断线时按退避重连。
    async fn listen(&self, mut stop: watch::Receiver<bool>) {
        let mut retry = Duration::from_secs(1);
        let notify = |event| match event {
            StreamEvent::Connected => self.triggers.network_restored(),
            StreamEvent::Changed => self.triggers.request_pull(),
        };
        while !*stop.borrow() {
            let started = Instant::now();
            let result = tokio::select! {
                _ = stop.changed() => break,
                result = self.transport.listen(&self.scope, &notify) => result,
            };
            if let Err(error) = &result { log::warn!("同步通知连接结束：{error}"); }
            if started.elapsed() >= MAX_RETRY { retry = Duration::from_secs(1); }
            let delay = if matches!(result, Err(SyncError::Blocked(_))) { RECHECK } else { retry };
            tokio::select! {
                _ = stop.changed() => break,
                _ = time::sleep(delay) => {}
            }
            retry = (retry * 2).min(MAX_RETRY);
        }
    }

    /// 定时核对遗漏通知和遗留队列。定时器也通过统一的唤醒入口提交请求。
    async fn fallback(&self, mut stop: watch::Receiver<bool>) {
        loop {
            tokio::select! {
                _ = stop.changed() => break,
                _ = time::sleep(RECHECK) => self.triggers.network_restored(),
            }
        }
    }
}

/// 补拉和上传共用限频与错误恢复逻辑，业务轮次只负责一批批处理数据。
async fn run_worker<F, Fut>(
    name: &str,
    mut wake: mpsc::Receiver<()>,
    mut stop: watch::Receiver<bool>,
    status: impl Fn(SessionStatus),
    round: F,
) where F: Fn() -> Fut, Fut: Future<Output = Result<bool, SyncError>> {
    let mut earliest = Instant::now();
    let mut retry = Duration::from_secs(1);
    let mut pending = false;
    while !*stop.borrow() {
        if !pending {
            tokio::select! {
                _ = stop.changed() => break,
                message = wake.recv() => if message.is_none() { break; },
            }
        }
        // 等候期间到达的通知与当前轮合并。运行期间再来的通知留给下一轮。
        tokio::select! {
            _ = stop.changed() => break,
            _ = time::sleep_until(earliest) => {}
        }
        let _ = wake.try_recv();
        let started = Instant::now();
        status(SessionStatus::Working);
        // 不在中途取消数据库操作。会话切换等该轮完成后才装配新范围。
        match round().await {
            Ok(more) => {
                status(SessionStatus::Idle);
                pending = more;
                retry = Duration::from_secs(1);
                earliest = started + MIN_INTERVAL;
            }
            Err(error) => {
                log::warn!("同步{name}失败：{error}");
                match error {
                    SyncError::Retry(_) => {
                        status(SessionStatus::Retrying);
                        pending = true;
                        earliest = Instant::now() + retry;
                        retry = (retry * 2).min(MAX_RETRY);
                    }
                    SyncError::Blocked(_) => {
                        status(SessionStatus::Blocked);
                        pending = false;
                        earliest = Instant::now() + RECHECK;
                    }
                }
            }
        }
    }
}

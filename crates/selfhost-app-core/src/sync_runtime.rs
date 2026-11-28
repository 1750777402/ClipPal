//! 连接生命周期及本机变更入口。会话任务和传输细节由 clip-sync-client 管理。
use crate::transfers::{CardSyncStatus, IncomingAttachment, Transfers};
use crate::sync_store::{self, Enqueue, SqliteSyncStore};
use clip_sync_client::{
    auth::SyncAuthProvider,
    protocol::{SyncLimits, DEFAULT_RECORD_BYTES},
    HttpsTransport, SessionState, SessionStatus, SyncError, SyncSession, SyncTransport, SyncTriggers,
};
use desktop_app_core::{
    capture::{CaptureInput, CaptureService, TextEncoder},
    clip_core::{domain::clip::ClipRecord, errors::{AppError, AppResult}},
    records::ClipRecordService,
    LocalContext,
};
use std::{collections::BTreeMap, sync::Arc, time::Duration};
use tokio::{sync::{watch, Mutex}, task::JoinHandle};

/// 对桌面层公开的同步状态。错误详情由后端记录。
#[derive(Clone, Default, serde::Serialize)]
pub struct SyncStatus {
    pub enabled: bool,
    pub waiting_for_limits: bool,
    #[serde(flatten)]
    pub session: SessionState,
}

/// 一次返回可见记录的上传状态和暂未落库的下载记录。
#[derive(Default, serde::Serialize)]
pub struct SyncCardStates {
    enabled: bool,
    cards: BTreeMap<String, CardSyncStatus>,
    incoming: Vec<IncomingAttachment>,
}

/// 同步扩展独立持有会话生命周期和同步能力。
pub struct SyncContext {
    local: Arc<LocalContext>,
    auth: Arc<dyn SyncAuthProvider>,
    running: Mutex<Option<Running>>,
    transfers: Transfers,
    // 初次导入持有此锁，确保任意一次采集都进入首批历史或直接写入队列。
    active: Arc<Mutex<Option<Active>>>,
}

struct Active {
    limits: SyncLimits,
    triggers: Option<SyncTriggers>,
}

struct Running {
    stop: watch::Sender<bool>,
    task: JoinHandle<()>,
    status: watch::Receiver<SyncStatus>,
}

impl Drop for Running {
    /// 应用上下文被释放时，后台任务也应关闭。
    fn drop(&mut self) {
        self.stop.send_replace(true);
    }
}

impl SyncContext {
    /// 组装控制资源，网络连接等到显式启用。
    pub(crate) fn new(local: Arc<LocalContext>, auth: Arc<dyn SyncAuthProvider>) -> Self {
        Self { local, auth, running: Mutex::new(None), transfers: Transfers::default(), active: Arc::new(Mutex::new(None)) }
    }

    /// 供桌面策略判断当前部署凭据是否有效。
    pub fn auth(&self) -> &dyn SyncAuthProvider {
        self.auth.as_ref()
    }

    /// 以部署配置启用同步。多次调用复用同一会话。
    pub async fn enable(&self, notify: Arc<dyn Fn() + Send + Sync>, encoder: Arc<dyn TextEncoder>) -> AppResult<()> {
        let connection = self.auth.connection().await.map_err(AppError::Config)?;
        let transport = Arc::new(HttpsTransport::new(connection)
            .map_err(|error| AppError::Network(error.to_string()))?);
        self.start(transport, notify, encoder).await
    }

    /// 串行装配会话。端口注入也用于验证完整的客户端流程。
    async fn start(
        &self, transport: Arc<dyn SyncTransport>,
        notify: Arc<dyn Fn() + Send + Sync>, encoder: Arc<dyn TextEncoder>,
    ) -> AppResult<()> {
        let mut running = self.running.lock().await;
        if running.is_some() { return Ok(()); }
        sync_store::initialize(self.local.db()).await?;
        // 已有本机状态时可以离线恢复队列。
        let saved = sync_store::load_state(self.local.db()).await?;
        let (stop, stop_rx) = watch::channel(false);
        let (status_tx, status) = watch::channel(SyncStatus {
            enabled: true, waiting_for_limits: true,
            ..Default::default()
        });
        let local = self.local.clone();
        let active = self.active.clone();
        let transfers = self.transfers.clone();
        if let Some(saved) = saved {
            *active.lock().await = Some(Active { limits: saved.offline_limits(), triggers: None });
        }
        let task = tokio::spawn(async move {
            let mut stop = stop_rx;
            let session = loop {
                if *stop.borrow() { return; }
                // 启动时读取同步能力。已有本机队列可在断网时继续入队。
                let result = tokio::select! {
                    biased;
                    _ = stop.changed() => return,
                    result = async {
                        let mut limits = transport.limits().await?;
                        limits.max_record_bytes = limits.max_record_bytes.min(DEFAULT_RECORD_BYTES);
                        Ok(limits)
                    } => result,
                };
                let error = match result {
                    Ok(limits) => {
                        let mut target = active.lock().await;
                        if *stop.borrow() { return; }
                        match sync_store::initialize_history(&local, limits).await {
                            Ok(()) => {
                                let session = activate(&local, &mut target,
                                    transport.clone(), notify.clone(), limits, transfers.clone(), encoder.clone());
                                (notify)();
                                break session;
                            }
                            Err(error) => SyncError::Blocked(error.to_string()),
                        }
                    }
                    Err(error) => error,
                };
                log::warn!("读取同步能力或初始化本地队列失败：{error}");
                let (state, wait) = match error {
                    SyncError::Retry(_) => (SessionStatus::Retrying, 10),
                    SyncError::Blocked(_) => (SessionStatus::Blocked, 60),
                };
                status_tx.send_replace(SyncStatus {
                    enabled: true, waiting_for_limits: true,
                    session: SessionState { upload: state, pull: state, ..Default::default() },
                });
                tokio::select! {
                    _ = stop.changed() => return,
                    _ = tokio::time::sleep(Duration::from_secs(wait)) => {}
                }
            };
            let mut state = session.subscribe_status();
            loop {
                if *stop.borrow() { break; }
                status_tx.send_replace(SyncStatus {
                    enabled: true, waiting_for_limits: false, session: *state.borrow_and_update(),
                });
                tokio::select! {
                    _ = stop.changed() => break,
                    result = state.changed() => if result.is_err() { break; },
                }
            }
            session.stop().await;
        });
        *running = Some(Running { stop, task, status });
        Ok(())
    }

    /// 关闭同步并等待会话退出，保留尚未确认的操作。
    pub async fn disable(&self) {
        let mut running = self.running.lock().await;
        if let Some(state) = running.as_mut() {
            state.stop.send_replace(true);
            self.active.lock().await.take();
            let _ = (&mut state.task).await;
            running.take();
            self.transfers.clear_in_flight();
        }
    }

    /// 提供前端可读的任务快照，内部错误通过日志排查。
    pub async fn status(&self) -> SyncStatus {
        let running = self.running.lock().await;
        running.as_ref().map_or_else(SyncStatus::default, |state| state.status.borrow().clone())
    }

    /// 批量读取卡片状态，避免每个虚拟列表卡片单独获取数据库。
    pub async fn card_states(&self, mut keys: Vec<String>) -> AppResult<SyncCardStates> {
        let running = self.running.lock().await;
        if running.is_none() { return Ok(SyncCardStates::default()); }
        keys.truncate(100);
        let uploads = sync_store::card_uploads(self.local.db(), &keys).await?;
        let (cards, incoming) = self.transfers.card_states(uploads);
        Ok(SyncCardStates { enabled: true, cards, incoming })
    }

    /// 返回当前部署的限额，未绑定时仍可展示不支持同步的记录类型。
    pub async fn limits(&self) -> Option<SyncLimits> {
        self.active.lock().await.as_ref().map(|entry| entry.limits)
    }

    /// 采集使用单机服务。入队提交成功后才唤醒上传任务。
    pub(crate) async fn capture(
        &self, input: CaptureInput<'_>, encoder: &dyn TextEncoder,
    ) -> AppResult<Option<ClipRecord>> {
        let service = CaptureService::new(&self.local, encoder);
        let active = self.active.lock().await;
        let Some(active) = active.as_ref() else { return service.capture(input).await; };
        let hook = Enqueue::Upsert { local: &self.local, limits: active.limits };
        let saved = service.capture_with(input, Some(&hook)).await?;
        if let (Some(_), Some(triggers)) = (&saved, &active.triggers) {
            triggers.request_upload();
        }
        Ok(saved)
    }

    /// 删除本机记录并登记同步动作。暂停时只写入本机，恢复后按原顺序发送。
    pub async fn delete_everywhere(&self, id: String) -> AppResult<String> {
        let running = self.running.lock().await;
        if running.is_none() {
            self.auth.connection().await.map_err(AppError::Config)?;
            sync_store::initialize(self.local.db()).await?;
        }
        let active = self.active.lock().await;
        let result = ClipRecordService::new(&self.local).delete_record_with(id, Some(&Enqueue::Delete)).await?;
        if let Some(triggers) = active.as_ref().and_then(|state| state.triggers.as_ref()) {
            triggers.request_upload();
        }
        Ok(result)
    }
}

/// 安装同步会话和它的唤醒入口。
fn activate(
    local: &Arc<LocalContext>, target: &mut Option<Active>,
    transport: Arc<dyn SyncTransport>, notify: Arc<dyn Fn() + Send + Sync>, limits: SyncLimits, transfers: Transfers,
    encoder: Arc<dyn TextEncoder>,
) -> SyncSession {
    let session = SyncSession::start(transport.clone(), Arc::new(SqliteSyncStore {
        local: local.clone(), notify, transport, transfers, encoder,
    }));
    *target = Some(Active { limits, triggers: Some(session.triggers().clone()) });
    session
}

#[cfg(test)]
#[path = "sync_tests.rs"]
mod tests;

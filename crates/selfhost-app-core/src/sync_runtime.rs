//! 连接生命周期及本机变更入口。会话任务和传输细节由 clip-sync-client 管理。
use crate::sync_store::{self, Binding, Enqueue, SqliteSyncStore};
use clip_sync_client::{
    auth::SyncAuthProvider,
    v2::{protocol::Scope, HttpsTransport, SessionStatus, SyncError, SyncSession, SyncTransport, SyncTriggers},
};
use desktop_app_core::{
    capture::{CaptureInput, CaptureService},
    clip_core::{domain::clip::ClipRecord, errors::{AppError, AppResult}},
    records::ClipRecordService,
    LocalContext,
};
use std::{sync::Arc, time::Duration};
use tokio::{sync::{watch, Mutex}, task::JoinHandle};

/// 对桌面层公开的同步状态。错误详情由后端记录。
#[derive(Clone, Default, serde::Serialize)]
pub struct SyncStatus {
    pub enabled: bool,
    pub waiting_for_binding: bool,
    pub can_delete_everywhere: bool,
    pub upload: SessionStatus,
    pub pull: SessionStatus,
}

/// 同步扩展独立持有生命周期和当前写入范围。
pub struct SyncContext {
    local: Arc<LocalContext>,
    auth: Arc<dyn SyncAuthProvider>,
    running: Mutex<Option<Running>>,
    // 初次导入持有此锁，确保任意一次采集都进入首批历史或直接写入队列。
    active: Arc<Mutex<Option<Active>>>,
}

struct Active {
    id: i64,
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
        Self { local, auth, running: Mutex::new(None), active: Arc::new(Mutex::new(None)) }
    }

    /// 供桌面策略判断当前部署凭据是否有效。
    pub fn auth(&self) -> &dyn SyncAuthProvider {
        self.auth.as_ref()
    }

    /// 以部署配置启用同步。多次调用复用同一会话。
    pub async fn enable(&self, notify: Arc<dyn Fn() + Send + Sync>) -> AppResult<()> {
        let connection = self.auth.connection().await.map_err(AppError::Config)?;
        let transport = Arc::new(HttpsTransport::new(connection)
            .map_err(|error| AppError::Network(error.to_string()))?);
        self.start(transport.endpoint().to_owned(), transport, notify).await
    }

    /// 串行装配会话。端口注入也用于验证完整的客户端流程。
    async fn start(
        &self, endpoint: String, transport: Arc<dyn SyncTransport>,
        notify: Arc<dyn Fn() + Send + Sync>,
    ) -> AppResult<()> {
        let mut running = self.running.lock().await;
        if running.is_some() { return Ok(()); }
        sync_store::initialize(self.local.db()).await?;
        // 已绑定的部署无需联网即可启动，断网重启后复制内容仍会写入同一个队列。
        let binding = sync_store::load_binding(self.local.db(), &endpoint).await?;
        let (stop, stop_rx) = watch::channel(false);
        let has_binding = binding.is_some();
        let (status_tx, status) = watch::channel(SyncStatus {
            enabled: true, waiting_for_binding: true, can_delete_everywhere: has_binding,
            ..Default::default()
        });
        let local = self.local.clone();
        let active = self.active.clone();
        if let Some(binding) = &binding {
            *active.lock().await = Some(Active { id: binding.id(), triggers: None });
        }
        let task = tokio::spawn(async move {
            let mut stop = stop_rx;
            let session = loop {
                if *stop.borrow() { return; }
                // 每次启动先验证数据集。断网时可以入队，验证通过前不会发送密文。
                let result = tokio::select! {
                    biased;
                    _ = stop.changed() => return,
                    scope = transport.scope() => scope,
                };
                let error = match result {
                    Ok(scope) if scope.is_valid() => {
                        let mut target = active.lock().await;
                        if *stop.borrow() { return; }
                        match sync_store::bind(&local, &endpoint, &scope).await {
                            Ok(binding) => break activate(&local, &mut target, binding,
                                transport.clone(), notify.clone()),
                            Err(error) => SyncError::Blocked(error.to_string()),
                        }
                    }
                    Ok(_) => SyncError::Blocked("无效的同步范围".into()),
                    Err(error) => error,
                };
                log::warn!("同步部署握手失败：{error}");
                let (state, wait) = match error {
                    SyncError::Retry(_) => (SessionStatus::Retrying, 10),
                    SyncError::Blocked(_) => (SessionStatus::Blocked, 60),
                };
                status_tx.send_replace(SyncStatus {
                    enabled: true, waiting_for_binding: true, can_delete_everywhere: has_binding,
                    upload: state, pull: state,
                });
                tokio::select! {
                    _ = stop.changed() => return,
                    _ = tokio::time::sleep(Duration::from_secs(wait)) => {}
                }
            };
            let mut state = session.subscribe_status();
            loop {
                if *stop.borrow() { break; }
                let (upload, pull) = *state.borrow_and_update();
                status_tx.send_replace(SyncStatus {
                    enabled: true, waiting_for_binding: false, can_delete_everywhere: true, upload, pull,
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

    /// 关闭写入范围并等旧会话退出，保留尚未确认的操作。
    pub async fn disable(&self) {
        let mut running = self.running.lock().await;
        if let Some(state) = running.as_mut() {
            state.stop.send_replace(true);
            self.active.lock().await.take();
            let _ = (&mut state.task).await;
            running.take();
        }
    }

    /// 提供前端可读的任务快照，内部错误通过日志排查。
    pub async fn status(&self) -> SyncStatus {
        let running = self.running.lock().await;
        running.as_ref().map_or_else(SyncStatus::default, |state| state.status.borrow().clone())
    }

    /// 文本采集使用单机服务。入队提交成功后才唤醒上传任务。
    pub(crate) async fn capture(
        &self, service: CaptureService<'_>, input: CaptureInput<'_>,
    ) -> AppResult<Option<ClipRecord>> {
        if !matches!(input, CaptureInput::Text(_)) {
            return service.capture(input).await;
        }
        let active = self.active.lock().await;
        let Some(active) = active.as_ref() else { return service.capture(input).await; };
        let hook = Enqueue { scope: active.id, delete: false };
        let saved = service.capture_with(input, Some(&hook)).await?;
        if let (Some(_), Some(triggers)) = (&saved, &active.triggers) {
            triggers.request_upload();
        }
        Ok(saved)
    }

    /// 跨端删除是显式业务动作。普通删除仍调用原有的单机服务。
    pub async fn delete_everywhere(&self, id: String) -> AppResult<String> {
        let active = self.active.lock().await;
        let active = active.as_ref().ok_or_else(|| AppError::Config("云同步尚未启用或完成首次连接".into()))?;
        let hook = Enqueue { scope: active.id, delete: true };
        let result = ClipRecordService::new(&self.local).delete_record_with(id, Some(&hook)).await?;
        if let Some(triggers) = &active.triggers { triggers.request_upload(); }
        Ok(result)
    }
}

/// 安装当前范围及它的唤醒入口，调用方持有写入范围锁以免采集经过空档。
fn activate(
    local: &Arc<LocalContext>, target: &mut Option<Active>, binding: Binding,
    transport: Arc<dyn SyncTransport>, notify: Arc<dyn Fn() + Send + Sync>,
) -> SyncSession {
    let id = binding.id();
    let scope: Scope = binding.scope();
    let session = SyncSession::start(scope, transport, Arc::new(SqliteSyncStore {
        local: local.clone(), binding, notify,
    }));
    *target = Some(Active { id, triggers: Some(session.triggers().clone()) });
    session
}

#[cfg(test)]
#[path = "sync_tests.rs"]
mod tests;

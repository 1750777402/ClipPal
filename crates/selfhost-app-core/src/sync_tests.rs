//! 用 SQLite 临时库和内存云端验证客户端闭环，不读取用户的剪贴板或正式数据库。
use super::*;
use crate::SelfhostContext;
use async_trait::async_trait;
use clip_sync_client::{protocol::*, StreamEvent, SyncStore, TransferProgress};
use desktop_app_core::clip_core::content_key::text_key;
use std::sync::{
    atomic::{AtomicBool, AtomicUsize, Ordering::SeqCst},
    Mutex as StdMutex,
};
use tokio::sync::{broadcast, Notify};

const ENDPOINT: &str = "https://sync.test/";
const TEST_KEY: &str = "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=";

/// 使用真实加密和解密，确认线上正文和本机文本采用同一套格式。
fn encode(value: &str) -> AppResult<String> {
    cipher().encrypt(value)
}

fn cipher() -> desktop_app_core::clip_core::crypto::ContentCipher {
    desktop_app_core::clip_core::crypto::ContentCipher::from_base64_key(TEST_KEY).unwrap()
}

async fn open(path: &std::path::Path) -> SelfhostContext {
    let local = LocalContext::open_with_resources(path, None, Arc::new(|value: &str| cipher().decrypt(value)),
        path.parent().map(|parent| parent.join("resources")))
        .await.unwrap();
    let deployment = crate::deployment::DeploymentConfig::new(ENDPOINT, "test-deployment-token").unwrap();
    SelfhostContext::with_auth(local, Arc::new(deployment))
}

/// 只实现客户端依赖的服务端契约。按 ID 幂等，模拟提交后的确认丢失和 SSE 断开。
struct FakeServer {
    online: AtomicBool,
    lose_ack: AtomicBool,
    state: StdMutex<ServerState>,
    events: broadcast::Sender<()>,
    pull_count: AtomicUsize,
    hold_pull: AtomicBool,
    attachment_enabled: AtomicBool,
    interrupt_attachment: AtomicBool,
    interrupt_download: AtomicBool,
    release_pull: Notify,
}

#[derive(Default)]
struct ServerState {
    committed: Vec<Operation>,
    upload_attempts: Vec<Vec<String>>,
    pull_after: Vec<i64>,
    objects: std::collections::HashMap<String, Vec<u8>>,
}

impl FakeServer {
    fn new() -> Arc<Self> {
        Arc::new(Self {
            online: AtomicBool::new(true), lose_ack: AtomicBool::new(false),
            state: StdMutex::default(), events: broadcast::channel(64).0,
            pull_count: AtomicUsize::new(0), hold_pull: AtomicBool::new(false),
            attachment_enabled: AtomicBool::new(false), interrupt_attachment: AtomicBool::new(false),
            interrupt_download: AtomicBool::new(false),
            release_pull: Notify::new(),
        })
    }

    fn connected(&self) -> Result<(), SyncError> {
        if self.online.load(SeqCst) { Ok(()) } else { Err(SyncError::Retry("test offline".into())) }
    }

    fn wake(&self) {
        let _ = self.events.send(());
    }
}

#[async_trait]
impl SyncTransport for FakeServer {
    async fn limits(&self) -> Result<SyncLimits, SyncError> {
        self.connected()?;
        Ok(SyncLimits { attachments: self.attachment_enabled.load(SeqCst), ..SyncLimits::default() })
    }

    async fn put_attachment(&self, item: &Attachment, bytes: Vec<u8>, progress: Arc<TransferProgress>) -> Result<(), SyncError> {
        self.connected()?;
        progress((bytes.len() / 2) as u64);
        if self.interrupt_attachment.swap(false, SeqCst) {
            return Err(SyncError::Retry("test stream interrupted".into()));
        }
        progress(bytes.len() as u64);
        self.state.lock().unwrap().objects.insert(item.object_id.clone(), bytes);
        Ok(())
    }

    async fn get_attachment(&self, item: &Attachment, progress: &TransferProgress) -> Result<Vec<u8>, SyncError> {
        self.connected()?;
        let bytes = self.state.lock().unwrap().objects.get(&item.object_id).cloned()
            .ok_or_else(|| SyncError::Retry("test object missing".into()))?;
        if self.interrupt_download.swap(false, SeqCst) {
            progress((bytes.len() / 2) as u64);
            return Err(SyncError::Retry("test download interrupted".into()));
        }
        progress(bytes.len() as u64);
        Ok(bytes)
    }

    async fn upload(&self, request: &UploadRequest) -> Result<UploadResponse, SyncError> {
        self.connected()?;
        let mut state = self.state.lock().unwrap();
        state.upload_attempts.push(request.operations.iter().map(|op| op.operation_id.clone()).collect());
        let mut acks = Vec::new();
        for op in &request.operations {
            let position = state.committed.iter().position(|stored| stored.operation_id == op.operation_id);
            let sync_seq = match position {
                Some(pos) => {
                    let previous = &state.committed[pos];
                    assert_eq!(previous.content_key, op.content_key);
                    assert_eq!(previous.created_at, op.created_at);
                    match (&previous.action, &op.action) {
                        (Action::Delete, Action::Delete) => {}
                        (Action::Upsert { ciphertext: before }, Action::Upsert { ciphertext: after }) => {
                            assert_eq!(cipher().decrypt(before).unwrap(), cipher().decrypt(after).unwrap());
                        }
                        _ => panic!("同一个动作重试时类型发生变化"),
                    }
                    pos + 1
                }
                None => {
                    state.committed.push(op.clone());
                    state.committed.len()
                }
            };
            acks.push(Ack { operation_id: op.operation_id.clone(), sync_seq: sync_seq as i64 });
        }
        drop(state);
        self.wake();
        if self.lose_ack.swap(false, SeqCst) {
            return Err(SyncError::Retry("test response lost after commit".into()));
        }
        Ok(UploadResponse { acknowledgements: acks })
    }

    async fn pull(&self, request: &PullRequest) -> Result<PullPage, SyncError> {
        self.connected()?;
        self.state.lock().unwrap().pull_after.push(request.after);
        self.pull_count.fetch_add(1, SeqCst);
        if self.hold_pull.load(SeqCst) {
            self.release_pull.notified().await;
        }
        let state = self.state.lock().unwrap();
        // 每页仅返回一条，确保现有客户端会从成功提交的游标接着拉取。
        let index = request.after as usize;
        let changes = state.committed.get(index).map(|op| Change {
            sync_seq: (index + 1) as i64,
            committed_at: op.created_at,
            content_key: op.content_key.clone(),
            action: op.action.clone(),
        }).into_iter().collect();
        Ok(PullPage {
            changes, has_more: state.committed.len() > index + 1,
        })
    }

    async fn listen(
        &self, notify: &(dyn Fn(StreamEvent) + Send + Sync),
    ) -> Result<(), SyncError> {
        self.connected()?;
        let mut events = self.events.subscribe();
        notify(StreamEvent::Connected);
        while events.recv().await.is_ok() {
            self.connected()?;
            notify(StreamEvent::Changed);
        }
        Ok(())
    }
}

async fn start(app: &SelfhostContext, server: Arc<FakeServer>) {
    app.sync().start(server, Arc::new(|| {}), Arc::new(encode)).await.unwrap();
}

async fn capture(app: &SelfhostContext, value: &str) -> ClipRecord {
    app.capture(CaptureInput::Text(value), &encode).await.unwrap().unwrap()
}

async fn eventually(mut condition: impl AsyncFnMut() -> bool) {
    tokio::time::timeout(Duration::from_secs(8), async {
        while !condition().await {
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    }).await.expect("等待客户端提交同步数据超时");
}

fn store(app: &SelfhostContext) -> SqliteSyncStore {
    SqliteSyncStore {
        local: app.local.clone(),
        notify: Arc::new(|| {}),
        transport: FakeServer::new(),
        transfers: Transfers::default(),
        encoder: Arc::new(encode),
    }
}

/// 暂停时隐藏卡片状态。重新打开同一 SQLite 后，即使离线也能恢复上传状态。
#[tokio::test]
async fn reopening_sync_restores_card_status_and_reports_connection() {
    use clip_sync_client::NotificationStatus;

    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("test.db");
    let app = open(&path).await;
    let server = FakeServer::new();
    start(&app, server.clone()).await;
    eventually(async || app.sync().status().await.session.notifications == NotificationStatus::Connected).await;

    let record = capture(&app, "already acknowledged").await;
    let uploaded = record.content_key;
    eventually(async || {
        app.sync().card_states(vec![uploaded.clone()]).await.unwrap().cards.get(&uploaded)
            == Some(&CardSyncStatus::Uploaded)
    }).await;
    assert_eq!(capture(&app, "already acknowledged").await.id, record.id);
    eventually(async || upload_confirmation_count(&app, record.id).await == 2).await;

    server.online.store(false, SeqCst);
    server.wake();
    eventually(async || app.sync().status().await.session.notifications == NotificationStatus::Retrying).await;
    let pending = capture(&app, "pending during disconnect").await.content_key;
    let keys = vec![uploaded.clone(), pending.clone()];
    app.sync().disable().await;
    let paused = app.sync().card_states(keys.clone()).await.unwrap();
    assert!(!paused.enabled);
    assert!(paused.cards.is_empty());

    drop(app);
    let app = open(&path).await;
    start(&app, server.clone()).await;
    let restored = app.sync().card_states(keys).await.unwrap();
    assert!(restored.enabled);
    assert_eq!(restored.cards.get(&uploaded), Some(&CardSyncStatus::Uploaded));
    assert_eq!(restored.cards.get(&pending), Some(&CardSyncStatus::PendingUpload));

    // 容量清理等本机删除也会级联清除旧记录的确认。新建同内容记录不会继承绿点。
    app.sync().disable().await;
    let old = app.local().repositories().clip_records().find_by_content_key(&uploaded).await.unwrap().unwrap();
    ClipRecordService::new(app.local()).delete_record(old.id.to_string()).await.unwrap();
    assert_eq!(upload_confirmation_count(&app, old.id).await, 0);
    let recreated = capture(&app, "already acknowledged").await;
    assert_ne!(old.id, recreated.id);

    server.online.store(true, SeqCst);
    start(&app, server).await;
    assert!(!app.sync().card_states(vec![uploaded]).await.unwrap().cards.contains_key(&recreated.content_key));
    eventually(async || {
        app.sync().status().await.session.notifications == NotificationStatus::Connected
            && app.sync().card_states(vec![pending.clone()]).await.unwrap().cards.get(&pending)
                == Some(&CardSyncStatus::Uploaded)
    }).await;
    app.sync().disable().await;

    async fn upload_confirmation_count(app: &SelfhostContext, record_id: i64) -> i64 {
        #[derive(serde::Deserialize)]
        struct Count { count: i64 }
        let rows: Vec<Count> = app.local().db().query_decode(
            "SELECT COUNT(*) AS count FROM clip_sync_record WHERE record_id=? AND upload_status=1",
            vec![rbs::to_value!(record_id)],
        ).await.unwrap();
        rows[0].count
    }
}

#[tokio::test]
async fn offline_restart_keeps_order_and_retries_with_the_same_operation_ids() {
    let path_a = tempfile::tempdir().unwrap();
    let path_b = tempfile::tempdir().unwrap();
    let db_a = path_a.path().join("test.db");
    let server = FakeServer::new();
    let a = open(&db_a).await;
    let b = open(&path_b.path().join("test.db")).await;

    let original = capture(&a, "123").await;
    start(&a, server.clone()).await;
    start(&b, server.clone()).await;
    eventually(async || {
        b.local().repositories().clip_records().find_by_content_key(&text_key("123"))
            .await.unwrap().is_some()
    }).await;
    // 两端均处理完首次上传。断网重启后，不需要等待握手即可追加本地操作。
    eventually(async || store(&a).pending().await.unwrap().is_empty()).await;
    a.sync().disable().await;
    drop(a);
    server.online.store(false, SeqCst);
    server.wake();
    let a = open(&db_a).await;
    start(&a, server.clone()).await;
    a.sync().delete_everywhere(original.id.to_string()).await.unwrap();
    capture(&a, "123").await;
    let pending = store(&a).pending().await.unwrap();
    assert!(matches!(pending[0].action, Action::Delete));
    assert!(matches!(&pending[1].action, Action::Upsert { ciphertext } if ciphertext != "123"));

    a.sync().disable().await;
    drop(a);
    server.lose_ack.store(true, SeqCst);
    server.online.store(true, SeqCst);
    let a = open(&db_a).await;
    start(&a, server.clone()).await;
    eventually(async || {
        let store_a = store(&a);
        store_a.pending().await.unwrap().is_empty() && store_a.cursor().await.unwrap() == 3
            && store(&b).cursor().await.unwrap() == 3
    }).await;
    {
        let state = server.state.lock().unwrap();
        assert_eq!(state.committed.len(), 3);
        assert!(state.committed[1..] == pending);
        assert_eq!(state.upload_attempts.len(), 3);
        assert_eq!(state.upload_attempts[1], state.upload_attempts[2]);
        for after in [0, 1, 2] { assert!(state.pull_after.contains(&after)); }
    }
    assert_eq!(b.local().repositories().clip_records().count().await.unwrap(), 1);
    assert_eq!(b.local().search_engine().search("123").await.len(), 1);
    a.sync().disable().await;
    b.sync().disable().await;
}

/// 本机删除和云端回放都不能清除待确认动作。重启重试时，云端只提交一次。
#[tokio::test]
async fn offline_delete_survives_restart_and_lost_acknowledgement() {
    let path_a = tempfile::tempdir().unwrap();
    let path_b = tempfile::tempdir().unwrap();
    let db_a = path_a.path().join("test.db");
    let server = FakeServer::new();
    let a = open(&db_a).await;
    let b = open(&path_b.path().join("test.db")).await;
    let record = capture(&a, "delete while offline").await;

    start(&a, server.clone()).await;
    start(&b, server.clone()).await;
    eventually(async || store(&a).cursor().await.unwrap() == 1
        && store(&b).cursor().await.unwrap() == 1
        && store(&a).pending().await.unwrap().is_empty()).await;

    server.online.store(false, SeqCst);
    server.wake();
    a.sync().delete_everywhere(record.id.to_string()).await.unwrap();
    assert_eq!(a.local().repositories().clip_records().count().await.unwrap(), 0);
    assert_eq!(b.local().repositories().clip_records().count().await.unwrap(), 1);
    let pending = store(&a).pending().await.unwrap();
    assert_eq!(pending.len(), 1);
    assert_eq!(pending[0].content_key, record.content_key);
    assert!(matches!(pending[0].action, Action::Delete));
    a.sync().disable().await;
    drop(a);

    let a = open(&db_a).await;
    assert!(store(&a).pending().await.unwrap() == pending);
    server.online.store(true, SeqCst);
    server.lose_ack.store(true, SeqCst);
    // 模拟服务端提交成功，客户端在收到上传响应前断开。
    let request = UploadRequest { operations: pending.clone() };
    assert!(matches!(server.upload(&request).await, Err(SyncError::Retry(_))));
    let page = server.pull(&PullRequest { after: 1, limit: 100 }).await.unwrap();
    store(&a).apply(1, page).await.unwrap();
    assert_eq!(store(&a).cursor().await.unwrap(), 2);
    assert!(store(&a).pending().await.unwrap() == pending);

    start(&a, server.clone()).await;
    eventually(async || store(&a).pending().await.unwrap().is_empty()
        && store(&b).cursor().await.unwrap() == 2).await;
    assert_eq!(b.local().repositories().clip_records().count().await.unwrap(), 0);
    {
        let state = server.state.lock().unwrap();
        assert_eq!(state.committed.len(), 2);
        assert!(state.committed[1] == pending[0]);
        assert_eq!(state.upload_attempts[1], state.upload_attempts[2]);
    }
    a.sync().disable().await;
    b.sync().disable().await;
}

#[tokio::test]
async fn failed_page_rolls_back_records_and_cursor_and_does_not_enqueue() {
    let dir = tempfile::tempdir().unwrap();
    let app = open(&dir.path().join("test.db")).await;
    sync_store::initialize(app.local().db()).await.unwrap();
    sync_store::initialize_history(app.local(), SyncLimits::default()).await.unwrap();
    let store = store(&app);
    let make_page = || PullPage {
        changes: ["first", "second"].into_iter().enumerate().map(|(index, value)| Change {
            sync_seq: index as i64 + 1,
            committed_at: 1, content_key: text_key(value),
            action: Action::Upsert { ciphertext: encode(value).unwrap() },
        }).collect(),
        has_more: false,
    };
    // 让第一条 INSERT 成功，第二条失败，实际经过 SQLite 回滚路径。
    app.local().db().exec(
        "CREATE TRIGGER fail_second BEFORE INSERT ON clip_record WHEN
         (SELECT COUNT(*) FROM clip_record)>0 BEGIN SELECT RAISE(ABORT,'fail second'); END",
        vec![],
    ).await.unwrap();
    assert!(store.apply(0, make_page()).await.is_err());
    assert_eq!(store.cursor().await.unwrap(), 0);
    assert_eq!(app.local().repositories().clip_records().count().await.unwrap(), 0);
    app.local().db().exec("DROP TRIGGER fail_second", vec![]).await.unwrap();

    store.apply(0, make_page()).await.unwrap();
    assert_eq!(store.cursor().await.unwrap(), 2);
    assert_eq!(app.local().repositories().clip_records().count().await.unwrap(), 2);
    assert!(store.pending().await.unwrap().is_empty());
}

#[tokio::test]
async fn armed_notifications_coalesce() {
    let dir = tempfile::tempdir().unwrap();
    let db = dir.path().join("test.db");
    let app = open(&db).await;
    let server = FakeServer::new();
    server.hold_pull.store(true, SeqCst);
    start(&app, server.clone()).await;
    eventually(async || server.pull_count.load(SeqCst) == 1).await;
    // 在同一轮尚未返回时重复提交唤醒，最多留下一次后续补拉。
    let triggers = app.sync().active.lock().await.as_ref().unwrap().triggers.clone().unwrap();
    for _ in 0..25 { triggers.request_pull(); }
    server.hold_pull.store(false, SeqCst);
    server.release_pull.notify_one();
    eventually(async || server.pull_count.load(SeqCst) == 2).await;
    tokio::time::sleep(Duration::from_millis(500)).await;
    assert_eq!(server.pull_count.load(SeqCst), 2);
    app.sync().disable().await;
}

#[tokio::test]
async fn an_enqueue_failure_rolls_back_local_changes_and_local_delete_removes_pending_upsert() {
    let dir = tempfile::tempdir().unwrap();
    let app = open(&dir.path().join("test.db")).await;
    sync_store::initialize(app.local().db()).await.unwrap();
    sync_store::initialize_history(app.local(), SyncLimits::default()).await.unwrap();
    let server = FakeServer::new();
    server.online.store(false, SeqCst);
    start(&app, server).await;
    let record = capture(&app, "keep pending").await;

    app.local().db().exec(
        "CREATE TRIGGER unavailable_queue BEFORE INSERT ON clip_sync_record
         BEGIN SELECT RAISE(ABORT,'queue unavailable'); END",
        vec![],
    ).await.unwrap();
    assert!(app.capture(CaptureInput::Text("must roll back"), &encode).await.is_err());
    assert!(app.sync().delete_everywhere(record.id.to_string()).await.is_err());
    assert_eq!(app.local().repositories().clip_records().count().await.unwrap(), 1);
    assert!(app.local().repositories().clip_records().find_by_id(record.id)
        .await.unwrap().is_some());

    ClipRecordService::new(app.local()).delete_record(record.id.to_string()).await.unwrap();
    assert_eq!(app.local().repositories().clip_records().count().await.unwrap(), 0);
    assert!(store(&app).pending().await.unwrap().is_empty());
    app.sync().disable().await;
}

/// 云端删除只能在该内容的本机新增确认之后回放，否则会提前清掉上传所需的本地记录。
#[tokio::test]
async fn remote_delete_waits_for_pending_upsert_before_advancing_cursor() {
    let path = tempfile::tempdir().unwrap();
    let app = open(&path.path().join("test.db")).await;
    capture(&app, "keep until confirmed").await;
    sync_store::initialize(app.local().db()).await.unwrap();
    sync_store::initialize_history(app.local(), SyncLimits::default()).await.unwrap();
    let store = store(&app);
    let pending = store.pending().await.unwrap();
    let delete = || PullPage {
        changes: vec![Change {
            sync_seq: 1, committed_at: 1,
            content_key: text_key("keep until confirmed"), action: Action::Delete,
        }],
        has_more: false,
    };
    assert!(matches!(store.apply(0, delete()).await, Err(SyncError::Retry(_))));
    assert_eq!(store.cursor().await.unwrap(), 0);
    assert!(store.pending().await.unwrap() == pending);
    store.confirm(&pending).await.unwrap();
    store.apply(0, delete()).await.unwrap();
    assert_eq!(store.cursor().await.unwrap(), 1);
    assert_eq!(app.local().repositories().clip_records().count().await.unwrap(), 0);
}

/// 通过实际本机备份、加密描述和两个会话验证原文件删除及断网重启后的补传。
#[tokio::test]
async fn file_attachment_retries_from_managed_backup() {
    let path_a = tempfile::tempdir().unwrap();
    let path_b = tempfile::tempdir().unwrap();
    let db_a = path_a.path().join("first.db");
    let server = FakeServer::new();
    server.attachment_enabled.store(true, SeqCst);

    let a = open(&db_a).await;
    start(&a, server.clone()).await;
    eventually(async || !a.sync().status().await.waiting_for_limits).await;
    a.sync().disable().await;

    server.online.store(false, SeqCst);
    start(&a, server.clone()).await;

    let source = path_a.path().join("test.txt");
    let bytes = vec![42u8; 1024];
    std::fs::write(&source, &bytes).unwrap();
    let paths = vec![source.to_string_lossy().into_owned()];
    let record = a.capture(CaptureInput::Files(Some(&paths)), &encode).await.unwrap().unwrap();
    std::fs::remove_file(&source).unwrap();
    let backup = desktop_app_core::records::resolve_resource(a.local(), &record.resources[0]).unwrap();
    assert_eq!(std::fs::read(backup).unwrap(), bytes);
    a.sync().disable().await;
    drop(a);

    let a = open(&db_a).await;
    let b = open(&path_b.path().join("second.db")).await;
    server.interrupt_attachment.store(true, SeqCst);
    server.lose_ack.store(true, SeqCst);
    server.online.store(true, SeqCst);
    start(&a, server.clone()).await;
    start(&b, server.clone()).await;
    eventually(async || a.sync().transfers.in_progress_count() == 1).await;
    eventually(async || b.local().repositories().clip_records()
        .find_by_content_key(&record.content_key).await.unwrap().is_some()).await;
    let attached = b.local().repositories().clip_records()
        .find_by_content_key(&record.content_key).await.unwrap().unwrap();
    let resource = &attached.resources[0];
    assert_eq!(resource.display_name, "test.txt");
    assert_eq!(std::fs::read(desktop_app_core::records::resolve_resource(b.local(), resource).unwrap()).unwrap(), bytes);
    assert_eq!(server.state.lock().unwrap().committed.len(), 1);
    assert!(server.state.lock().unwrap().objects.keys().next().unwrap().ends_with(".txt"));
    assert_eq!(store(&b).cursor().await.unwrap(), 1);
    eventually(async || {
        a.sync().card_states(vec![record.content_key.clone()]).await.unwrap().cards.get(&record.content_key)
            == Some(&CardSyncStatus::Uploaded)
    }).await;
    {
        let state = server.state.lock().unwrap();
        assert_eq!(state.upload_attempts.len(), 2);
        assert_eq!(state.upload_attempts[0], state.upload_attempts[1]);
    }
    a.sync().disable().await;
    a.sync().delete_everywhere(record.id.to_string()).await.unwrap();
    assert!(a.local().repositories().clip_records()
        .find_by_content_key(&record.content_key).await.unwrap().is_none());
    assert!(matches!(store(&a).pending().await.unwrap()[0].action, Action::Delete));
    start(&a, server.clone()).await;
    eventually(async || b.local().repositories().clip_records()
        .find_by_content_key(&record.content_key).await.unwrap().is_none()).await;
    a.sync().disable().await;
    b.sync().disable().await;
}

/// 下载中断时游标和记录均保持原值，重新获取整张图片后才提交。
#[tokio::test]
async fn image_download_retries_without_advancing_cursor() {
    use desktop_app_core::clip_core::content_key;
    let path = tempfile::tempdir().unwrap();
    let app = open(&path.path().join("images.db")).await;
    let server = FakeServer::new();
    let limits = SyncLimits { attachments: true, ..SyncLimits::default() };
    sync_store::initialize(app.local().db()).await.unwrap();
    sync_store::initialize_history(app.local(), limits).await.unwrap();
    let store = SqliteSyncStore {
        local: app.local.clone(), notify: Arc::new(|| {}),
        transport: server.clone(), transfers: Transfers::default(),
        encoder: Arc::new(encode),
    };

    // 真实的 1×1 PNG 用于验证落盘后字节保持完整。
    let bytes = vec![
        137,80,78,71,13,10,26,10,0,0,0,13,73,72,68,82,0,0,0,1,0,0,0,1,8,4,0,0,0,181,28,12,2,
        0,0,0,11,73,68,65,84,120,218,99,252,255,31,0,3,3,2,0,239,191,107,153,
        0,0,0,0,73,69,78,68,174,66,96,130,
    ];
    let manifest = Attachment {
        object_id: "11223344556677889900112233445566".into(), name: "clipboard.png".into(),
        sha256: content_key::bytes_hash(&bytes), size: bytes.len(),
    };
    server.state.lock().unwrap().objects.insert(manifest.object_id.clone(), bytes.clone());
    let key = content_key::image_key(&bytes);
    let change = Change {
        sync_seq: 1, committed_at: 1, content_key: key.clone(),
        action: Action::Upsert { ciphertext: encode(&serde_json::to_string(&manifest).unwrap()).unwrap() },
    };
    let page = || PullPage { changes: vec![change.clone()], has_more: false };
    server.interrupt_download.store(true, SeqCst);
    assert!(matches!(store.apply(0, page()).await, Err(SyncError::Retry(_))));
    assert_eq!(store.cursor().await.unwrap(), 0);
    assert!(app.local().repositories().clip_records().find_by_content_key(&key).await.unwrap().is_none());
    store.apply(0, page()).await.unwrap();
    assert_eq!(store.cursor().await.unwrap(), 1);
    let resource = app.local().repositories().clip_records().find_by_content_key(&key)
        .await.unwrap().unwrap().resources.remove(0);
    let location = desktop_app_core::records::resolve_resource(app.local(), &resource).unwrap();
    assert_eq!(std::fs::read(location).unwrap(), bytes);
}

/// 删除本机备份后，跳过新增附件，但继续提交显式跨端删除和后续文本。
#[tokio::test]
async fn missing_backup_does_not_block_later_actions() {
    let dir = tempfile::tempdir().unwrap();
    let app = open(&dir.path().join("test.db")).await;
    let server = FakeServer::new();
    server.attachment_enabled.store(true, SeqCst);
    start(&app, server.clone()).await;
    eventually(async || !app.sync().status().await.waiting_for_limits).await;
    app.sync().disable().await;
    server.online.store(false, SeqCst);
    start(&app, server.clone()).await;

    let source = dir.path().join("file.txt");
    std::fs::write(&source, b"managed copy").unwrap();
    let paths = vec![source.to_string_lossy().into_owned()];
    let file = app.capture(CaptureInput::Files(Some(&paths)), &encode).await.unwrap().unwrap();
    // 本机历史删除会回收副本，跨端删除仍按内容键登记。
    app.sync().delete_everywhere(file.id.to_string()).await.unwrap();
    let another_source = dir.path().join("another.txt");
    std::fs::write(&another_source, b"missing on disk").unwrap();
    let paths = vec![another_source.to_string_lossy().into_owned()];
    let missing = app.capture(CaptureInput::Files(Some(&paths)), &encode).await.unwrap().unwrap();
    let backup = desktop_app_core::records::resolve_resource(app.local(), &missing.resources[0]).unwrap();
    std::fs::remove_file(backup).unwrap();
    capture(&app, "later text").await;
    app.sync().disable().await;

    server.online.store(true, SeqCst);
    start(&app, server.clone()).await;
    eventually(async || store(&app).cursor().await.unwrap() == 2).await;
    assert!(store(&app).pending().await.unwrap().is_empty());
    assert!(app.local().repositories().clip_records().find_by_id(missing.id).await.unwrap().is_some());
    let state = server.state.lock().unwrap();
    assert_eq!(state.committed.len(), 2);
    assert_eq!(state.committed[0].content_key, file.content_key);
    assert!(matches!(state.committed[0].action, Action::Delete));
    assert_eq!(state.committed[1].content_key, text_key("later text"));
    assert!(state.objects.is_empty());
    assert!(source.exists());
    drop(state);
    app.sync().disable().await;
}

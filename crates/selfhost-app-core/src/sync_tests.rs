//! 用 SQLite 临时库和内存云端验证客户端闭环，不读取用户的剪贴板或正式数据库。
use super::*;
use crate::SelfhostContext;
use async_trait::async_trait;
use clip_sync_client::v2::{protocol::*, StreamEvent, SyncStore};
use desktop_app_core::clip_core::content_key::text_key;
use std::sync::{
    atomic::{AtomicBool, AtomicUsize, Ordering::SeqCst},
    Mutex as StdMutex,
};
use tokio::sync::{broadcast, Notify};

const ENDPOINT: &str = "https://sync.test/";
const TEST_KEY: &str = "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=";

/// 使用真实加密和解密，确认线上正文和 SQLite 快照采用同一套格式。
fn encode(value: &str) -> AppResult<String> {
    cipher().encrypt(value)
}

fn cipher() -> desktop_app_core::clip_core::crypto::ContentCipher {
    desktop_app_core::clip_core::crypto::ContentCipher::from_base64_key(TEST_KEY).unwrap()
}

async fn open(path: &std::path::Path) -> SelfhostContext {
    let local = LocalContext::open(path, None, Arc::new(|value: &str| cipher().decrypt(value)))
        .await.unwrap();
    SelfhostContext::new(local)
}

fn scope() -> Scope {
    Scope { scope_id: "private-home".into(), dataset_id: "dataset-1".into() }
}

/// 只实现客户端依赖的服务端契约。按 ID 幂等，模拟提交后的确认丢失和 SSE 断开。
struct FakeServer {
    scope: Scope,
    online: AtomicBool,
    lose_ack: AtomicBool,
    state: StdMutex<ServerState>,
    events: broadcast::Sender<()>,
    pull_count: AtomicUsize,
    hold_pull: AtomicBool,
    release_pull: Notify,
}

#[derive(Default)]
struct ServerState {
    committed: Vec<Operation>,
    upload_attempts: Vec<Vec<String>>,
    pull_after: Vec<i64>,
}

impl FakeServer {
    fn new(scope: Scope) -> Arc<Self> {
        Arc::new(Self {
            scope, online: AtomicBool::new(true), lose_ack: AtomicBool::new(false),
            state: StdMutex::default(), events: broadcast::channel(64).0,
            pull_count: AtomicUsize::new(0), hold_pull: AtomicBool::new(false),
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
    async fn scope(&self) -> Result<Scope, SyncError> {
        self.connected()?;
        Ok(self.scope.clone())
    }

    async fn upload(&self, request: &UploadRequest) -> Result<UploadResponse, SyncError> {
        self.connected()?;
        assert_eq!(request.scope, self.scope);
        let mut state = self.state.lock().unwrap();
        state.upload_attempts.push(request.operations.iter().map(|op| op.operation_id.clone()).collect());
        let mut acks = Vec::new();
        for op in &request.operations {
            let position = state.committed.iter().position(|stored| stored.operation_id == op.operation_id);
            let sync_seq = match position {
                Some(pos) => {
                    assert!(state.committed[pos] == *op, "同一个动作重试时数据发生变化");
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
        Ok(UploadResponse { scope: self.scope.clone(), acknowledgements: acks })
    }

    async fn pull(&self, request: &PullRequest) -> Result<PullPage, SyncError> {
        self.connected()?;
        assert_eq!(request.scope, self.scope);
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
            scope: self.scope.clone(), changes, has_more: state.committed.len() > index + 1,
        })
    }

    async fn listen(
        &self, scope: &Scope, notify: &(dyn Fn(StreamEvent) + Send + Sync),
    ) -> Result<(), SyncError> {
        self.connected()?;
        assert_eq!(*scope, self.scope);
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
    start_at(app, ENDPOINT, server).await;
}

async fn start_at(app: &SelfhostContext, endpoint: &str, server: Arc<FakeServer>) {
    app.sync().start(endpoint.into(), server, Arc::new(|| {})).await.unwrap();
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

async fn store_at(app: &SelfhostContext, endpoint: &str) -> SqliteSyncStore {
    SqliteSyncStore {
        local: app.local.clone(),
        binding: sync_store::load_binding(app.local().db(), endpoint).await.unwrap().unwrap(),
        notify: Arc::new(|| {}),
    }
}

#[tokio::test]
async fn offline_restart_keeps_order_and_retries_with_the_same_operation_ids() {
    let path_a = tempfile::tempdir().unwrap();
    let path_b = tempfile::tempdir().unwrap();
    let db_a = path_a.path().join("test.db");
    let server = FakeServer::new(scope());
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
    eventually(async || store_at(&a, ENDPOINT).await.pending().await.unwrap().is_empty()).await;
    a.sync().disable().await;
    drop(a);
    server.online.store(false, SeqCst);
    server.wake();
    let a = open(&db_a).await;
    start(&a, server.clone()).await;
    a.sync().delete_everywhere(original.id.to_string()).await.unwrap();
    capture(&a, "123").await;
    let pending = store_at(&a, ENDPOINT).await.pending().await.unwrap();
    assert!(matches!(pending[0].action, Action::Delete));
    assert!(matches!(&pending[1].action, Action::Upsert { ciphertext } if ciphertext != "123"));

    a.sync().disable().await;
    drop(a);
    server.lose_ack.store(true, SeqCst);
    server.online.store(true, SeqCst);
    let a = open(&db_a).await;
    start(&a, server.clone()).await;
    eventually(async || {
        let store = store_at(&a, ENDPOINT).await;
        store.pending().await.unwrap().is_empty() && store.cursor().await.unwrap() == 3
            && store_at(&b, ENDPOINT).await.cursor().await.unwrap() == 3
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

#[tokio::test]
async fn failed_page_rolls_back_records_and_cursor_and_does_not_enqueue() {
    let dir = tempfile::tempdir().unwrap();
    let app = open(&dir.path().join("test.db")).await;
    sync_store::initialize(app.local().db()).await.unwrap();
    sync_store::bind(app.local(), ENDPOINT, &scope()).await.unwrap();
    let store = store_at(&app, ENDPOINT).await;
    let make_page = || PullPage {
        scope: scope(),
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
async fn armed_notifications_coalesce_and_dataset_change_keeps_old_queue() {
    let dir = tempfile::tempdir().unwrap();
    let db = dir.path().join("test.db");
    let app = open(&db).await;
    let server = FakeServer::new(scope());
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

    let new_scope = Scope { dataset_id: "dataset-2".into(), ..scope() };
    let new_server = FakeServer::new(new_scope);
    start(&app, new_server.clone()).await;
    capture(&app, "仍在原队列").await;
    eventually(async || app.sync().status().await.upload == SessionStatus::Blocked).await;
    assert_eq!(new_server.pull_count.load(SeqCst), 0);
    assert!(new_server.state.lock().unwrap().upload_attempts.is_empty());
    assert_eq!(store_at(&app, ENDPOINT).await.pending().await.unwrap().len(), 1);
    app.sync().disable().await;
}

#[tokio::test]
async fn an_enqueue_failure_rolls_back_local_changes_and_local_delete_keeps_queue() {
    let dir = tempfile::tempdir().unwrap();
    let app = open(&dir.path().join("test.db")).await;
    sync_store::initialize(app.local().db()).await.unwrap();
    sync_store::bind(app.local(), ENDPOINT, &scope()).await.unwrap();
    let server = FakeServer::new(scope());
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
    let pending = store_at(&app, ENDPOINT).await.pending().await.unwrap();
    assert_eq!(pending.len(), 1);
    assert!(matches!(&pending[0].action, Action::Upsert { ciphertext }
        if cipher().decrypt(ciphertext).unwrap() == "keep pending"));
    app.sync().disable().await;
}

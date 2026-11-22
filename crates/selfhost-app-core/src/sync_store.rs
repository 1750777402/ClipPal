//! SQLite 同步仓储。待上传动作独立保存，本地历史删除不影响已入队的密文。
use async_trait::async_trait;
use clip_sync_client::v2::{protocol::*, SyncError, SyncStore};
use desktop_app_core::{
    clip_core::{
        content_key, domain::clip::{ClipRecord, TEXT_TYPE}, errors::{AppError, AppResult},
        storage::{begin_record_write, delete_in, save_in, RecordWriteHook},
    },
    cleanup::CleanupService,
    records::finish_record_removal,
    LocalContext,
};
use rbatis::{executor::Executor, RBatis};
use rbs::to_value;
use serde::Deserialize;
use std::sync::Arc;
use uuid::Uuid;

/// 保存于本机的范围。id 仅供同库关联，不会随 HTTP 请求传出。
#[derive(Deserialize)]
pub(crate) struct Binding {
    id: i64,
    scope_id: String,
    dataset_id: String,
}

impl Binding {
    /// 返回队列所绑定的本机范围主键。
    pub(crate) fn id(&self) -> i64 { self.id }

    /// 还原服务端给出的范围身份。
    pub(crate) fn scope(&self) -> Scope {
        Scope { scope_id: self.scope_id.clone(), dataset_id: self.dataset_id.clone() }
    }
}

/// 待上传动作的 SQLite 实体。自增 id 只用于本机队列排序。
#[derive(Deserialize)]
struct PendingRow {
    operation_id: String,
    content_key: String,
    created_at: i64,
    action: String,
    ciphertext: Option<String>,
}

impl PendingRow {
    /// 读取已存快照，重试时使用原始加密字节和操作 ID。
    fn into_operation(self) -> Result<Operation, SyncError> {
        let action = match (self.action.as_str(), self.ciphertext) {
            ("upsert", Some(ciphertext)) if !ciphertext.is_empty() && ciphertext.len() <= TEXT_LIMIT =>
                Action::Upsert { ciphertext },
            ("delete", None) => Action::Delete,
            _ => return Err(SyncError::Blocked("待上传动作的快照无效".into())),
        };
        Ok(Operation {
            operation_id: self.operation_id, content_key: self.content_key,
            created_at: self.created_at, action,
        })
    }
}

/// 自建扩展表仅在显式启用时创建。单机库初始化仍只包含自己的两张业务表。
pub(crate) async fn initialize(db: &RBatis) -> AppResult<()> {
    let tx = begin_record_write(db).await?;
    for sql in [
        "CREATE TABLE IF NOT EXISTS clip_sync_scope (
            id INTEGER PRIMARY KEY AUTOINCREMENT, endpoint TEXT NOT NULL UNIQUE,
            scope_id TEXT NOT NULL, dataset_id TEXT NOT NULL,
            pull_cursor INTEGER NOT NULL DEFAULT 0 CHECK(pull_cursor >= 0)
        )",
        "CREATE TABLE IF NOT EXISTS clip_sync_record (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            scope INTEGER NOT NULL REFERENCES clip_sync_scope(id),
            operation_id TEXT NOT NULL UNIQUE, content_key TEXT NOT NULL, created_at INTEGER NOT NULL,
            action TEXT NOT NULL CHECK(action IN ('upsert','delete')), ciphertext TEXT,
            CHECK((action='upsert' AND ciphertext IS NOT NULL) OR (action='delete' AND ciphertext IS NULL))
        )",
        "CREATE INDEX IF NOT EXISTS idx_clip_sync_pending ON clip_sync_record(scope,id)",
        "CREATE TABLE IF NOT EXISTS clip_sync_origin (
            record_id INTEGER PRIMARY KEY REFERENCES clip_record(id) ON DELETE CASCADE,
            scope INTEGER NOT NULL REFERENCES clip_sync_scope(id)
        )",
    ] {
        tx.exec(sql, vec![]).await?;
    }
    tx.commit().await?;
    Ok(())
}

/// 查询已有绑定。离线重启时依然可以向原范围登记本地动作。
pub(crate) async fn load_binding(db: &dyn Executor, endpoint: &str) -> AppResult<Option<Binding>> {
    let rows: Vec<Binding> = rbatis::decode(db.query(
        "SELECT id,scope_id,dataset_id FROM clip_sync_scope WHERE endpoint=?",
        vec![to_value!(endpoint)],
    ).await?)?;
    Ok(rows.into_iter().next())
}

/// 绑定和首次登记同事务提交。绑定存在就表示初始历史已经登记过。
pub(crate) async fn bind(local: &LocalContext, endpoint: &str, scope: &Scope) -> AppResult<Binding> {
    let _guard = local.record_write_lock().lock().await;
    let tx = begin_record_write(local.db()).await?;
    let binding = if let Some(binding) = load_binding(&tx, endpoint).await? {
        if binding.scope() != *scope {
            return Err(AppError::Config("云端同步范围或数据集已变更，已有队列已保留".into()));
        }
        binding
    } else {
        tx.exec("INSERT INTO clip_sync_scope(endpoint,scope_id,dataset_id) VALUES (?,?,?)",
            vec![to_value!(endpoint), to_value!(&scope.scope_id), to_value!(&scope.dataset_id)],
        ).await?;
        let binding = load_binding(&tx, endpoint).await?.ok_or_else(|| {
            AppError::System("未能读取刚创建的同步范围".into())
        })?;
        // 由 SQLite 流式登记现有文本，复制顺序沿用本地排序，避免一次加载全部密文。
        // 从其他部署下载的历史不作为新部署的本机历史上传。
        tx.exec(
            "INSERT INTO clip_sync_record(scope,operation_id,content_key,created_at,action,ciphertext)
             SELECT ?, LOWER(HEX(RANDOMBLOB(16))), r.content_key,r.last_copied_at,'upsert',r.content
             FROM clip_record r WHERE r.type='text'
             AND LENGTH(CAST(r.content AS BLOB)) BETWEEN 1 AND ?
             AND NOT EXISTS (SELECT 1 FROM clip_sync_origin o WHERE o.record_id=r.id)
             ORDER BY r.sort_order,r.id",
            vec![to_value!(binding.id), to_value!(TEXT_LIMIT as i64)],
        ).await?;
        // 已纳入这个范围的历史不应再跟随安装包切换被导入另一个部署。
        tx.exec(
            "INSERT INTO clip_sync_origin(record_id,scope)
             SELECT r.id, q.scope FROM clip_sync_record q
             JOIN clip_record r ON r.content_key=q.content_key WHERE q.scope=?",
            vec![to_value!(binding.id)],
        ).await?;
        binding
    };
    tx.commit().await?;
    Ok(binding)
}

/// 本地新增或显式跨端删除共用的事务扩展。只向已绑定的范围写入。
pub(crate) struct Enqueue {
    pub(crate) scope: i64,
    pub(crate) delete: bool,
}

#[async_trait]
impl RecordWriteHook for Enqueue {
    /// 文本密文直接用已保存的快照。没有资源或本地主键需要参与通信。
    async fn on_write(&self, tx: &dyn Executor, record: &ClipRecord) -> AppResult<()> {
        if self.delete && record.r#type != TEXT_TYPE {
            return Err(AppError::Config("首版跨端删除只支持文本记录".into()));
        }
        if record.r#type != TEXT_TYPE { return Ok(()); }
        let (action, ciphertext, created_at) = if self.delete {
            ("delete", None, now_millis()?)
        } else {
            let Some(ciphertext) = record.content.as_deref().filter(|value| {
                !value.is_empty() && value.len() <= TEXT_LIMIT
            }) else { return Ok(()); };
            ("upsert", Some(ciphertext), record.last_copied_at)
        };
        tx.exec(
            "INSERT INTO clip_sync_record(scope,operation_id,content_key,created_at,action,ciphertext)
             VALUES (?,?,?,?,?,?)",
            vec![to_value!(self.scope), to_value!(Uuid::new_v4().simple().to_string()),
                to_value!(&record.content_key), to_value!(created_at), to_value!(action),
                to_value!(ciphertext)],
        ).await?;
        if !self.delete {
            // 用户主动重新复制后，这条文本属于当前范围。
            tx.exec("INSERT INTO clip_sync_origin(record_id,scope) VALUES (?,?)
                     ON CONFLICT(record_id) DO UPDATE SET scope=excluded.scope",
                vec![to_value!(record.id), to_value!(self.scope)],
            ).await?;
        }
        Ok(())
    }
}

/// 统一转换时间为毫秒，操作顺序仍由 SQLite 自增主键决定。
fn now_millis() -> AppResult<i64> {
    let elapsed = std::time::UNIX_EPOCH.elapsed().map_err(|error| AppError::System(error.to_string()))?;
    i64::try_from(elapsed.as_millis()).map_err(|_| AppError::System("时间超出范围".into()))
}

/// 单个会话使用自己的 binding，所有查询和写入均限制在该范围。
pub(crate) struct SqliteSyncStore {
    pub(crate) local: Arc<LocalContext>,
    pub(crate) binding: Binding,
    pub(crate) notify: Arc<dyn Fn() + Send + Sync>,
}

#[async_trait]
impl SyncStore for SqliteSyncStore {
    /// SQL 先限定条数和累计字节，避免把整条积压队列一次读入内存。
    async fn pending(&self) -> Result<Vec<Operation>, SyncError> {
        let rows: Vec<PendingRow> = self.local.db().query_decode(
            "WITH batch AS (
                SELECT * FROM clip_sync_record WHERE scope=? ORDER BY id LIMIT ?
             ), sized AS (
                SELECT *, SUM(COALESCE(LENGTH(CAST(ciphertext AS BLOB)),0)) OVER (ORDER BY id) AS bytes FROM batch
             )
             SELECT operation_id,content_key,created_at,action,ciphertext FROM sized WHERE bytes<=? ORDER BY id",
            vec![to_value!(self.binding.id), to_value!(BATCH_LIMIT as i64), to_value!(BATCH_BYTES as i64)],
        ).await.map_err(db_retry)?;
        rows.into_iter().map(PendingRow::into_operation).collect()
    }

    /// 服务器完整确认后才清理快照，后续新增动作不会被这个批次误删。
    async fn confirm(&self, operations: &[Operation]) -> Result<(), SyncError> {
        let tx = begin_record_write(self.local.db()).await.map_err(db_retry)?;
        for operation in operations {
            tx.exec("DELETE FROM clip_sync_record WHERE scope=? AND operation_id=?",
                vec![to_value!(self.binding.id), to_value!(&operation.operation_id)],
            ).await.map_err(db_retry)?;
        }
        tx.commit().await.map_err(db_retry)?;
        Ok(())
    }

    async fn cursor(&self) -> Result<i64, SyncError> {
        cursor(self.local.db(), self.binding.id).await
    }

    /// 整页回放与游标共同提交。索引和界面在提交成功后接收变化。
    async fn apply(&self, after: i64, page: PullPage) -> Result<(), SyncError> {
        let decoded = page.changes.into_iter().map(|change| self.validate(change))
            .collect::<Result<Vec<_>, _>>()?;
        let Some((last, _)) = decoded.last() else { return Ok(()); };
        let lock = self.local.record_write_lock().lock().await;
        let tx = begin_record_write(self.local.db()).await.map_err(db_retry)?;
        if cursor(&tx, self.binding.id).await? != after {
            return Err(SyncError::Retry("本地补拉游标已变更，重新读取".into()));
        }

        let mut effects = Vec::new();
        for (change, plaintext) in &decoded {
            if let Some(effect) = self.replay(&tx, change, plaintext.as_deref()).await.map_err(db_retry)? {
                effects.push(effect);
            }
        }
        tx.exec("UPDATE clip_sync_scope SET pull_cursor=? WHERE id=?",
            vec![to_value!(last.sync_seq), to_value!(self.binding.id)],
        ).await.map_err(db_retry)?;
        tx.commit().await.map_err(db_retry)?;

        let changed = !effects.is_empty();
        for effect in effects {
            match effect {
                Effect::Add(id, plaintext) => {
                    if let Err(error) = self.local.search_engine().add(&id.to_string(), &plaintext).await {
                        log::error!("同步回放更新搜索索引失败：{error}");
                    }
                }
                Effect::Remove(record) => finish_record_removal(&self.local, &record).await,
            }
        }
        // 清理服务也会取得同一个锁，先释放当前事务后的索引更新锁。
        drop(lock);
        if changed {
            if let Err(error) = CleanupService::new(&self.local).try_clean().await {
                log::error!("同步回放清理本地容量失败：{error}");
            }
            (self.notify)();
        }
        Ok(())
    }
}

/// 一次已提交动作对应的本地索引和缓存操作。
enum Effect {
    Add(i64, String),
    Remove(ClipRecord),
}

impl SqliteSyncStore {
    /// 解密后重新计算内容键，验证共享密钥、动作内容和身份匹配才允许落库。
    fn validate(&self, change: Change) -> Result<(Change, Option<String>), SyncError> {
        let Some(hash) = change.content_key.strip_prefix("v1:text:") else {
            return Err(SyncError::Blocked("收到非文本同步动作".into()));
        };
        if hash.len() != 64 || !hash.bytes().all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte)) {
            return Err(SyncError::Blocked("文本同步内容键无效".into()));
        }
        let plaintext = match &change.action {
            Action::Upsert { ciphertext } if !ciphertext.is_empty() && ciphertext.len() <= TEXT_LIMIT => {
                let plaintext = self.local.decoder().decode(ciphertext).map_err(|_| {
                    SyncError::Blocked("无法解密同步文本，请检查各设备的内容密钥".into())
                })?;
                if plaintext.is_empty() || content_key::text_key(&plaintext) != change.content_key {
                    return Err(SyncError::Blocked("同步文本与内容键不匹配".into()));
                }
                Some(plaintext)
            }
            Action::Upsert { .. } => return Err(SyncError::Blocked("同步密文的大小无效".into())),
            Action::Delete => None,
        };
        Ok((change, plaintext))
    }

    /// 复用单机写入和物理删除，不调用本机采集扩展，因此不会重复上传远端动作。
    async fn replay(
        &self, tx: &dyn Executor, change: &Change, plaintext: Option<&str>,
    ) -> AppResult<Option<Effect>> {
        #[derive(Deserialize)]
        struct Existing { id: i64, origin: Option<i64> }
        let existing: Vec<Existing> = rbatis::decode(tx.query(
            "SELECT r.id,o.scope AS origin FROM clip_record r
             LEFT JOIN clip_sync_origin o ON r.id=o.record_id WHERE r.content_key=?",
            vec![to_value!(&change.content_key)],
        ).await?)?;
        let existing = existing.first();
        // 切换部署后，旧部署下载来的历史不会被新部署覆盖或删除。
        if existing.and_then(|record| record.origin).is_some_and(|origin| origin != self.binding.id) {
            return Ok(None);
        }
        match (&change.action, plaintext) {
            (Action::Upsert { ciphertext }, Some(plaintext)) => {
                let saved = save_in(tx, &ClipRecord {
                    content_key: change.content_key.clone(), r#type: TEXT_TYPE.into(),
                    content: Some(ciphertext.clone()), created_at: change.committed_at,
                    last_copied_at: change.committed_at, ..Default::default()
                }).await?;
                // 已存在的本机历史保留其本机来源。只有云端创建的记录才标记下载范围。
                if existing.is_none() {
                    tx.exec("INSERT INTO clip_sync_origin(record_id,scope) VALUES (?,?)",
                        vec![to_value!(saved.id), to_value!(self.binding.id)],
                    ).await?;
                }
                Ok(Some(Effect::Add(saved.id, plaintext.to_owned())))
            }
            (Action::Delete, _) => {
                if let Some(record) = existing {
                    return Ok(delete_in(tx, record.id).await?.map(Effect::Remove));
                }
                Ok(None)
            }
            _ => Err(AppError::System("回放前未验证同步文本".into())),
        }
    }
}

/// 严格读取存在的绑定，缺失时不能误以为游标归零。
async fn cursor(tx: &dyn Executor, id: i64) -> Result<i64, SyncError> {
    #[derive(Deserialize)]
    struct Cursor { pull_cursor: i64 }
    let rows: Vec<Cursor> = rbatis::decode(tx.query("SELECT pull_cursor FROM clip_sync_scope WHERE id=?",
        vec![to_value!(id)]).await.map_err(db_retry)?).map_err(db_retry)?;
    rows.into_iter().next().map(|row| row.pull_cursor)
        .ok_or_else(|| SyncError::Blocked("本地同步范围已丢失".into()))
}

/// 错误分类与数据分离，数据库日志不输出待上传正文或令牌。
fn db_retry(error: impl std::fmt::Display) -> SyncError {
    SyncError::Retry(format!("本地同步仓储失败：{error}"))
}

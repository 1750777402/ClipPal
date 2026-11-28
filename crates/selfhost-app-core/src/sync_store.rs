//! SQLite 同步仓储。同一张表保存待上传动作和上传确认，附件上传使用本机受管备份。
use crate::{attachment, transfers::{CardSyncStatus, Direction, IncomingAttachment, Transfers, CONCURRENT_FILES}};
use futures_util::future::join_all;
use async_trait::async_trait;
use clip_sync_client::{protocol::*, SyncError, SyncStore, SyncTransport};
use desktop_app_core::{
    clip_core::{
        content_key, domain::clip::{ClipRecord, ClipRecordResource, TEXT_TYPE}, errors::{AppError, AppResult},
        storage::{begin_record_write, delete_in, load_record_in, save_in, RecordWriteHook},
    },
    cleanup::CleanupService,
    capture::TextEncoder,
    records::finish_record_removal,
    LocalContext,
};
use rbatis::{executor::Executor, RBatis};
use rbs::to_value;
use serde::Deserialize;
use std::{collections::BTreeMap, sync::Arc};
use uuid::Uuid;

/// 本机保存一份补拉游标，同时缓存上次连接确认的附件开关。
#[derive(Deserialize)]
pub(crate) struct SyncState {
    attachments: i64,
}

impl SyncState {
    /// 离线重启时沿用上次确认的附件能力。
    pub(crate) fn offline_limits(&self) -> SyncLimits {
        SyncLimits { attachments: self.attachments != 0, ..SyncLimits::default() }
    }
}

/// 待上传动作的 SQLite 实体。自增 id 只用于本机队列排序。
#[derive(Deserialize)]
struct PendingRow {
    operation_id: String,
    content_key: String,
    created_at: i64,
    action: String,
    record_id: Option<i64>,
    text: Option<String>,
}

impl PendingRow {
    /// 请求使用入队时保存的动作标识、内容键和时间。
    fn into_operation(self, action: Action) -> Operation {
        Operation {
            operation_id: self.operation_id, content_key: self.content_key,
            created_at: self.created_at, action,
        }
    }
}

/// 按当前结构创建同步表和索引，重复启动时保留本机队列和游标。
pub(crate) async fn initialize(db: &RBatis) -> AppResult<()> {
    let tx = begin_record_write(db).await?;
    for sql in [
        "CREATE TABLE IF NOT EXISTS clip_sync_state (
            id INTEGER PRIMARY KEY CHECK(id = 1),
            attachments INTEGER NOT NULL DEFAULT 0,
            pull_cursor INTEGER NOT NULL DEFAULT 0 CHECK(pull_cursor >= 0)
        )",
        "CREATE TABLE IF NOT EXISTS clip_sync_record (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            operation_id TEXT NOT NULL UNIQUE, content_key TEXT NOT NULL, created_at INTEGER NOT NULL,
            action TEXT NOT NULL CHECK(action IN ('upsert','delete')),
            upload_status INTEGER NOT NULL DEFAULT 0,
            record_id INTEGER REFERENCES clip_record(id) ON DELETE CASCADE,
            CHECK((action='delete' AND upload_status=0 AND record_id IS NULL)
               OR (action='upsert' AND upload_status IN (0,1) AND record_id IS NOT NULL))
        )",
    ] {
        tx.exec(sql, vec![]).await?;
    }

    // 队列索引只覆盖待传动作，记录索引供卡片查询和级联删除使用。
    for sql in [
        "CREATE INDEX IF NOT EXISTS idx_clip_sync_pending_order ON clip_sync_record(id) WHERE upload_status=0",
        "CREATE INDEX IF NOT EXISTS idx_clip_sync_local_record ON clip_sync_record(record_id)",
    ] {
        tx.exec(sql, vec![]).await?;
    }
    tx.commit().await?;
    Ok(())
}

/// 已有状态允许客户端断网重启后继续入队。
pub(crate) async fn load_state(db: &dyn Executor) -> AppResult<Option<SyncState>> {
    let rows: Vec<SyncState> = rbatis::decode(db.query(
        "SELECT attachments FROM clip_sync_state WHERE id=1", vec![],
    ).await?)?;
    Ok(rows.into_iter().next())
}

/// 首次登记历史和创建同步状态一起提交。后续连接沿用已保存的游标。
pub(crate) async fn initialize_history(local: &LocalContext, limits: SyncLimits) -> AppResult<()> {
    let _guard = local.record_write_lock().lock().await;
    let tx = begin_record_write(local.db()).await?;
    let state = load_state(&tx).await?;
    if state.is_none() {
        // 用 SQLite 登记现有文本，避免一次加载所有密文。
        tx.exec("INSERT INTO clip_sync_state(id) VALUES (1)", vec![]).await?;
        tx.exec(
            "INSERT INTO clip_sync_record(operation_id,content_key,created_at,action,record_id)
             SELECT LOWER(HEX(RANDOMBLOB(16))), r.content_key,r.last_copied_at,'upsert',r.id
             FROM clip_record r WHERE r.type='text'
             AND LENGTH(CAST(r.content AS BLOB)) BETWEEN 40 AND ?
             AND LENGTH(r.content) / 4 * 3
                 - (CASE WHEN r.content LIKE '%==' THEN 2 WHEN r.content LIKE '%=' THEN 1 ELSE 0 END)
                 - 28 <= ?
             ORDER BY r.sort_order,r.id",
            vec![to_value!(TEXT_LIMIT as i64), to_value!(limits.max_record_bytes as i64)],
        ).await?;
    }
    // 首次开启附件能力时登记现有图片和单文件，跳过队列中已有的同内容新增。
    if limits.attachments && state.is_none_or(|state| state.attachments == 0) {
        #[derive(Deserialize)]
        struct Id { id: i64 }
        let candidates: Vec<Id> = rbatis::decode(tx.query(
            "SELECT r.id FROM clip_record r
             JOIN clip_record_resource a ON a.record_id=r.id
             WHERE r.type IN ('image','files') AND a.storage_kind='managed'
               AND a.byte_size BETWEEN 0 AND ?
               AND NOT EXISTS (SELECT 1 FROM clip_sync_record q
                               WHERE q.record_id=r.id)
               AND (SELECT COUNT(*) FROM clip_record_resource WHERE record_id=r.id)=1
             ORDER BY r.sort_order,r.id",
            vec![to_value!(limits.max_record_bytes as i64)],
        ).await?)?;
        let enqueue = Enqueue::Upsert { local, limits };
        for candidate in candidates {
            if let Some(record) = load_record_in(&tx, candidate.id).await? {
                enqueue.on_write(&tx, &record).await?;
            }
        }
    }
    tx.exec("UPDATE clip_sync_state SET attachments=? WHERE id=1",
        vec![to_value!(i64::from(limits.attachments))],
    ).await?;
    tx.commit().await?;
    Ok(())
}

/// 查询可见卡片的上传状态。同一条记录有新动作时，以最新的待传动作为准。
pub(crate) async fn card_uploads(
    db: &RBatis, keys: &[String],
) -> AppResult<BTreeMap<String, Option<String>>> {
    if keys.is_empty() { return Ok(BTreeMap::new()); }
    #[derive(Deserialize)]
    struct Row { content_key: String, operation_id: Option<String> }
    let placeholders = vec!["?"; keys.len()].join(",");
    // 有操作 ID 就是待上传。用 EXISTS 判断已上传，避免同一记录的多次确认返回重复卡片。
    let rows: Vec<Row> = db.query_decode(
        &format!("SELECT r.content_key, q.operation_id
                  FROM clip_record r
                  LEFT JOIN clip_sync_record q ON q.id=(
                      SELECT id FROM clip_sync_record WHERE record_id=r.id AND upload_status=0
                      ORDER BY id DESC LIMIT 1
                  )
                  WHERE r.content_key IN ({placeholders})
                    AND (q.id IS NOT NULL OR EXISTS (
                        SELECT 1 FROM clip_sync_record u WHERE u.record_id=r.id AND u.upload_status=1
                    ))"),
        keys.iter().map(|key| to_value!(key)).collect(),
    ).await?;
    Ok(rows.into_iter().map(|row| (row.content_key, row.operation_id)).collect())
}

/// 新增需要检查当前部署的类型和大小限制。
fn ineligible(
    local: &LocalContext, record: &ClipRecord, limits: SyncLimits,
) -> AppResult<Option<&'static str>> {
    let text_bytes = if record.r#type == TEXT_TYPE {
        let Some(ciphertext) = &record.content else { return Ok(Some("缺少文本内容")); };
        if ciphertext.len() > TEXT_LIMIT { return Ok(Some("文本超过同步大小上限")); }
        local.decoder().decode(ciphertext)?.len()
    } else {
        0
    };
    Ok(attachment::skip_reason(record, Some(limits), text_bytes))
}

/// 本地新增或显式跨端删除共用的事务扩展。
pub(crate) enum Enqueue<'a> {
    Upsert { local: &'a LocalContext, limits: SyncLimits },
    Delete,
}

#[async_trait]
impl RecordWriteHook for Enqueue<'_> {
    /// 与本机增删共用事务。新增关联记录，删除只保存内容键以便独立补传。
    async fn on_write(&self, tx: &dyn Executor, record: &ClipRecord) -> AppResult<()> {
        let operation_id = Uuid::new_v4().simple().to_string();
        let (action, created_at, record_id) = match self {
            Self::Delete => {
                // 目录是纯本机引用。其他记录按内容键删除，不受当前上传限制或副本状态影响。
                if attachment::kind(&record.content_key).is_err() { return Ok(()); }
                // 删除主记录会级联清理新增动作，独立登记的删除动作一直保留到确认。
                ("delete", now_millis()?, None)
            }
            Self::Upsert { local, limits } => {
                if ineligible(local, record, *limits)?.is_some() { return Ok(()); }
                ("upsert", record.last_copied_at, Some(record.id))
            }
        };
        tx.exec(
            "INSERT INTO clip_sync_record(operation_id,content_key,created_at,action,record_id)
             VALUES (?,?,?,?,?)",
            vec![to_value!(&operation_id), to_value!(&record.content_key), to_value!(created_at),
                to_value!(action), to_value!(record_id)],
        ).await?;
        Ok(())
    }
}

/// 统一转换时间为毫秒，操作顺序仍由 SQLite 自增主键决定。
fn now_millis() -> AppResult<i64> {
    let elapsed = std::time::UNIX_EPOCH.elapsed().map_err(|error| AppError::System(error.to_string()))?;
    i64::try_from(elapsed.as_millis()).map_err(|_| AppError::System("时间超出范围".into()))
}

/// 管理本机待传动作和补拉游标。
pub(crate) struct SqliteSyncStore {
    pub(crate) local: Arc<LocalContext>,
    pub(crate) notify: Arc<dyn Fn() + Send + Sync>,
    pub(crate) transport: Arc<dyn SyncTransport>,
    pub(crate) transfers: Transfers,
    pub(crate) encoder: Arc<dyn TextEncoder>,
}

#[async_trait]
impl SyncStore for SqliteSyncStore {
    /// 清理缺失附件后继续扫描，整批缺失时也会处理后面的动作。
    async fn pending(&self) -> Result<Vec<Operation>, SyncError> {
        loop {
            let operations = self.next_batch().await?;
            if operations.is_empty() { return Ok(Vec::new()); }
            let batch = self.upload_attachments(operations).await?;
            if !batch.is_empty() { return Ok(batch); }
        }
    }

    fn uploading(&self, operations: &[Operation], active: bool) {
        self.transfers.submitting(operations, active);
    }

    /// 收到完整云端确认后更新新增状态，或清理对应的待传删除动作。
    async fn confirm(&self, operations: &[Operation]) -> Result<(), SyncError> {
        let tx = begin_record_write(self.local.db()).await.map_err(db_retry)?;
        for operation in operations {
            match &operation.action {
                Action::Upsert { .. } => {
                    tx.exec("UPDATE clip_sync_record SET upload_status=1
                             WHERE operation_id=? AND action='upsert' AND upload_status=0",
                        vec![to_value!(&operation.operation_id)],
                    ).await.map_err(db_retry)?;
                }
                Action::Delete => {
                    tx.exec("DELETE FROM clip_sync_record
                             WHERE operation_id=? AND action='delete' AND upload_status=0",
                        vec![to_value!(&operation.operation_id)],
                    ).await.map_err(db_retry)?;
                }
            }
        }
        tx.commit().await.map_err(db_retry)?;
        Ok(())
    }

    async fn cursor(&self) -> Result<i64, SyncError> {
        cursor(self.local.db()).await
    }

    /// 整页回放与游标共同提交。索引和界面在提交成功后接收变化。
    async fn apply(&self, after: i64, page: PullPage) -> Result<(), SyncError> {
        // 下载在写事务外执行。最多同时保留两个附件的字节，其余已完成的文件由守卫管理。
        let attachments = page.changes.iter().map(|change| {
            let kind = attachment::kind(&change.content_key)?;
            match &change.action {
                Action::Upsert { ciphertext } if kind != TEXT_TYPE =>
                    attachment::decode(&self.local, &change.content_key, ciphertext).map(Some),
                _ => Ok(None),
            }
        }).collect::<Result<Vec<_>, SyncError>>()?;

        self.transfers.clear_downloads();
        for (change, manifest) in page.changes.iter().zip(&attachments) {
            if let Some(manifest) = manifest {
                self.transfers.queue_download(change.sync_seq.to_string(), IncomingAttachment {
                    content_key: change.content_key.clone(),
                    name: manifest.name.clone(),
                    created: change.committed_at,
                    status: CardSyncStatus::PendingDownload,
                });
            }
        }
        let mut decoded = Vec::with_capacity(page.changes.len());
        let mut changes = page.changes.into_iter().zip(attachments);
        loop {
            let group: Vec<_> = changes.by_ref().take(CONCURRENT_FILES).collect();
            if group.is_empty() { break; }
            let jobs = group.into_iter().map(|(change, attachment)| async move {
                let prepared = self.prepare_change(&change, attachment.as_ref()).await?;
                Ok::<_, SyncError>((change, prepared))
            });
            for result in join_all(jobs).await { decoded.push(result?); }
        }
        let Some((last, _)) = decoded.last() else { return Ok(()); };
        let cursor_after = last.sync_seq;
        let lock = self.local.record_write_lock().lock().await;
        let tx = begin_record_write(self.local.db()).await.map_err(db_retry)?;
        if cursor(&tx).await? != after {
            return Err(SyncError::Retry("本地补拉游标已变更，重新读取".into()));
        }

        let mut effects = Vec::new();
        let mut used_attachments = Vec::new();
        for (index, (change, prepared)) in decoded.iter().enumerate() {
            if matches!(prepared, PreparedChange::Delete) {
                // 先补传同内容的本机新增，再按云端顺序重放删除。提前删除会带走上传所需的副本。
                #[derive(Deserialize)]
                struct Pending { has_pending: i64 }
                let rows: Vec<Pending> = rbatis::decode(tx.query(
                    "SELECT EXISTS(SELECT 1 FROM clip_record r
                     JOIN clip_sync_record q ON q.record_id=r.id
                     WHERE r.content_key=? AND q.upload_status=0) AS has_pending",
                    vec![to_value!(&change.content_key)],
                ).await.map_err(db_retry)?).map_err(db_retry)?;
                if rows[0].has_pending != 0 {
                    return Err(SyncError::Retry("等待同内容的本机新增确认后回放云端删除".into()));
                }
            }
            if let Some(effect) = self.replay(&tx, change, prepared).await.map_err(db_retry)? {
                if matches!(prepared, PreparedChange::Attachment(_)) {
                    used_attachments.push(index);
                }
                effects.push(effect);
            }
        }
        tx.exec("UPDATE clip_sync_state SET pull_cursor=? WHERE id=1",
            vec![to_value!(cursor_after)],
        ).await.map_err(db_retry)?;
        tx.commit().await.map_err(db_retry)?;

        self.transfers.clear_downloads();
        for index in used_attachments {
            if let PreparedChange::Attachment(file) = &mut decoded[index].1 {
                file.keep();
            }
        }
        let changed = !effects.is_empty();
        for effect in effects {
            match effect {
                Effect::Add(id, plaintext, replaced) => {
                    if !plaintext.is_empty() {
                        if let Err(error) = self.local.search_engine().add(&id.to_string(), &plaintext).await {
                            log::error!("同步回放更新搜索索引失败：{error}");
                        }
                    }
                    if let Some(old) = replaced {
                        desktop_app_core::cleanup::delete_managed_resources(&self.local, &old).await;
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
    Add(i64, String, Option<ClipRecord>),
    Remove(ClipRecord),
}

/// 已验证的回放内容。附件文件在事务开始前下载并落盘。
enum PreparedChange {
    Text(String),
    Attachment(attachment::Prepared),
    Delete,
}

impl SqliteSyncStore {
    /// SQL 先限定条数和累计字节，避免把整条积压队列一次读入内存。
    async fn next_batch(&self) -> Result<Vec<PendingRow>, SyncError> {
        self.local.db().query_decode(
            "WITH batch AS (
                SELECT q.id,q.operation_id,q.content_key,q.created_at,q.action,r.id AS record_id,
                    CASE WHEN r.type='text' AND LENGTH(CAST(r.content AS BLOB)) BETWEEN 1 AND ?
                         THEN r.content END AS text
                FROM clip_sync_record q
                LEFT JOIN clip_record r ON r.id=q.record_id
                WHERE q.upload_status=0 ORDER BY q.id LIMIT ?
             ), sized AS (
                SELECT *, SUM(COALESCE(LENGTH(CAST(text AS BLOB)),0)) OVER (ORDER BY id) AS bytes FROM batch
             )
             SELECT operation_id,content_key,created_at,action,record_id,text FROM sized
             WHERE bytes<=? ORDER BY id",
            vec![to_value!(TEXT_LIMIT as i64), to_value!(BATCH_LIMIT as i64), to_value!(BATCH_BYTES as i64)],
        ).await.map_err(db_retry)
    }

    /// OSS 可并行传输，成功的动作仍按原顺序提交。缺失的附件只移除对应新增。
    async fn upload_attachments(&self, rows: Vec<PendingRow>) -> Result<Vec<Operation>, SyncError> {
        let mut batch = Vec::new();
        let mut attachments = Vec::new();
        let mut attachment_bytes = 0;
        let mut ciphertext_bytes = 0;
        for mut row in rows {
            let (action, attachment) = if row.action == "delete" {
                (Action::Delete, None)
            } else if let Some(ciphertext) = row.text.take() {
                (Action::Upsert { ciphertext }, None)
            } else {
                let Some(record_id) = row.record_id else {
                    self.discard_unavailable(&row.operation_id).await?;
                    continue;
                };
                let Some(manifest) = self.attachment_manifest(record_id, &row).await? else {
                    self.discard_unavailable(&row.operation_id).await?;
                    continue;
                };
                if attachment_bytes + manifest.size > BATCH_BYTES {
                    break;
                }
                let description = serde_json::to_string(&manifest).map_err(db_retry)?;
                let ciphertext = self.encoder.encode(&description).map_err(db_retry)?;
                (Action::Upsert { ciphertext }, Some((record_id, manifest)))
            };
            if let Action::Upsert { ciphertext } = &action {
                if ciphertext_bytes + ciphertext.len() > BATCH_BYTES {
                    break;
                }
                ciphertext_bytes += ciphertext.len();
            }
            if let Some((record_id, manifest)) = attachment {
                attachment_bytes += manifest.size;
                attachments.push((batch.len(), record_id, manifest));
            }
            batch.push(Some(row.into_operation(action)));
        }

        for group in attachments.chunks(CONCURRENT_FILES) {
            let results = {
                let jobs = group.iter().map(|(index, record_id, manifest)| {
                    let operation = batch[*index].as_ref().expect("批次中的附件只处理一次");
                    self.upload_attachment(*record_id, operation, manifest)
                });
                join_all(jobs).await
            };
            let mut error = None;
            for ((index, _, _), result) in group.iter().zip(results) {
                match result {
                    Ok(true) => {}
                    Ok(false) => {
                        let operation = batch[*index].take().expect("缺失附件仍在批次中");
                        self.discard_unavailable(&operation.operation_id).await?;
                    }
                    Err(failure) => { error.get_or_insert(failure); }
                }
            }
            if let Some(error) = error { return Err(error); }
        }
        Ok(batch.into_iter().flatten().collect())
    }

    /// 根据当前本机资源重建附件描述，保证发给 OSS 的字节仍使用该内容键。
    async fn attachment_manifest(&self, record_id: i64, row: &PendingRow) -> Result<Option<Attachment>, SyncError> {
        let record = self.local.repositories().clip_records().find_by_id(record_id).await.map_err(db_retry)?;
        let Some(record) = record else { return Ok(None); };
        let [resource] = record.resources.as_slice() else { return Ok(None); };
        let (Some(sha256), Some(size)) = (
            resource.content_hash.as_ref(),
            resource.byte_size.and_then(|size| usize::try_from(size).ok()),
        ) else { return Ok(None); };
        if resource.storage_kind != "managed" || !attachment::valid_name(&resource.display_name)
            || size > DEFAULT_RECORD_BYTES || !attachment::digest(sha256)
            || attachment::content_identity(&record.r#type, sha256).as_deref() != Some(row.content_key.as_str())
        {
            return Ok(None);
        }
        Ok(Some(Attachment {
            object_id: attachment::object_id(&row.operation_id, &resource.display_name),
            name: resource.display_name.clone(), sha256: sha256.clone(), size,
        }))
    }

    /// 源记录或副本缺失时只清理新增动作，删除动作仍等待云端确认。
    async fn discard_unavailable(&self, operation_id: &str) -> Result<(), SyncError> {
        self.local.db().exec("DELETE FROM clip_sync_record
                              WHERE operation_id=? AND action='upsert' AND upload_status=0",
            vec![to_value!(operation_id)],
        ).await.map_err(db_retry)?;
        self.transfers.forget_upload(operation_id);
        log::warn!("跳过缺失或已损坏的本地同步内容，操作 ID：{operation_id}");
        Ok(())
    }

    /// 一次传输重试仍使用同一个操作标识。读取失败时区分文件缺失和临时 IO 错误。
    async fn upload_attachment(&self, record_id: i64, operation: &Operation, manifest: &Attachment) -> Result<bool, SyncError> {
        let id = &operation.operation_id;
        let Some(bytes) = self.read_backup(record_id, manifest).await? else {
            return Ok(false);
        };
        self.transfers.run(Direction::Upload, id.clone(), manifest, |progress| {
            self.transport.put_attachment(manifest, bytes, progress)
        }).await?;
        Ok(true)
    }

    /// 按本机记录 ID 找到受管副本，读取后校验大小和摘要。
    async fn read_backup(&self, record_id: i64, manifest: &Attachment) -> Result<Option<Vec<u8>>, SyncError> {
        use std::io::ErrorKind::NotFound;
        use tokio::io::AsyncReadExt;

        // 解析和打开文件期间，采集、清理不会并发替换该记录的资源。
        let guard = self.local.record_write_lock().lock().await;
        let record = self.local.repositories().clip_records().find_by_id(record_id).await.map_err(db_retry)?;
        let Some([resource]) = record.as_ref().map(|record| record.resources.as_slice()) else { return Ok(None); };
        if resource.storage_kind != "managed" || resource.content_hash.as_deref() != Some(&manifest.sha256) {
            return Ok(None);
        }
        let path = match desktop_app_core::records::resolve_resource(&self.local, resource) {
            Ok(path) => path,
            Err(AppError::Io(error)) if error.kind() == NotFound => return Ok(None),
            Err(error) => return Err(db_retry(error)),
        };
        let mut file = match tokio::fs::File::open(path).await {
            Ok(file) => file,
            Err(error) if error.kind() == NotFound => return Ok(None),
            Err(error) => return Err(db_retry(error)),
        };
        drop(guard);
        let mut bytes = Vec::with_capacity(manifest.size);
        (&mut file).take(manifest.size as u64 + 1).read_to_end(&mut bytes).await.map_err(db_retry)?;
        if bytes.len() != manifest.size || content_key::bytes_hash(&bytes) != manifest.sha256 {
            return Ok(None);
        }
        Ok(Some(bytes))
    }

    /// 解密并验证内容。附件下载失败时整页仍从原游标重试。
    async fn prepare_change(&self, change: &Change, manifest: Option<&Attachment>) -> Result<PreparedChange, SyncError> {
        let kind = attachment::kind(&change.content_key)?;
        let Action::Upsert { ciphertext } = &change.action else {
            return Ok(PreparedChange::Delete);
        };
        if ciphertext.is_empty() || ciphertext.len() > TEXT_LIMIT {
            return Err(SyncError::Blocked("同步密文的大小无效".into()));
        }
        if kind == TEXT_TYPE {
            let plaintext = self.local.decoder().decode(ciphertext).map_err(|_| {
                SyncError::Blocked("无法解密同步文本，请检查各设备的内容密钥".into())
            })?;
            if plaintext.is_empty() || plaintext.len() > DEFAULT_RECORD_BYTES
                || content_key::text_key(&plaintext) != change.content_key
            {
                return Err(SyncError::Blocked("同步文本与内容键不匹配".into()));
            }
            return Ok(PreparedChange::Text(plaintext));
        }
        let manifest = manifest.ok_or_else(|| SyncError::Blocked("附件描述缺失".into()))?;
        let file = self.transfers.run(Direction::Download, change.sync_seq.to_string(), manifest, |progress| async move {
            let bytes = self.transport.get_attachment(manifest, progress.as_ref()).await?;
            attachment::prepare(&self.local, kind, manifest, bytes).await
        }).await?;
        Ok(PreparedChange::Attachment(file))
    }

    /// 复用单机写入和物理删除，不调用本机采集扩展，因此不会重复上传远端动作。
    async fn replay(
        &self, tx: &dyn Executor, change: &Change, prepared: &PreparedChange,
    ) -> AppResult<Option<Effect>> {
        #[derive(Deserialize)]
        struct Existing { id: i64 }
        let existing: Vec<Existing> = rbatis::decode(tx.query(
            "SELECT id FROM clip_record WHERE content_key=?",
            vec![to_value!(&change.content_key)],
        ).await?)?;
        let existing = existing.first();
        if matches!(prepared, PreparedChange::Delete) {
            return match existing {
                Some(record) => Ok(delete_in(tx, record.id).await?.map(Effect::Remove)),
                None => Ok(None),
            };
        }

        let kind = attachment::kind(&change.content_key).map_err(|error| AppError::Config(error.to_string()))?;
        let (content, resources, plaintext, replaced) = match prepared {
            PreparedChange::Text(plaintext) => {
                let Action::Upsert { ciphertext } = &change.action else {
                    return Err(AppError::System("文本回放缺少密文".into()));
                };
                (Some(ciphertext.clone()), vec![], plaintext.clone(), None)
            }
            PreparedChange::Attachment(file) => {
                // 已保存在本机的同内容副本优先，记住原来的文件名和副本路径。
                let mut old = None;
                if let Some(existing) = existing {
                    let resources: Vec<ClipRecordResource> = rbatis::decode(tx.query(
                        "SELECT * FROM clip_record_resource WHERE record_id=?",
                        vec![to_value!(existing.id)],
                    ).await?)?;
                    if resources.iter().any(|resource| resource.storage_kind == "managed"
                        && desktop_app_core::records::resolve_resource(&self.local, resource)
                            .is_ok_and(|path| path.is_file()))
                    {
                        return Ok(None);
                    }
                    old = Some(ClipRecord { id: existing.id, resources, ..Default::default() });
                }
                (None, vec![file.resource.clone()],
                    if kind == "files" { file.resource.display_name.clone() } else { String::new() }, old)
            }
            PreparedChange::Delete => unreachable!(),
        };
        let saved = save_in(tx, &ClipRecord {
            content_key: change.content_key.clone(), r#type: kind.into(),
            content, resources, created_at: change.committed_at,
            last_copied_at: change.committed_at, ..Default::default()
        }).await?;
        Ok(Some(Effect::Add(saved.id, plaintext, replaced)))
    }
}

/// 从已初始化的本机状态获取提交过的补拉游标。
async fn cursor(tx: &dyn Executor) -> Result<i64, SyncError> {
    #[derive(Deserialize)]
    struct Cursor { pull_cursor: i64 }
    let rows: Vec<Cursor> = rbatis::decode(tx.query("SELECT pull_cursor FROM clip_sync_state WHERE id=1",
        vec![]).await.map_err(db_retry)?).map_err(db_retry)?;
    rows.into_iter().next().map(|row| row.pull_cursor)
        .ok_or_else(|| SyncError::Blocked("本地同步状态已丢失".into()))
}

/// 错误分类与数据分离，数据库日志不输出待上传正文或令牌。
fn db_retry(error: impl std::fmt::Display) -> SyncError {
    SyncError::Retry(format!("本地同步仓储失败：{error}"))
}

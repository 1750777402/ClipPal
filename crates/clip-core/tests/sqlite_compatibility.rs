#![cfg(feature = "sqlite")]

use clip_core::{domain::clip::ClipRecord, repositories::ClipRecordRepository, storage::{connect, SqliteClipRecordRepository}};
use serde_json::json;

struct DatabaseFile(std::path::PathBuf);
impl DatabaseFile {
    fn new() -> Self {
        Self(std::env::temp_dir().join(format!("clip-pal-迁移-{}.db", uuid::Uuid::new_v4())))
    }
}
impl Drop for DatabaseFile {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

fn record(id: &str, sort: i32) -> ClipRecord {
    ClipRecord {
        id: id.to_owned(), r#type: "Text".into(), content: json!("legacy-ciphertext=="),
        md5_str: format!("md5-{id}"), created: 100, os_type: "macOS".into(), sort,
        pinned_flag: 0, sync_flag: Some(0), sync_time: Some(0),
        device_id: Some("legacy-device".into()), version: Some(1), del_flag: Some(0),
        cloud_source: Some(0), skip_type: Some(2), local_file_path: None,
    }
}

#[tokio::test]
async fn preserves_existing_database_records_and_repository_behaviour() {
    let path = DatabaseFile::new();
    let db = connect(&path.0).await.unwrap();
    // 按旧版本 SQL 格式写入，验证迁移后的仓储可以读取已有内容。
    db.exec("INSERT INTO clip_record (id, type, content, md5_str, created, os_type, sort, pinned_flag, del_flag, cloud_source) VALUES ('legacy', 'Text', '原有密文==', 'legacy-md5', 1, 'macOS', 0, 0, 0, 0)", vec![]).await.unwrap();
    let repo = SqliteClipRecordRepository::new(db.clone());
    assert_eq!(repo.find_by_id("legacy").await.unwrap().unwrap().content, json!("原有密文=="));

    let first = record("first", 1);
    repo.insert(&first).await.unwrap();
    repo.insert(&record("second", 2)).await.unwrap();
    assert_eq!(serde_json::to_value(repo.find_by_id("first").await.unwrap().unwrap()).unwrap(), serde_json::to_value(&first).unwrap());
    repo.set_pinned("first", 1).await.unwrap();
    repo.set_pinned("second", 1).await.unwrap();
    assert!(!repo.find_by_id("first").await.unwrap().unwrap().is_pinned());
    assert!(repo.list(10, 0).await.unwrap()[0].is_pinned());
    repo.mark_deleted(&["second".into()]).await.unwrap();
    assert!(repo.find_by_id("second").await.unwrap().unwrap().is_deleted());
    assert_eq!(repo.list(10, 0).await.unwrap().len(), 2);
    assert_eq!(repo.list_by_ids(&["first".into()], 10, 0).await.unwrap().len(), 1);
    assert!(repo.list_by_ids(&[], 10, 0).await.unwrap().is_empty());

    // 再次连接必须保持原内容，不能因迁移丢失记录或重建表。
    let reopened = SqliteClipRecordRepository::new(connect(&path.0).await.unwrap());
    assert_eq!(reopened.count_active().await.unwrap(), 2);
    assert_eq!(reopened.find_by_id("first").await.unwrap().unwrap().content, first.content);
}

#[tokio::test]
async fn merged_cloud_records_preserve_order_and_failed_insert_rolls_back_sort() {
    let path = DatabaseFile::new();
    let repo = SqliteClipRecordRepository::new(connect(&path.0).await.unwrap());
    let mut existing = record("existing", 0);
    existing.sync_time = Some(200);
    repo.insert(&existing).await.unwrap();
    let mut remote = record("remote", 0);
    remote.sync_time = Some(100);
    assert_eq!(repo.insert_batch_merge_by_sync_time(vec![remote.clone()]).await.unwrap(), (1, 0));
    assert_eq!(repo.find_by_id("existing").await.unwrap().unwrap().sort, 1);
    assert_eq!(repo.find_by_id("remote").await.unwrap().unwrap().sort, 0);
    assert_eq!(repo.insert_batch_merge_by_sync_time(vec![remote]).await.unwrap(), (0, 1));
    assert_eq!(repo.find_by_id("existing").await.unwrap().unwrap().sort, 1);
}

#[tokio::test]
async fn restore_deleted_replaces_skip_reason_and_pin_errors_are_not_swallowed() {
    let path = DatabaseFile::new();
    let db = connect(&path.0).await.unwrap();
    let repo = SqliteClipRecordRepository::new(db.clone());
    let mut restored = record("restored", 1);
    repo.insert(&restored).await.unwrap();
    repo.mark_deleted(&[restored.id.clone()]).await.unwrap();
    restored.skip_type = None;
    repo.restore_deleted(&restored.id, &restored).await.unwrap();
    assert_eq!(repo.find_by_id(&restored.id).await.unwrap().unwrap().skip_type, None);
    repo.insert(&record("pinned", 2)).await.unwrap();
    repo.set_pinned("pinned", 1).await.unwrap();
    // 强制第二次更新失败，既要返回错误，也要回滚前一次取消置顶。
    db.exec("CREATE TRIGGER reject_pin BEFORE UPDATE OF pinned_flag ON clip_record WHEN NEW.id = 'restored' AND NEW.pinned_flag = 1 BEGIN SELECT RAISE(ABORT, 'test pin failure'); END", vec![]).await.unwrap();
    assert!(repo.set_pinned("restored", 1).await.is_err());
    assert!(repo.find_by_id("pinned").await.unwrap().unwrap().is_pinned());
}

use std::{path::{Path, PathBuf}, sync::Arc};
use clip_core::{crypto::ContentCipher, domain::clip::ClipRecord, errors::{AppError, AppResult}};
use desktop_app_core::{LocalContext, capture::{CaptureInput, CapturePolicy, CaptureService, DeviceIdentity}, cleanup::{CleanupService, delete_managed_resources}, dto::QueryParam, records::ClipRecordService};

const KEY: &str = "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=";
struct TempDir(PathBuf);
impl TempDir {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!("clippal-local-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(path.join("resources")).unwrap();
        Self(path)
    }
}
impl Drop for TempDir { fn drop(&mut self) { let _ = std::fs::remove_dir_all(&self.0); } }
struct LocalPolicy;
#[async_trait::async_trait]
impl CapturePolicy for LocalPolicy {
    fn cached_sync_file_limit(&self) -> u64 { 0 }
    async fn file_copy_limit(&self) -> u64 { u64::MAX }
}
fn identity() -> DeviceIdentity { DeviceIdentity { device_id: "test-device".into(), os_type: "test-os".into() } }
fn encode(value: &str) -> AppResult<String> { ContentCipher::from_base64_key(KEY)?.encrypt(value) }
async fn open(dir: &Path) -> LocalContext {
    let decode = |value: &str| ContentCipher::from_base64_key(KEY)?.decrypt(value);
    LocalContext::open(&dir.join("test.db"), Some(dir.join("settings.json")), Arc::new(decode)).await.unwrap()
        .with_resources_dir(Some(dir.join("resources")))
}
fn query(search: Option<&str>) -> QueryParam { QueryParam { page: 1, size: 100, search: search.map(str::to_owned) } }

#[tokio::test]
async fn encrypted_text_dedup_search_preview_and_permanent_delete() {
    let dir = TempDir::new();
    let local = open(&dir.0).await;
    let id = identity();
    let capture = CaptureService::new(&local, &encode, &id, &LocalPolicy);
    assert!(capture.capture(CaptureInput::Text("  \n ")).await.unwrap().is_none());
    let content = "中文🙂总结".repeat(1000);
    let first = capture.capture(CaptureInput::Text(&format!("  {content} \n"))).await.unwrap().unwrap();
    assert_ne!(first.content.as_str().unwrap(), content);
    assert_eq!(ContentCipher::from_base64_key(KEY).unwrap().decrypt(first.content.as_str().unwrap()).unwrap(), content);
    capture.capture(CaptureInput::Text("second")).await.unwrap();
    assert!(capture.capture(CaptureInput::Text(&content)).await.unwrap().is_none());
    let records = ClipRecordService::new(&local);
    let found = records.query_clip_records(query(Some("总结"))).await.unwrap();
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].id, first.id);
    assert!(found[0].content_truncated);
    assert!(content.starts_with(&found[0].content));
    assert_eq!(records.get_full_text_content(first.id.clone()).await.unwrap().content, content);
    assert_eq!(records.query_clip_records(query(None)).await.unwrap()[0].id, first.id);
    local.initialize_search_index().await;
    assert_eq!(records.query_clip_records(query(Some("总结"))).await.unwrap().len(), 1);
    records.delete_record(first.id.clone(), false).await.unwrap();
    assert!(local.repositories().clip_records().find_by_id(&first.id).await.unwrap().is_none());
    assert!(local.search_engine().search("总结").await.is_empty());
}

#[tokio::test]
async fn image_and_file_cache_lifecycle_preserves_source_files() {
    let dir = TempDir::new();
    let local = open(&dir.0).await;
    let id = identity();
    let capture = CaptureService::new(&local, &encode, &id, &LocalPolicy);
    let records = ClipRecordService::new(&local);
    let image = vec![1, 2, 3, 4];
    let saved = capture.capture(CaptureInput::Image(Some(&image))).await.unwrap().unwrap();
    let image_path = PathBuf::from(records.get_image_path(&saved.id).await.unwrap().file_path);
    assert_eq!(std::fs::read(&image_path).unwrap(), image);
    assert!(capture.capture(CaptureInput::Image(Some(&image))).await.unwrap().is_none());
    records.delete_record(saved.id, false).await.unwrap();
    assert!(!image_path.exists());

    let original = dir.0.join("测试.tar.gz");
    std::fs::write(&original, b"archive contents").unwrap();
    let paths = vec![original.to_string_lossy().to_string()];
    let saved = capture.capture(CaptureInput::Files(Some(&paths))).await.unwrap().unwrap();
    let cache = PathBuf::from(saved.local_file_path.as_ref().unwrap());
    assert_ne!(cache, original);
    assert!(cache.to_string_lossy().ends_with(".tar.gz"));
    let view = records.query_clip_records(query(Some("测试"))).await.unwrap();
    assert_eq!(view[0].file_info[0].path, "测试.tar.gz");
    records.delete_record(saved.id, false).await.unwrap();
    assert!(!cache.exists());
    assert_eq!(std::fs::read(&original).unwrap(), b"archive contents");

    let second = dir.0.join("second.txt");
    std::fs::write(&second, "second").unwrap();
    let paths = vec![original.to_string_lossy().to_string(), second.to_string_lossy().to_string()];
    let saved = capture.capture(CaptureInput::Files(Some(&paths))).await.unwrap().unwrap();
    records.delete_record(saved.id, false).await.unwrap();
    assert!(original.exists() && second.exists());
}

#[tokio::test]
async fn retention_keeps_pin_and_local_cleanup_removes_unsynced_legacy_tombstones() {
    let dir = TempDir::new();
    let local = open(&dir.0).await;
    local.update_settings(|settings| settings.max_records = 2).unwrap();
    let id = identity();
    let capture = CaptureService::new(&local, &encode, &id, &LocalPolicy);
    let first = capture.capture(CaptureInput::Text("pinned")).await.unwrap().unwrap();
    let old = capture.capture(CaptureInput::Text("oldest unpinned")).await.unwrap().unwrap();
    let latest = capture.capture(CaptureInput::Text("latest")).await.unwrap().unwrap();
    ClipRecordService::new(&local).set_pinned(&first.id, 1).await.unwrap();
    CleanupService::new(&local, false).try_clean().await.unwrap();
    let repo = local.repositories().clip_records();
    assert_eq!(repo.count_active().await.unwrap(), 2);
    assert!(repo.find_by_id(&old.id).await.unwrap().is_none());
    assert!(repo.find_by_id(&first.id).await.unwrap().unwrap().is_pinned());
    repo.mark_deleted(&[latest.id.clone()]).await.unwrap();
    CleanupService::new(&local, true).try_clean().await.unwrap();
    assert!(repo.find_by_id(&latest.id).await.unwrap().is_some());
    CleanupService::new(&local, false).try_clean().await.unwrap();
    assert!(repo.find_by_id(&latest.id).await.unwrap().is_none());
}

#[tokio::test]
async fn resource_cleanup_rejects_external_and_parent_paths() {
    let dir = TempDir::new();
    let local = open(&dir.0).await;
    let outside = dir.0.join("original.png");
    std::fs::write(&outside, "original").unwrap();
    let record = ClipRecord { r#type: "Image".into(), content: serde_json::json!("../original.png"), local_file_path: Some(outside.to_string_lossy().to_string()), ..Default::default() };
    delete_managed_resources(&local, &record).await;
    assert!(outside.exists());
    #[cfg(unix)] {
        let link = dir.0.join("resources/link.png");
        std::os::unix::fs::symlink(&outside, &link).unwrap();
        let record = ClipRecord { content: serde_json::json!("link.png"), local_file_path: None, ..record };
        delete_managed_resources(&local, &record).await;
        assert!(outside.exists() && link.exists());
    }
}

#[tokio::test]
async fn concurrent_capture_has_unique_sort_and_one_record_for_duplicate_content() {
    let dir = TempDir::new();
    let local = open(&dir.0).await;
    let id = identity();
    let capture = CaptureService::new(&local, &encode, &id, &LocalPolicy);
    let (a, b, c) = tokio::join!(capture.capture(CaptureInput::Text("same")), capture.capture(CaptureInput::Text("same")), capture.capture(CaptureInput::Text("other")));
    assert_eq!([a.unwrap(), b.unwrap(), c.unwrap()].iter().filter(|value| value.is_some()).count(), 2);
    let all = local.repositories().clip_records().list(10, 0).await.unwrap();
    assert_eq!(all.len(), 2);
    assert_ne!(all[0].sort, all[1].sort);
}

#[tokio::test]
async fn invalid_parameters_and_failed_unicode_encryption_return_errors() {
    let dir = TempDir::new();
    let local = open(&dir.0).await;
    let records = ClipRecordService::new(&local);
    for (page, size) in [(0, 10), (1, -1), (i32::MAX, i32::MAX)] {
        assert!(records.query_clip_records(QueryParam { page, size, search: None }).await.is_err());
    }
    assert!(records.set_pinned("missing", 2).await.is_err());
    assert!(records.set_pinned("missing", 1).await.is_err());
    let id = identity();
    let fail = |_: &str| -> AppResult<String> { Err(AppError::Crypto("test failure".into())) };
    let content = "中文".repeat(100);
    assert!(CaptureService::new(&local, &fail, &id, &LocalPolicy).capture(CaptureInput::Text(&content)).await.is_err());
    assert_eq!(local.repositories().clip_records().count_active().await.unwrap(), 0);
}

#[tokio::test]
async fn recapture_deleted_file_returns_persisted_cache_snapshot() {
    let dir = TempDir::new();
    let local = open(&dir.0).await;
    let id = identity();
    let capture = CaptureService::new(&local, &encode, &id, &LocalPolicy);
    let source = dir.0.join("source.txt");
    std::fs::write(&source, "test file").unwrap();
    let paths = vec![source.to_string_lossy().to_string()];
    let original = capture.capture(CaptureInput::Files(Some(&paths))).await.unwrap().unwrap();
    ClipRecordService::new(&local).delete_record(original.id.clone(), true).await.unwrap();
    let restored = capture.capture(CaptureInput::Files(Some(&paths))).await.unwrap().unwrap();
    assert_eq!(restored.id, original.id);
    let stored = local.repositories().clip_records().find_by_id(&restored.id).await.unwrap().unwrap();
    assert_eq!(serde_json::to_value(&restored).unwrap(), serde_json::to_value(&stored).unwrap());
    assert!(Path::new(restored.local_file_path.as_ref().unwrap()).exists());
    assert!(source.exists());
}

#[tokio::test]
async fn directory_capture_keeps_a_local_reference_and_delete_preserves_contents() {
    let dir = TempDir::new();
    let local = open(&dir.0).await;
    let source = dir.0.join("资料文件夹");
    std::fs::create_dir(&source).unwrap();
    std::fs::write(source.join("child.txt"), "original").unwrap();
    let paths = vec![source.to_string_lossy().into_owned()];
    let id = identity();
    let capture = CaptureService::new(&local, &encode, &id, &LocalPolicy);
    let record = capture.capture(CaptureInput::Files(Some(&paths))).await.unwrap().unwrap();
    assert_eq!(record.local_file_path.as_deref(), Some(paths[0].as_str()));
    assert_eq!(ClipRecordService::new(&local).query_clip_records(query(Some("资料"))).await.unwrap().len(), 1);
    ClipRecordService::new(&local).delete_record(record.id, false).await.unwrap();
    assert_eq!(std::fs::read_to_string(source.join("child.txt")).unwrap(), "original");
}

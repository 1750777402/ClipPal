use std::sync::Arc;
use selfhost_app_core::{SelfhostContext, desktop_app_core::{LocalContext, clip_core::{domain::clip::ClipRecord, errors::AppResult}}, clip_sync_client::queue::QueueEvent};

#[tokio::test]
async fn selfhost_extends_the_same_local_resources() {
    let dir = std::env::temp_dir().join(format!("clip-pal-composition-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&dir).unwrap();
    let decode = |value: &str| -> AppResult<String> { Ok(value.to_owned()) };
    let local = LocalContext::open(&dir.join("clip.db"), Some(dir.join("settings.json")), Arc::new(decode)).await.unwrap();
    let shared_settings = local.settings();
    let app = SelfhostContext::new(local);
    app.local().update_settings(|settings| settings.max_records = 250).unwrap();
    assert_eq!(shared_settings.read().unwrap().max_records, 250);
    app.local().initialize_search_index().await;
    app.local().search_engine().add("record", "clipboard context").await.unwrap();
    assert_eq!(app.local().search_engine().search("context").await, vec!["record"]);

    let producer = app.sync().record_queue();
    producer.send_add(ClipRecord { id: "queued".into(), ..Default::default() }).await.unwrap();
    assert!(matches!(app.sync().record_queue().try_recv().unwrap(), QueueEvent::Add(record) if record.id == "queued"));
    let guard = app.sync().lock().try_lock().unwrap();
    assert!(app.sync().lock().try_lock().is_none());
    drop(guard);
    assert!(app.sync().lock().try_lock().is_some());
    drop(app);
    let _ = std::fs::remove_dir_all(dir);
}

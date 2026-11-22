use std::sync::Arc;
use selfhost_app_core::{SelfhostContext, desktop_app_core::{LocalContext, clip_core::{storage::has_table, errors::AppResult}}};

/// 验证自建版复用单机上下文及仓储，不另建第二套本地数据。
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

    assert!(!app.sync().status().await.enabled);
    assert!(!has_table(app.local().db(), "clip_sync_record").await.unwrap());
    drop(app);
    let _ = std::fs::remove_dir_all(dir);
}

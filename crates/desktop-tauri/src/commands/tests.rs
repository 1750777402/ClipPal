//! 在 Tauri 官方 mock runtime 中验证迁移后的真实 IPC 注册与响应，不操作系统剪贴板。
use std::sync::Arc;
use serde_json::{json, Value};
use tauri::{test::{mock_builder, mock_context, noop_assets, get_ipc_response, INVOKE_KEY},
    webview::InvokeRequest, ipc::{CallbackFn, InvokeBody}};
use crate::{app_context::AppContext, core::policy::LocalPolicy};

#[test]
fn local_ipc_keeps_record_queries_errors_and_edition_registration() {
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let dir = std::env::temp_dir().join(format!("clippal-ipc-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&dir).unwrap();
    let local = runtime.block_on(async {
        let decode = |value: &str| -> clip_core::errors::AppResult<String> { Ok(value.into()) };
        let local = desktop_app_core::LocalContext::open(&dir.join("test.db"), Some(dir.join("settings.json")), Arc::new(decode)).await.unwrap();
        let record = clip_core::domain::clip::ClipRecord {
            id: "ipc-record".into(), r#type: "Text".into(), content: json!("迁移后的本地 IPC"),
            md5_str: "ipc-test-md5".into(), del_flag: Some(0), cloud_source: Some(0), ..Default::default()
        };
        local.repositories().clip_records().insert(&record).await.unwrap();
        local
    });
    let context = Arc::new(AppContext::new(local, Arc::new(LocalPolicy)));
    let app = super::register_commands(mock_builder()).manage(context)
        .build(mock_context(noop_assets())).unwrap();
    let webview = tauri::WebviewWindowBuilder::new(&app, "main", Default::default()).build().unwrap();
    let invoke = |command: &str, body: Value| {
        get_ipc_response(&webview, InvokeRequest {
            cmd: command.into(), callback: CallbackFn(0), error: CallbackFn(1),
            url: "http://tauri.localhost".parse().unwrap(), body: InvokeBody::Json(body),
            headers: Default::default(), invoke_key: INVOKE_KEY.into(),
        })
    };
    let response: Value = invoke("get_clip_records", json!({"param":{"page":1,"size":10}})).unwrap().deserialize().unwrap();
    assert_eq!(response["success"], true);
    assert_eq!(response["code"], "ok");
    assert_eq!(response["data"][0]["id"], "ipc-record");
    assert_eq!(response["data"][0]["content"], "迁移后的本地 IPC");
    assert_eq!(response["error"], Value::Null);
    let invalid: Value = invoke("get_clip_records", json!({"param":{"page":0,"size":10}})).unwrap().deserialize().unwrap();
    assert_eq!(invalid["success"], false);
    assert_eq!(invalid["data"], Value::Null);
    assert!(!invalid["error"]["code"].as_str().unwrap().is_empty());
    let capabilities: Value = invoke("get_app_capabilities", json!({})).unwrap().deserialize().unwrap();
    assert_eq!(capabilities["data"]["cloud_sync"], false);
    assert_eq!(capabilities["data"]["account"], false);
    assert!(invoke("login", json!({})).is_err());
    drop(webview);
    drop(app);
    drop(runtime);
    let _ = std::fs::remove_dir_all(dir);
}

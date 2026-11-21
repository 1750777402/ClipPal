//! 在 Tauri 官方 mock runtime 中验证迁移后的真实 IPC 注册与响应，不操作系统剪贴板。
use std::sync::Arc;
use serde_json::{json, Value};
use tauri::{Manager, test::{mock_builder, mock_context, noop_assets, get_ipc_response, INVOKE_KEY},
    webview::InvokeRequest, ipc::{CallbackFn, InvokeBody}};
use crate::{app_context::AppContext, core::policy::LocalPolicy};

/// 在 Tauri mock 中验证采集、查询、主键、删除、错误信封和版本命令注册。
#[test]
fn local_ipc_keeps_record_queries_errors_and_edition_registration() {
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let dir = std::env::temp_dir().join(format!("clippal-ipc-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&dir).unwrap();
    let local = runtime.block_on(async {
        let decode = |value: &str| -> clip_core::errors::AppResult<String> { Ok(value.into()) };
        let local = desktop_app_core::LocalContext::open(&dir.join("test.db"), Some(dir.join("settings.json")), Arc::new(decode)).await.unwrap();
        let record = clip_core::domain::clip::ClipRecord {
            r#type: "text".into(), content: Some("迁移后的本地 IPC".into()),
            content_key: "v1:text:ipc-test-hash".into(), ..Default::default()
        };
        local.repositories().clip_records().save_capture(&record).await.unwrap();
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
    assert_eq!(response["data"][0]["id"], "1");
    assert_eq!(response["data"][0]["content"], "迁移后的本地 IPC");
    assert_eq!(response["error"], Value::Null);
    for forbidden in ["sync_flag","cloud_source","os_type"]{assert!(response["data"][0].get(forbidden).is_none());}
    let pin:Value=invoke("set_pinned",json!({"param":{"record_id":"1","pinned_flag":1}})).unwrap().deserialize().unwrap();
    assert_eq!(pin["success"],true);
    let full:Value=invoke("get_full_text_content",json!({"param":{"record_id":"1"}})).unwrap().deserialize().unwrap();
    assert_eq!(full["data"]["content"],"迁移后的本地 IPC");
    // 真实文件采集 -> 两表持久化 -> IPC 列表/删除，资源 ID 必须属于返回的记录。
    let context = app.state::<Arc<AppContext>>();
    let source = dir.join("original.txt");
    std::fs::write(&source, "local file").unwrap();
    std::fs::create_dir_all(dir.join("resources")).unwrap();
    // 测试上下文未配置资源目录，文件保留外部引用；删除不能移除原件。
    let file = runtime.block_on(async {
        desktop_app_core::capture::CaptureService::new(context.core(), &|value: &str| Ok(value.to_owned()))
            .capture(desktop_app_core::capture::CaptureInput::Files(Some(&vec![source.to_string_lossy().into_owned()])))
            .await.unwrap().unwrap()
    });
    let files:Value=invoke("get_clip_records",json!({"param":{"page":1,"size":10}})).unwrap().deserialize().unwrap();
    let dto=files["data"].as_array().unwrap().iter().find(|row|row["id"]==file.id.to_string()).unwrap();
    assert_eq!(dto["type"],"File");
    assert_eq!(dto["file_info"][0]["resource_id"],file.resources[0].id.to_string());
    assert_eq!(dto["file_info"][0]["path"],"original.txt");
    for id in ["1".to_owned(),file.id.to_string()] {
        let deleted:Value=invoke("del_record",json!({"param":{"record_id":id}})).unwrap().deserialize().unwrap();
        assert_eq!(deleted["success"],true);
    }
    let empty:Value=invoke("get_clip_records",json!({"param":{"page":1,"size":10}})).unwrap().deserialize().unwrap();
    assert_eq!(empty["data"],json!([]));
    assert!(source.exists());
    let missing:Value=invoke("get_full_text_content",json!({"param":{"record_id":"1"}})).unwrap().deserialize().unwrap();
    assert_eq!(missing["success"],false);
    let invalid: Value = invoke("get_clip_records", json!({"param":{"page":0,"size":10}})).unwrap().deserialize().unwrap();
    assert_eq!(invalid["success"], false);
    assert_eq!(invalid["data"], Value::Null);
    assert!(!invalid["error"]["code"].as_str().unwrap().is_empty());
    let capabilities: Value = invoke("get_app_capabilities", json!({})).unwrap().deserialize().unwrap();
    assert_eq!(capabilities["data"]["cloud_sync"], false);
    assert_eq!(capabilities["data"]["account"], false);
    let permission: Value = invoke("get_auto_paste_permission", json!({})).unwrap().deserialize().unwrap();
    assert_eq!(permission["success"], true);
    assert_eq!(permission["code"], "ok");
    assert!(matches!(permission["data"].as_str(), Some("required" | "granted" | "not_required")));
    assert!(invoke("login", json!({})).is_err());
    drop(webview);
    drop(context);
    drop(app);
    drop(runtime);
    let _ = std::fs::remove_dir_all(dir);
}

use clip_sync_protocol::{ClipRecordParam, CloudSyncRequest, DownloadCloudFileResponse};
use serde_json::json;

/// 验证同步线协议保持既有字段名，不混入本地数据库专有字段。
#[test]
fn keeps_legacy_wire_names_and_excludes_local_fields() {
    let param = ClipRecordParam {
        id: Some("local-id".into()), r#type: Some("Text".into()), content: json!("ciphertext=="),
        md5_str: Some("digest".into()), created: Some(12), os_type: Some("macOS".into()), sort:Some(0),pinned_flag:Some(0),
        local_file_path: Some("/private/local-file".into()), sync_flag: Some(0),
        sync_time: Some(10), device_id: Some("device".into()), version: Some(1),del_flag:Some(0),
    };
    let value = serde_json::to_value(CloudSyncRequest { clips: vec![param], timestamp: 20, last_sync_time: 10, device_id: "device".into() }).unwrap();
    assert_eq!(value, json!({
        "clips": [{ "type": "Text", "content": "ciphertext==", "md5Str": "digest", "created": 12,
            "osType": "macOS", "sort": 0, "pinnedFlag": 0, "syncFlag": 0, "syncTime": 10,
            "deviceId": "device", "version": 1, "delFlag": 0 }],
        "timestamp": 20, "last_sync_time": 10, "device_id": "device"
    }));
}

/// 验证当前下载响应能按协议结构正确反序列化。
#[test]
fn reads_current_file_download_response() {
    let response: DownloadCloudFileResponse = serde_json::from_value(json!({
        "url": "https://example.test/file", "md5Str": "digest", "type": "File", "fileName": "记录.txt"
    })).unwrap();
    assert_eq!(response.file_name, "记录.txt");
    assert_eq!(response.md5_str, "digest");
}

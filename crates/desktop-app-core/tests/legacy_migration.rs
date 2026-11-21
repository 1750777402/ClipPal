use std::{path::Path,sync::Arc};
use clip_core::{crypto::ContentCipher,domain::clip::ClipRecord,errors::{AppError,AppResult},content_key};
use desktop_app_core::LocalContext;
use rbatis::RBatis;
use rbs::to_value;
const KEY:&str="AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=";

/// 在临时路径创建旧版表结构，作为兼容迁移测试输入。
async fn old_db(path:&Path)->RBatis{
    let db=RBatis::new();db.init(rbdc_sqlite::Driver{},&format!("sqlite://{}",path.to_string_lossy())).unwrap();
    db.exec("CREATE TABLE clip_record(id TEXT PRIMARY KEY,type TEXT,content TEXT,md5_str TEXT,local_file_path TEXT,created INTEGER,sort INTEGER,pinned_flag INTEGER,del_flag INTEGER,sync_flag INTEGER,sync_time INTEGER,cloud_source INTEGER,skip_type INTEGER,os_type TEXT,device_id TEXT,version INTEGER)",vec![]).await.unwrap();
    db.exec("CREATE TABLE sync_time(id TEXT PRIMARY KEY,last_time INTEGER)",vec![]).await.unwrap();
    db.exec("INSERT INTO sync_time VALUES('last_sync_ts',123)",vec![]).await.unwrap();db
}
/// 插入一条可控制置顶、删除和同步旧字段的迁移样本。
async fn insert(db:&RBatis,id:&str,kind:&str,content:&str,path:Option<&str>,sort:i32,pin:i32,deleted:i32){
    db.exec("INSERT INTO clip_record VALUES(?,?,?,?,?,100,?,?,?,0,123,1,2,'macos','legacy-device',4)",vec![to_value!(id),to_value!(kind),to_value!(content),to_value!(format!("md5-{id}")),to_value!(path),to_value!(sort),to_value!(pin),to_value!(deleted)]).await.unwrap();
}
/// 用测试密钥解密，不访问桌面打包密钥。
fn decode(text:&str)->AppResult<String>{ContentCipher::from_base64_key(KEY)?.decrypt(text)}
/// 用测试解密器打开临时旧库，触发与实际启动一致的迁移流程。
async fn open(dir:&Path)->AppResult<LocalContext>{LocalContext::open_with_resources(&dir.join("test.db"),Some(dir.join("settings.json")),Arc::new(decode),Some(dir.join("resources"))).await}
/// 列出测试目录中的迁移备份，验证备份次数与重复打开行为。
fn backups(dir:&Path)->Vec<std::path::PathBuf>{std::fs::read_dir(dir).unwrap().map(|row|row.unwrap().path()).filter(|path|path.extension().is_some_and(|ext|ext=="bak")).collect()}

/// 验证旧库转换身份、资源和排序，保留密文、设置及完整备份。
#[tokio::test]
async fn migration_recomputes_content_keys_preserves_ciphertext_resources_settings_and_backup(){
    let dir=tempfile::tempdir().unwrap();let root=dir.path();std::fs::create_dir(root.join("resources")).unwrap();
    let db=old_db(&root.join("test.db")).await;
    let cipher=ContentCipher::from_base64_key(KEY).unwrap();let original="  缩进文本\n";let encrypted=cipher.encrypt(original).unwrap();
    insert(&db,"uuid-text","Text",&encrypted,None,1,1,0).await;
    insert(&db,"duplicate-text","Text",&cipher.encrypt(original).unwrap(),None,5,0,0).await;
    db.exec("UPDATE clip_record SET created=200 WHERE id='uuid-text'",vec![]).await.unwrap();
    insert(&db,"deleted","Text",&cipher.encrypt("deleted").unwrap(),None,0,0,1).await;
    std::fs::write(root.join("resources/screenshot.png"),[1,2,3]).unwrap();
    insert(&db,"uuid-image","Image","screenshot.png",None,2,0,0).await;
    std::fs::create_dir(root.join("resources/files")).unwrap();std::fs::write(root.join("resources/files/cache.bin"),b"cached").unwrap();
    let cached=root.join("resources/files/cache.bin");
    insert(&db,"uuid-file","File","名称.bin",Some(cached.to_str().unwrap()),3,0,0).await;
    let external=root.join("external.txt");std::fs::write(&external,"external").unwrap();
    insert(&db,"uuid-reference","File","external.txt",Some(external.to_str().unwrap()),4,0,0).await;
    // 已缺失附件仍保留可见记录，不用旧 MD5 冒充新内容摘要。
    insert(&db,"uuid-missing","Image","missing.png",None,6,0,0).await;
    std::fs::write(root.join("settings.json"),r#"{"max_records":500,"auto_paste":0,"shortcut_key":"Ctrl+Shift+V","cloud_sync":1}"#).unwrap();
    drop(db);
    let local=open(root).await.unwrap();let repo=local.repositories().clip_records();let all=repo.list_all().await.unwrap();assert_eq!(all.len(),5);
    let text=repo.find_by_content_key(&content_key::text_key(original)).await.unwrap().unwrap();assert!(text.id>0);assert!(text.is_pinned());assert_eq!(text.sort_order,5);assert_eq!(text.created_at,100);
    assert_eq!(decode(text.content.as_deref().unwrap()).unwrap(),original);
    assert_eq!(local.with_settings(|settings|settings.max_records).unwrap(),500);assert_eq!(local.with_settings(|settings|settings.auto_paste).unwrap(),0);
    let file=all.iter().find(|row|row.resources.first().is_some_and(|resource|resource.display_name=="名称.bin")).unwrap();
    assert_eq!(file.resources[0].storage_kind,"managed");assert_eq!(file.resources[0].path,"files/cache.bin");
    let reference=all.iter().find(|row|row.resources.first().is_some_and(|resource|resource.display_name=="external.txt")).unwrap();assert_eq!(reference.resources[0].storage_kind,"reference");
    assert!(all.iter().any(|row|row.content_key.starts_with("v1:unavailable:image:")));
    local.initialize_search_index().await;assert_eq!(local.search_engine().search("缩进").await,vec![text.id.to_string()]);
    assert!(!clip_core::storage::has_table(local.db(),"sync_time").await.unwrap());
    let backup=backups(root);assert_eq!(backup.len(),1);
    let old=RBatis::new();old.init(rbdc_sqlite::Driver{},&format!("sqlite://{}",backup[0].to_string_lossy())).unwrap();
    #[derive(serde::Deserialize)]struct Count{count:i64}
    let count:Vec<Count>=old.query_decode("SELECT COUNT(*) AS count FROM clip_record",vec![]).await.unwrap();assert_eq!(count[0].count,7);
    let preserved:Vec<serde_json::Value>=old.query_decode("SELECT sync_flag,skip_type,device_id FROM clip_record WHERE id='uuid-text'",vec![]).await.unwrap();assert_eq!(preserved[0]["device_id"],"legacy-device");
    drop(local);let reopened=open(root).await.unwrap();assert_eq!(reopened.repositories().clip_records().find_by_content_key(&text.content_key).await.unwrap().unwrap().id,text.id);assert_eq!(backups(root).len(),1);
    assert!(external.exists() && cached.exists());
}

/// 验证解密或换表失败时回滚结构和数据，不静默跳过记录。
#[tokio::test]
async fn decryption_or_migration_write_failure_leaves_old_schema_and_rows_intact(){
    for bad_cipher in [true,false]{
        let dir=tempfile::tempdir().unwrap();let root=dir.path();std::fs::create_dir(root.join("resources")).unwrap();
        let db=old_db(&root.join("test.db")).await;
        let data=if bad_cipher{"broken".into()}else{ContentCipher::from_base64_key(KEY).unwrap().encrypt("safe").unwrap()};
        insert(&db,"old","Text",&data,None,1,0,0).await;
        if !bad_cipher{
            // 在重命名后创建新表阶段故意失败，验证 DDL 和记录复制均回滚。
            db.exec("CREATE TABLE clip_record_resource(dummy TEXT)",vec![]).await.unwrap();
        }
        let result=open(root).await;
        // 空资源旧库通过表结构冲突保护，不能静默接受未知资源表。
        if !bad_cipher && result.is_ok(){panic!("incompatible preexisting resource table must be rejected");}
        assert!(result.is_err());assert_eq!(backups(root).len(),1);
        #[derive(serde::Deserialize)]struct Row{id:String,content:String}
        let rows:Vec<Row>=db.query_decode("SELECT id,content FROM clip_record",vec![]).await.unwrap();assert_eq!(rows[0].id,"old");assert_eq!(rows[0].content,data);
        assert!(clip_core::storage::has_table(&db,"sync_time").await.unwrap());
    }
}

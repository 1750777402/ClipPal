use std::{path::{Path,PathBuf},sync::Arc};
use clip_core::{crypto::ContentCipher,domain::clip::{ClipRecord,ClipRecordResource},errors::{AppError,AppResult}};
use desktop_app_core::{LocalContext,capture::{CaptureInput,CaptureService},cleanup::{CleanupService,delete_managed_resources},dto::QueryParam,records::{ClipRecordService,resolve_resource}};
const KEY:&str="AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=";
/// 每个用例独占的临时目录，结束后回收，不使用本机开发数据库。
struct TempDir(PathBuf);
impl TempDir{
    /// 创建唯一目录和资源根，避免多个测试相互覆盖。
    fn new()->Self{let path=std::env::temp_dir().join(format!("clippal-local-{}",uuid::Uuid::new_v4()));std::fs::create_dir_all(path.join("resources")).unwrap();Self(path)}
}
impl Drop for TempDir{
    /// 测试退出时只删除本用例创建的目录，不触碰真实数据。
    fn drop(&mut self){let _=std::fs::remove_dir_all(&self.0);}
}
/// 用固定测试密钥加密原文，保持生产加密格式但不使用真实密钥。
fn encode(text:&str)->AppResult<String>{ContentCipher::from_base64_key(KEY)?.encrypt(text)}
/// 用临时路径、测试解密器和资源目录组装单机核心。
async fn open(dir:&Path)->LocalContext{LocalContext::open_with_resources(&dir.join("test.db"),Some(dir.join("settings.json")),Arc::new(|text:&str|ContentCipher::from_base64_key(KEY)?.decrypt(text)),Some(dir.join("resources"))).await.unwrap()}
/// 构造统一分页参数，按需附带搜索词。
fn query(search:Option<&str>)->QueryParam{QueryParam{page:1,size:100,search:search.map(str::to_owned)}}

/// 验证原文加密、预览全文、去重排序、搜索、删除和重新采集。
#[tokio::test]
async fn text_roundtrip_preserves_whitespace_deduplicates_reorders_searches_and_deletes(){
    let dir=TempDir::new();let local=open(&dir.0).await;let capture=CaptureService::new(&local,&encode);
    assert!(capture.capture(CaptureInput::Text("")).await.unwrap().is_none());
    let content=format!("  {} \n","中文🙂总结".repeat(1000));
    let first=capture.capture(CaptureInput::Text(&content)).await.unwrap().unwrap();
    assert_ne!(first.content.as_deref().unwrap(),content);
    assert_eq!(ContentCipher::from_base64_key(KEY).unwrap().decrypt(first.content.as_deref().unwrap()).unwrap(),content);
    capture.capture(CaptureInput::Text("second")).await.unwrap();
    let repeat=capture.capture(CaptureInput::Text(&content)).await.unwrap().unwrap();assert_eq!(first.id,repeat.id);
    let records=ClipRecordService::new(&local);let found=records.query_clip_records(query(Some("总结"))).await.unwrap();
    assert_eq!(found.len(),1);assert!(found[0].content_truncated);assert_eq!(found[0].id,first.id.to_string());
    assert!(content.starts_with(&found[0].content));assert_eq!(records.get_full_text_content(first.id.to_string()).await.unwrap().content,content);
    local.initialize_search_index().await;assert_eq!(records.query_clip_records(query(Some("总结"))).await.unwrap().len(),1);
    assert_eq!(records.query_clip_records(query(None)).await.unwrap()[0].id,first.id.to_string());
    let whitespace=capture.capture(CaptureInput::Text("  \n")).await.unwrap().unwrap();assert_ne!(whitespace.content_key,first.content_key);
    records.delete_record(first.id.to_string()).await.unwrap();assert!(local.repositories().clip_records().find_by_id(first.id).await.unwrap().is_none());assert!(local.search_engine().search("总结").await.is_empty());
    let recreated=capture.capture(CaptureInput::Text(&content)).await.unwrap().unwrap();assert!(recreated.id>whitespace.id);
}

/// 验证图片复用、文件改名、同名附件和目录的采集删除闭环。
#[tokio::test]
async fn image_file_directory_and_duplicate_name_resources_close_the_local_lifecycle(){
    let dir=TempDir::new();let local=open(&dir.0).await;let capture=CaptureService::new(&local,&encode);let records=ClipRecordService::new(&local);
    let image=vec![1,2,3,4];let saved=capture.capture(CaptureInput::Image(Some(&image))).await.unwrap().unwrap();
    let path=PathBuf::from(records.get_image_path(&saved.id.to_string()).await.unwrap().file_path);assert_eq!(std::fs::read(&path).unwrap(),image);
    let again=capture.capture(CaptureInput::Image(Some(&image))).await.unwrap().unwrap();assert_eq!(again.id,saved.id);assert_eq!(again.resources[0].path,saved.resources[0].path);
    records.delete_record(saved.id.to_string()).await.unwrap();assert!(!path.exists());
    let original=dir.0.join("测试.tar.gz");std::fs::write(&original,b"archive").unwrap();
    let paths=vec![original.to_string_lossy().into_owned()];let saved=capture.capture(CaptureInput::Files(Some(&paths))).await.unwrap().unwrap();
    let cache=resolve_resource(&local,&saved.resources[0]).unwrap();assert_ne!(cache,original);assert!(cache.to_string_lossy().ends_with(".tar.gz"));
    let renamed=dir.0.join("renamed.txt");std::fs::rename(&original,&renamed).unwrap();
    let paths=vec![renamed.to_string_lossy().into_owned()];let again=capture.capture(CaptureInput::Files(Some(&paths))).await.unwrap().unwrap();
    assert_eq!(again.id,saved.id);assert_eq!(again.resources[0].display_name,"renamed.txt");assert!(!cache.exists());
    assert_eq!(records.query_clip_records(query(Some("renamed"))).await.unwrap()[0].file_info[0].resource_id,again.resources[0].id.to_string());
    records.delete_record(again.id.to_string()).await.unwrap();assert!(renamed.exists());
    for name in ["a","b"]{std::fs::create_dir(dir.0.join(name)).unwrap();std::fs::write(dir.0.join(name).join("same.txt"),name).unwrap();}
    let paths=vec![dir.0.join("a/same.txt").to_string_lossy().into_owned(),dir.0.join("b/same.txt").to_string_lossy().into_owned()];
    let multiple=capture.capture(CaptureInput::Files(Some(&paths))).await.unwrap().unwrap();assert_eq!(multiple.resources.len(),2);assert_ne!(multiple.resources[0].id,multiple.resources[1].id);
    let hashes:Vec<_>=multiple.resources.iter().map(|r|r.content_hash.clone()).collect();assert_ne!(hashes[0],hashes[1]);
    let mut reversed=paths.clone();reversed.reverse();assert_eq!(capture.capture(CaptureInput::Files(Some(&reversed))).await.unwrap().unwrap().id,multiple.id);
    records.delete_record(multiple.id.to_string()).await.unwrap();for path in paths{assert!(Path::new(&path).exists());}
    let folder=vec![dir.0.join("a").to_string_lossy().into_owned()];let directory=capture.capture(CaptureInput::Files(Some(&folder))).await.unwrap().unwrap();
    assert_eq!(directory.r#type,"directory");assert_eq!(directory.resources[0].storage_kind,"reference");records.delete_record(directory.id.to_string()).await.unwrap();assert!(Path::new(&folder[0]).is_dir());
}

/// 验证并发采集去重、置顶保护、容量清理和重启索引恢复。
#[tokio::test]
async fn retention_pin_concurrent_capture_and_reopen_remain_consistent(){
    let dir=TempDir::new();let local=open(&dir.0).await;let capture=CaptureService::new(&local,&encode);
    let (a,b,c)=tokio::join!(capture.capture(CaptureInput::Text("same")),capture.capture(CaptureInput::Text("same")),capture.capture(CaptureInput::Text("other")));
    let a=a.unwrap().unwrap();assert_eq!(a.id,b.unwrap().unwrap().id);let c=c.unwrap().unwrap();assert_ne!(a.sort_order,c.sort_order);
    ClipRecordService::new(&local).set_pinned(&a.id.to_string(),1).await.unwrap();
    local.update_settings(|settings|settings.max_records=1).unwrap();CleanupService::new(&local).try_clean().await.unwrap();
    assert_eq!(local.repositories().clip_records().count().await.unwrap(),1);assert!(local.repositories().clip_records().find_by_id(a.id).await.unwrap().unwrap().is_pinned());
    drop(local);let reopened=open(&dir.0).await;reopened.initialize_search_index().await;assert_eq!(reopened.search_engine().search("same").await,vec![a.id.to_string()]);
}

/// 验证非法参数、加密失败和越界路径不会改坏记录或用户原件。
#[tokio::test]
async fn invalid_commands_failed_encryption_and_resource_path_escape_do_not_change_data(){
    let dir=TempDir::new();let local=open(&dir.0).await;let records=ClipRecordService::new(&local);
    assert!(records.query_clip_records(QueryParam{page:0,size:10,search:None}).await.is_err());
    assert!(records.set_pinned("invalid",1).await.is_err());assert!(records.set_pinned("1",2).await.is_err());
    let fail=|_:&str|->AppResult<String>{Err(AppError::Crypto("test failure".into()))};
    assert!(CaptureService::new(&local,&fail).capture(CaptureInput::Text("中文内容")).await.is_err());assert_eq!(local.repositories().clip_records().count().await.unwrap(),0);
    let outside=dir.0.join("original.png");std::fs::write(&outside,"original").unwrap();
    let record=ClipRecord{resources:vec![ClipRecordResource{storage_kind:"managed".into(),path:"../original.png".into(),..Default::default()}],..Default::default()};
    assert!(resolve_resource(&local,&record.resources[0]).is_err());delete_managed_resources(&local,&record).await;assert!(outside.exists());
    #[cfg(unix)]{
        std::os::unix::fs::symlink(&outside,dir.0.join("resources/link.png")).unwrap();
        let mut record=record;record.resources[0].path="link.png".into();assert!(resolve_resource(&local,&record.resources[0]).is_err());delete_managed_resources(&local,&record).await;assert!(outside.exists());
    }
}

/// 验证复制保留完整文本，并按资源主键选择同名文件。
#[tokio::test]
async fn copy_payload_restores_exact_text_and_selects_duplicate_filenames_by_resource_id(){
    use desktop_app_core::clipboard::{prepare_copy,ClipboardPayload};
    let dir=TempDir::new();let local=open(&dir.0).await;let capture=CaptureService::new(&local,&encode);
    let text="  code\n";let saved=capture.capture(CaptureInput::Text(text)).await.unwrap().unwrap();
    assert_eq!(prepare_copy(&local,&saved,None).unwrap(),ClipboardPayload::Text(text.into()));
    assert!(prepare_copy(&local,&saved,Some(1)).is_err());
    for name in ["first","second"]{std::fs::create_dir(dir.0.join(name)).unwrap();std::fs::write(dir.0.join(name).join("same.txt"),name).unwrap();}
    let paths=vec![dir.0.join("first/same.txt").to_string_lossy().into_owned(),dir.0.join("second/same.txt").to_string_lossy().into_owned()];
    let saved=capture.capture(CaptureInput::Files(Some(&paths))).await.unwrap().unwrap();
    for (index,expected) in ["first","second"].iter().enumerate(){
        let ClipboardPayload::Files{names,paths,..}=prepare_copy(&local,&saved,Some(saved.resources[index].id)).unwrap()else{panic!("file payload expected")};
        assert_eq!(names,vec!["same.txt"]);assert_eq!(std::fs::read_to_string(&paths[0]).unwrap(),*expected);
    }
    assert!(prepare_copy(&local,&saved,Some(i64::MAX)).is_err());
    let ClipboardPayload::Files{paths,..}=prepare_copy(&local,&saved,None).unwrap()else{panic!("file payload expected")};
    std::fs::remove_file(&paths[0]).unwrap();assert!(prepare_copy(&local,&saved,None).is_err());
}

/// 验证旧采样区域外的内容差异也会产生不同完整文件摘要。
#[tokio::test]
async fn full_hash_distinguishes_large_files_that_only_differ_outside_legacy_sample_regions(){
    let dir=TempDir::new();let local=open(&dir.0).await;let capture=CaptureService::new(&local,&encode);
    let a=dir.0.join("a.bin");let b=dir.0.join("b.bin");
    let data=vec![0u8;11*1024*1024];std::fs::write(&a,&data).unwrap();let mut other=data;other[2*1024*1024]=1;std::fs::write(&b,&other).unwrap();
    let first=capture.capture(CaptureInput::Files(Some(&vec![a.to_string_lossy().into_owned()]))).await.unwrap().unwrap();
    let second=capture.capture(CaptureInput::Files(Some(&vec![b.to_string_lossy().into_owned()]))).await.unwrap().unwrap();
    assert_ne!(first.id,second.id);assert_ne!(first.content_key,second.content_key);
    assert_eq!(clip_core::content_key::file_hash(&a).unwrap(),first.resources[0].content_hash.as_ref().unwrap().as_str());
}

/// 验证数据库写入失败时回收新缓存，保留旧记录和旧资源。
#[tokio::test]
async fn capture_database_failure_releases_new_cache_and_keeps_previous_file_record(){
    let dir=TempDir::new();let local=open(&dir.0).await;let capture=CaptureService::new(&local,&encode);
    let source=dir.0.join("source.txt");std::fs::write(&source,"data").unwrap();
    let paths=vec![source.to_string_lossy().into_owned()];let saved=capture.capture(CaptureInput::Files(Some(&paths))).await.unwrap().unwrap();
    let cache=resolve_resource(&local,&saved.resources[0]).unwrap();
    local.db().exec("CREATE TRIGGER reject_resource BEFORE INSERT ON clip_record_resource BEGIN SELECT RAISE(ABORT,'test failure'); END",vec![]).await.unwrap();
    assert!(capture.capture(CaptureInput::Files(Some(&paths))).await.is_err());
    let current=local.repositories().clip_records().find_by_id(saved.id).await.unwrap().unwrap();assert_eq!(current.resources[0].id,saved.resources[0].id);assert!(cache.exists() && source.exists());
    assert_eq!(std::fs::read_dir(dir.0.join("resources/files")).unwrap().count(),1);
}

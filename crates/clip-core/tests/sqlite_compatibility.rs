#![cfg(feature="sqlite")]
use clip_core::{domain::clip::{ClipRecord,ClipRecordResource},repositories::ClipRecordRepository,storage::{connect,SqliteClipRecordRepository}};
use rbs::to_value;

/// 创建隔离的 SQLite 与仓储，测试不会打开用户正式数据库。
async fn repo()->(tempfile::TempDir,rbatis::RBatis,SqliteClipRecordRepository){
    let dir=tempfile::tempdir().unwrap();let db=connect(&dir.path().join("test.db")).await.unwrap();
    let repo=SqliteClipRecordRepository::new(db.clone());(dir,db,repo)
}
/// 构造最小文本实体，便于专注验证数据库事务规则。
fn text(key:&str)->ClipRecord{ClipRecord{content_key:key.into(),r#type:"text".into(),content:Some("cipher".into()),created_at:10,last_copied_at:20,..Default::default()}}

/// 验证两表边界、自增主键、同内容复用、最近排序和唯一置顶。
#[tokio::test]
async fn local_tables_autoincrement_unique_content_order_and_pins(){
    let(_dir,db,repo)=repo().await;
    let first=repo.save_capture(&text("first")).await.unwrap();
    let second=repo.save_capture(&text("second")).await.unwrap();
    assert!(first.id>0 && second.id>first.id);
    repo.set_pinned(first.id,1).await.unwrap();
    let mut repeat=text("first");repeat.created_at=100;repeat.last_copied_at=200;
    let repeat=repo.save_capture(&repeat).await.unwrap();
    assert_eq!(repeat.id,first.id);assert_eq!(repeat.created_at,10);assert_eq!(repeat.last_copied_at,200);assert_eq!(repeat.pinned,1);
    assert!(repeat.sort_order>second.sort_order);assert_eq!(repo.count().await.unwrap(),2);
    repo.set_pinned(second.id,1).await.unwrap();assert!(!repo.find_by_id(first.id).await.unwrap().unwrap().is_pinned());
    // 部分唯一索引也保护绕过仓储的直接 SQL。
    assert!(db.exec("UPDATE clip_record SET pinned=1 WHERE id=?",vec![to_value!(first.id)]).await.is_err());
    repo.delete(second.id).await.unwrap();
    let third=repo.save_capture(&text("third")).await.unwrap();assert!(third.id>second.id);
    #[derive(serde::Deserialize)]struct Table{name:String}
    let tables:Vec<Table>=db.query_decode("SELECT name FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%' ORDER BY name",vec![]).await.unwrap();
    assert_eq!(tables.into_iter().map(|t|t.name).collect::<Vec<_>>(),vec!["clip_record","clip_record_resource"]);
}

/// 验证不同池连接均启用外键，资源替换失败恢复原清单。
#[tokio::test]
async fn resources_cascade_on_every_connection_and_failed_replacement_rolls_back(){
    let(_dir,db,repo)=repo().await;
    let mut row=ClipRecord{r#type:"files".into(),content_key:"files:one".into(),resources:vec![ClipRecordResource{position:0,display_name:"one.txt".into(),resource_kind:"file".into(),storage_kind:"reference".into(),path:"/original.txt".into(),..Default::default()}],..Default::default()};
    let saved=repo.save_capture(&row).await.unwrap();assert_eq!(saved.resources[0].record_id,saved.id);
    let original_resource_id=saved.resources[0].id;
    row.resources.push(row.resources[0].clone()); // 重复位置违反联合唯一约束。
    assert!(repo.save_capture(&row).await.is_err());
    let current=repo.find_by_id(saved.id).await.unwrap().unwrap();assert_eq!(current.resources.len(),1);assert_eq!(current.resources[0].id,original_resource_id);
    #[derive(serde::Deserialize)]struct Count{count:i64}
    // 占住一个连接，从另一个连接执行删除，验证连接池中每个连接都启用外键。
    let connection=db.acquire().await.unwrap();
    db.exec("DELETE FROM clip_record WHERE id=?",vec![to_value!(saved.id)]).await.unwrap();
    let rows:Vec<Count>=repo_count(&db).await;assert_eq!(rows[0].count,0);
    drop(connection);
    /// 直接统计资源行，确认级联删除实际生效。
    async fn repo_count(db:&rbatis::RBatis)->Vec<Count>{db.query_decode("SELECT COUNT(*) AS count FROM clip_record_resource",vec![]).await.unwrap()}
}

/// 验证置顶失败保留旧置顶，超限删除返回完整回收快照。
#[tokio::test]
async fn pin_failure_restores_previous_pin_and_overflow_returns_resource_snapshots(){
    let(_dir,db,repo)=repo().await;
    let a=repo.save_capture(&text("a")).await.unwrap();let b=repo.save_capture(&text("b")).await.unwrap();
    repo.set_pinned(a.id,1).await.unwrap();
    db.exec(&format!("CREATE TRIGGER reject_pin BEFORE UPDATE OF pinned ON clip_record WHEN NEW.id={} AND NEW.pinned=1 BEGIN SELECT RAISE(ABORT,'test failure'); END",b.id),vec![]).await.unwrap();
    assert!(repo.set_pinned(b.id,1).await.is_err());assert!(repo.find_by_id(a.id).await.unwrap().unwrap().is_pinned());
    let removed=repo.remove_overflow(1).await.unwrap();assert_eq!(removed[0].id,b.id);assert_eq!(repo.count().await.unwrap(),1);
}

/// 验证大量两表实体字段读写、资源归属、顺序及分批分页。
#[tokio::test]
async fn table_entities_roundtrip_and_bulk_loading_keeps_resource_ownership_and_order() {
    let (_dir, db, repo) = repo().await;
    let tx = db.acquire_begin().await.unwrap();
    for index in 0..805 {
        let record = ClipRecord {
            content_key: format!("files:{index}"), r#type: "files".into(), sort_order: index,
            resources: (0..2).map(|position| ClipRecordResource {
                position, display_name: format!("{index}-{position}.txt"),
                resource_kind: "file".into(), storage_kind: "reference".into(),
                path: format!("/original/{index}-{position}.txt"),
                byte_size: Some(123), content_hash: Some(format!("hash-{index}-{position}")),
                ..Default::default()
            }).collect(), ..Default::default()
        };
        clip_core::storage::persist_record(&tx, &record).await.unwrap();
    }
    tx.commit().await.unwrap();
    let all = repo.list_all().await.unwrap();
    assert_eq!(all.len(), 805);
    for record in &all {
        assert_eq!(record.resources.len(), 2);
        for (position, resource) in record.resources.iter().enumerate() {
            assert_eq!(resource.record_id, record.id);
            assert_eq!(resource.position, position as i32);
            let loaded: Vec<ClipRecordResource> = db.query_decode(
                "SELECT * FROM clip_record_resource WHERE id=?", vec![to_value!(resource.id)],
            ).await.unwrap();
            assert_eq!(&loaded[0], resource);
        }
    }
    let ids: Vec<_> = all.iter().map(|record| record.id.to_string()).collect();
    assert_eq!(repo.list_by_ids(&ids, 25, 399).await.unwrap().iter().map(|row| row.id).collect::<Vec<_>>(),
        all[399..424].iter().map(|row| row.id).collect::<Vec<_>>());
    let removed = repo.delete(all[0].id).await.unwrap().unwrap();
    assert_eq!(removed.resources, all[0].resources);
    let remaining: Vec<ClipRecordResource> = db.query_decode(
        "SELECT * FROM clip_record_resource WHERE record_id=?", vec![to_value!(removed.id)],
    ).await.unwrap();
    assert!(remaining.is_empty());
}

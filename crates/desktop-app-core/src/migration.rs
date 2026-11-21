//! 一次性旧库升级。旧表及完整同步字段先备份，迁移事务失败不改变旧表。
use std::path::{Path,PathBuf};
use rbatis::{RBatis,executor::Executor};
use rbs::to_value;
use clip_core::{domain::clip::{ClipRecord,ClipRecordResource,TEXT_TYPE,IMAGE_TYPE,FILE_TYPE,DIRECTORY_TYPE},content_key,errors::{AppError,AppResult},search::TextDecoder,storage};

/// 仅用于读取旧库的临时模型；旧同步字段不进入新的本地业务实体。
#[derive(serde::Deserialize,Default)]
#[serde(default)]
struct LegacyRecord {
    id:String, r#type:String, content:serde_json::Value, md5_str:String,
    local_file_path:Option<String>, created:u64, sort:i64, pinned_flag:i32, del_flag:Option<i32>,
}

/// 把旧路径转换成资源实体；只有确认属于缓存根目录的文件才标记为 managed。
fn legacy_resource(name:&str,path:PathBuf,kind:&str,base:Option<&Path>,position:i32) -> AppResult<ClipRecordResource> {
    let managed=base.and_then(|base| path.strip_prefix(base).ok()).map(Path::to_path_buf);
    let actual=path.canonicalize().unwrap_or(path.clone());
    // 既检查原相对位置，也检查符号链接解析后的实际位置，防止误删外部原件。
    let owned=managed.is_some() && base.and_then(|base| base.canonicalize().ok()).is_some_and(|base| actual.starts_with(base));
    // 缺失资源保留名称和位置，不虚构文件大小和完整内容摘要。
    let exists=actual.is_file();
    Ok(ClipRecordResource { display_name:name.into(),position,resource_kind:kind.into(),
        storage_kind:if owned {"managed"} else {"reference"}.into(),
        path:if owned {managed.unwrap().to_string_lossy().into_owned()} else {path.to_string_lossy().into_owned()},
        byte_size:if exists {Some(i64::try_from(std::fs::metadata(&actual)?.len()).map_err(|_|AppError::Config("资源过大".into()))?)} else {None},
        content_hash:if exists {Some(content_key::file_hash(&actual)?)} else {None},..Default::default() })
}

/// 只升级旧结构：先备份，再在事务中转换并换表；新库重复打开直接返回。
pub async fn migrate(db:&RBatis,db_path:&Path,decoder:&dyn TextDecoder,resources:Option<&Path>) -> AppResult<()> {
    if !storage::is_legacy(db).await? { return Ok(()); }
    // VACUUM INTO 包含 WAL 中已经提交的数据，不能只复制主文件。
    let backup=db_path.with_file_name(format!("{}.before-local-v2-{}.bak",db_path.file_name().unwrap_or_default().to_string_lossy(),uuid::Uuid::new_v4()));
    db.exec("VACUUM INTO ?",vec![to_value!(backup.to_string_lossy().as_ref())]).await?;
    log::info!("旧数据库完整备份已保存: {:?}",backup);
    // 所有换表和写入都在同一事务内；任何一步失败时恢复旧表。
    let tx=db.acquire_begin().await?;
    let tx=tx.defer_async(|tx|async move{if !tx.done(){if let Err(error)=tx.rollback().await{log::error!("回滚迁移事务失败: {}",error);}}});
    // 从旧到新处理，同内容的后一次复制覆盖最近状态；首次时间另取最小值。
    let old:Vec<LegacyRecord>=tx.query_decode("SELECT * FROM clip_record ORDER BY sort ASC,created ASC,id ASC",vec![]).await?;
    // 先完成可失败的解密/资源读取；任何错误都让迁移回滚，而不是跳过用户记录。
    let mut converted=Vec::new();
    for row in old {
        // 旧删除标记只用于迁移过滤，不给新表增加逻辑删除字段。
        if row.del_flag==Some(1) { continue; }
        let text=row.content.as_str().unwrap_or_default();
        let mut record=ClipRecord { created_at:i64::try_from(row.created).map_err(|_|AppError::Config("旧记录时间超出范围".into()))?,last_copied_at:row.created as i64,
            sort_order:row.sort,pinned:0,..Default::default() };
        match row.r#type.as_str() {
            "Text"|"text" => {
                let original=decoder.decode(text)?;
                record.r#type=TEXT_TYPE.into();record.content_key=content_key::text_key(&original);
                record.content=Some(text.into()); // 保留旧密文，不重加密、不改变密钥。
            }
            "Image"|"image" => {
                // 优先沿用仍存在的旧绝对路径，否则在原资源目录寻找。
                let path=row.local_file_path.as_ref().map(PathBuf::from)
                    .filter(|path|path.exists()).or_else(||resources.map(|base|base.join(text)))
                    .ok_or_else(||AppError::Config("迁移图片需要提供原资源目录".into()))?;
                let mut resource=legacy_resource(&format!("clip_{}.png",row.id),path,"image",resources,0)?;
                record.r#type=IMAGE_TYPE.into();
                // 缺失图片使用带 unavailable 标记的占位身份，避免假称已核对文件内容。
                record.content_key=resource.content_hash.as_ref().map(|hash|format!("v1:image:{hash}"))
                    .unwrap_or_else(||format!("v1:unavailable:image:{}",content_key::bytes_hash(format!("{}:{}",row.id,row.md5_str).as_bytes())));
                // 图片尺寸等展示不依赖旧 content 路径。
                resource.position=0;record.resources.push(resource);
            }
            "File"|"files"|"directory" => {
                // 旧版用 ::: 分隔多文件，名称和来源路径必须一一对应。
                let names=text.split(":::").collect::<Vec<_>>();
                let paths=row.local_file_path.as_deref().unwrap_or_default().split(":::").collect::<Vec<_>>();
                if paths.len()!=names.len() || paths.iter().any(|path|path.is_empty()) {
                    return Err(AppError::Config("旧文件记录缺少有效资源信息，已保留备份并停止迁移".into()));
                }
                for (position,(name,path)) in names.iter().zip(paths).enumerate() {
                    let path=PathBuf::from(path);
                    let kind=if path.is_dir(){"directory"}else{"file"};
                    record.resources.push(legacy_resource(name,path,kind,resources,position as i32)?);
                }
                record.r#type=if record.resources.len()==1 && record.resources[0].resource_kind=="directory" {DIRECTORY_TYPE}else{FILE_TYPE}.into();
                record.content_key=content_key::resource_key(&record.resources)?;
            }
            _ => return Err(AppError::Config("存在无法识别的旧记录类型，已停止迁移并保留旧库".into())),
        }
        converted.push((record,row.pinned_flag==1));
    }
    // 旧索引名称保留在旧表上，故新表使用独立的索引名。
    tx.exec("ALTER TABLE clip_record RENAME TO clip_record_before_local_v2",vec![]).await?;
    storage::create_tables(&tx).await?;
    // 新版最多一条置顶，旧数据若有多条则保留遍历中最后一条。
    let mut pinned=None;
    for (record,is_pinned) in &converted {
        let id=storage::persist_record(&tx,record).await?;
        // 多个旧记录按新内容身份合并时，首次时间取最早值，显示顺序仍取最近复制。
        tx.exec("UPDATE clip_record SET created_at=MIN(created_at,?) WHERE id=?",vec![to_value!(record.created_at),to_value!(id)]).await?;
        if *is_pinned { pinned=Some(id); }
    }
    if let Some(id)=pinned { tx.exec("UPDATE clip_record SET pinned=1 WHERE id=?",vec![to_value!(id)]).await?; }
    tx.exec("DROP TABLE clip_record_before_local_v2",vec![]).await?;
    // 旧同步游标只保留在完整备份中，不属于单机业务数据库。
    tx.exec("DROP TABLE IF EXISTS sync_time",vec![]).await?;
    // 结构、数据和版本一起提交，不能出现版本已升级但换表未完成的状态。
    tx.exec("PRAGMA user_version=2",vec![]).await?;
    tx.commit().await?;
    log::info!("本地记录与资源表迁移成功，迁移输入 {} 条",converted.len());
    Ok(())
}

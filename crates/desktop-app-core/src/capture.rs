//! 本地采集只处理原始内容、完整摘要、加密、资源快照和本地去重。
use std::{path::{Path,PathBuf},time::{SystemTime,UNIX_EPOCH}};
use clip_core::{domain::clip::{ClipRecord,ClipRecordResource,TEXT_TYPE,IMAGE_TYPE,FILE_TYPE,DIRECTORY_TYPE},content_key,errors::{AppError,AppResult}};
use crate::LocalContext;

/// 从系统事件借用的原始数据；此处尚未计算摘要、加密或复制文件。
pub enum CaptureInput<'a>{
    /// 剪贴板原文，不提前去空白或截断。
    Text(&'a str),
    /// 图片文件字节；None 表示事件没有附带可用图片。
    Image(Option<&'a Vec<u8>>),
    /// 原始文件和目录路径；None 表示事件没有附带路径。
    Files(Option<&'a Vec<String>>)
}
/// 文本加密能力由桌面层注入，单机业务不直接读取打包密钥。
pub trait TextEncoder:Send+Sync{
    /// 把完整原文编码成数据库可保存的密文；失败则不入库。
    fn encode(&self,content:&str)->AppResult<String>;
}
/// 普通函数和闭包也可作为加密器，便于复用桌面实现和隔离测试。
impl<F:Fn(&str)->AppResult<String>+Send+Sync> TextEncoder for F{
    /// 将接口调用转交给注入的函数，不额外改变文本。
    fn encode(&self,content:&str)->AppResult<String>{self(content)}
}

/// 本地采集服务：先准备内容和资源，再提交数据库，最后更新索引和旧缓存。
pub struct CaptureService<'a>{local:&'a LocalContext,encoder:&'a dyn TextEncoder}
impl<'a> CaptureService<'a>{
    /// 借用当前应用上下文和加密器，不创建额外的全局状态。
    pub fn new(local:&'a LocalContext,encoder:&'a dyn TextEncoder)->Self{Self{local,encoder}}
    /// 保存一个有效剪贴板事件；空内容返回 None，保存失败返回具体后端错误。
    pub async fn capture(&self,input:CaptureInput<'_>)->AppResult<Option<ClipRecord>>{
        // 与删除、置顶和超限清理互斥，避免资源准备期间记录被另一操作删掉。
        let _guard=self.local.capture_lock.lock().await;
        // 毫秒时间用于展示；真正的先后顺序由仓储在写事务内分配。
        let now=i64::try_from(SystemTime::now().duration_since(UNIX_EPOCH).map_err(|error|AppError::System(error.to_string()))?.as_millis()).map_err(|_|AppError::System("时间超出范围".into()))?;
        let mut next=ClipRecord{created_at:now,last_copied_at:now,..Default::default()};
        let mut search=String::new();
        match input{
            CaptureInput::Text(text)=>{
                // 只忽略零长度，不修改首尾空白、缩进、换行或纯空白内容。
                if text.is_empty(){return Ok(None);}
                next.r#type=TEXT_TYPE.into();next.content_key=content_key::text_key(text);
                next.content=Some(self.encoder.encode(text)?);search=text.into();
            }
            CaptureInput::Image(Some(data))=>{
                next.r#type=IMAGE_TYPE.into();next.content_key=content_key::image_key(data);
                let old=self.local.repositories().clip_records().find_by_content_key(&next.content_key).await?;
                // 相同图片且缓存仍存在时复用文件；缓存丢失则重新保存。
                if let Some(old)=old.filter(|old|old.resources.iter().all(|resource|self.resolve(resource).is_ok_and(|path|path.is_file()))){next.resources=old.resources;}
                else{
                    let base=self.local.resources_dir().ok_or_else(||AppError::Config("图片资源目录不可用".into()))?;
                    let relative=format!("images/{}.png",uuid::Uuid::new_v4());
                    // 文件名用随机值，图片内容身份由 SHA-256 独立确定。
                    let path=base.join(&relative);std::fs::create_dir_all(path.parent().unwrap())?;
                    if let Err(error)=std::fs::write(&path,data){let _=std::fs::remove_file(path);return Err(error.into());}
                    next.resources.push(ClipRecordResource{display_name:"clipboard.png".into(),resource_kind:"image".into(),storage_kind:"managed".into(),path:relative,byte_size:Some(data.len() as i64),content_hash:Some(content_key::bytes_hash(data)),..Default::default()});
                }
            }
            CaptureInput::Files(Some(paths)) if !paths.is_empty()=>{
                // 磁盘操作放到阻塞线程；任何一项失败都不保存不完整的文件清单。
                let paths=paths.clone();let base=self.local.resources_dir();
                next.resources=tokio::task::spawn_blocking(move||prepare_files(&paths,base.as_deref())).await.map_err(|error|AppError::System(error.to_string()))??;
                next.r#type=if next.resources.len()==1 && next.resources[0].resource_kind=="directory"{DIRECTORY_TYPE}else{FILE_TYPE}.into();
                next.content_key=content_key::resource_key(&next.resources)?;
                // 文件搜索使用显示名，避免随机缓存路径影响搜索结果。
                search=next.search_names();
            }
            _=>return Ok(None),
        }
        let repo=self.local.repositories().clip_records();
        let old=repo.find_by_content_key(&next.content_key).await?;
        // 先保留旧资源快照：写失败清理新副本，写成功才清理被替换的旧副本。
        let saved=match repo.save_capture(&next).await{
            Ok(saved)=>saved,
            Err(error)=>{
                if let Some(old)=&old{crate::cleanup::delete_replaced_resources(self.local,&next,old).await;}
                else{crate::cleanup::delete_managed_resources(self.local,&next).await;}
                return Err(error);
            }
        };
        if let Some(old)=old{crate::cleanup::delete_replaced_resources(self.local,&old,&saved).await;}
        if !search.is_empty(){
            // 数据库提交后索引失败只记日志，不能误报为整次采集没有保存。
            if let Err(error)=self.local.search_engine().add(&saved.id.to_string(),&search).await{log::error!("本地索引更新失败: {}",error);}
        }
        Ok(Some(saved))
    }
    /// 复用统一路径校验，不能仅凭数据库字符串信任资源位于缓存目录。
    fn resolve(&self,resource:&ClipRecordResource)->AppResult<PathBuf>{crate::records::resolve_resource(self.local,resource)}
}

/// 为多选文件建立快照，目录保留引用；失败时清理本次已创建的缓存。
fn prepare_files(paths:&[String],base:Option<&Path>)->AppResult<Vec<ClipRecordResource>>{
    let mut resources=Vec::new();
    let result=(||{
        for (index,path) in paths.iter().enumerate(){
            // 规范化实际来源，但显示名保留用户原来选择的文件名。
            let source=Path::new(path).canonicalize()?;
            let meta=std::fs::metadata(&source)?;
            let name=Path::new(path).file_name().and_then(|name|name.to_str()).ok_or_else(||AppError::Config("文件名不是有效 UTF-8".into()))?;
            let mut resource=ClipRecordResource{position:index as i32,display_name:name.into(),resource_kind:if meta.is_dir(){"directory"}else{"file"}.into(),storage_kind:"reference".into(),path:source.to_string_lossy().into_owned(),..Default::default()};
            if meta.is_file(){
                // 对副本本身计算摘要，确保摘要与最终保存的字节一致。
                if let Some(base)=base{
                    let extension=clip_core::files::extensions::extract_full_extension(Path::new(name));
                    // 保留 tar.gz 等完整扩展名，随机命名避免覆盖同名文件。
                    let relative=format!("files/{}{}",uuid::Uuid::new_v4(),if extension.is_empty(){String::new()}else{format!(".{extension}")});
                    let target=base.join(&relative);std::fs::create_dir_all(target.parent().unwrap())?;
                    match std::fs::copy(&source,&target){
                        Ok(_)=>{resource.storage_kind="managed".into();resource.path=relative;}
                        Err(error)=>{let _=std::fs::remove_file(&target);log::warn!("缓存复制失败，保留外部引用: {}",error);}
                    }
                }
                // 先登记副本，后续校验失败时也能清理。
                resources.push(resource);
                let resource=resources.last_mut().unwrap();
                let actual=if resource.storage_kind=="managed"{base.unwrap().join(&resource.path)}else{source};
                resource.content_hash=Some(content_key::file_hash(&actual)?);
                resource.byte_size=Some(i64::try_from(std::fs::metadata(actual)?.len()).map_err(|_|AppError::Config("文件大小超出范围".into()))?);
            // 目录不复制、不递归计算摘要，删除历史时也不能删除目录原件。
            }else if meta.is_dir(){resources.push(resource);}
            else{return Err(AppError::Config("不支持该文件系统对象".into()));}
        }
        Ok(())
    })();
    if let Err(error)=result{
        // 整组准备失败就回收已创建副本，不能保存只有一部分附件的记录。
        if let Some(base)=base{for resource in resources.iter().filter(|resource|resource.storage_kind=="managed"){let _=std::fs::remove_file(base.join(&resource.path));}}
        return Err(error);
    }
    Ok(resources)
}

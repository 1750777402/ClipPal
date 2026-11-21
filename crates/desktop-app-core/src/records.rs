//! 本地查询和页面 DTO 适配。数据库自增 ID 通过 IPC 以字符串传输，避免 JS 整数精度损失。
use std::{collections::HashMap,path::{Path,PathBuf}};
use clip_core::{domain::clip::{ClipRecord,ClipRecordResource,TEXT_TYPE,IMAGE_TYPE,FILE_TYPE,DIRECTORY_TYPE},errors::{AppError,AppResult},repositories::ClipRecordRepository};
use crate::{LocalContext,dto::*};

/// 将 IPC 十进制字符串还原成正整数主键，拒绝空值、负数和溢出值。
pub fn parse_id(value:&str)->AppResult<i64>{
    value.parse::<i64>().ok().filter(|id|*id>0).ok_or_else(||AppError::Config("记录 ID 无效".into()))
}

/// 受管路径必须位于资源根内，外部引用只读取，不授予删除权限。
pub fn resolve_resource(local:&LocalContext,resource:&ClipRecordResource)->AppResult<PathBuf>{
    if resource.storage_kind=="reference"{
        let path=PathBuf::from(&resource.path);
        if !path.is_absolute(){return Err(AppError::Config("外部资源路径必须为绝对路径".into()));}
        return Ok(path);
    }
    if resource.storage_kind!="managed" || Path::new(&resource.path).is_absolute(){return Err(AppError::Config("资源存储路径无效".into()));}
    let base=local.resources_dir().ok_or_else(||AppError::Config("资源目录不可用".into()))?.canonicalize()?;
    let path=base.join(&resource.path);
    // 不存在仍返回候选路径供展示缺失；存在时防止符号链接和 .. 越界。
    if resource.path.split(['/', '\\']).any(|part|part==".."){return Err(AppError::Config("资源路径越界".into()));}
    if path.exists() && !path.canonicalize()?.starts_with(&base){return Err(AppError::Config("资源路径越界".into()));}
    Ok(path)
}

/// 本地记录业务入口。负责校验和页面数据转换，系统剪贴板操作由桌面层执行。
pub struct ClipRecordService<'a>{context:&'a LocalContext,repository:&'a dyn ClipRecordRepository}
impl<'a> ClipRecordService<'a>{
    /// 从同一个应用上下文取得仓储、解密器和搜索索引。
    pub fn new(context:&'a LocalContext)->Self{Self{context,repository:context.repositories().clip_records()}}
    /// 查询一页轻量记录；有关键词时先搜候选 ID，再回查数据库排序分页。
    pub async fn query_clip_records(&self,param:QueryParam)->AppResult<Vec<ClipRecordLiteDTO>>{
        if param.page<1 || param.size<1{return Err(AppError::Config("页码和每页条数必须大于零".into()));}
        // 页码从 1 开始；使用较宽整数并检查乘法，避免偏移量溢出。
        let offset=(param.page as i64-1).checked_mul(param.size as i64).ok_or_else(||AppError::Config("分页超出范围".into()))?;
        let records=if let Some(search)=param.search.as_deref().filter(|search|!search.is_empty()){
            let ids=self.context.search_engine().search(search).await;
            self.repository.list_by_ids(&ids,param.size,offset).await?
        }else{self.repository.list(param.size,offset).await?};
        records.into_iter().map(|record|self.to_dto(record)).collect()
    }
    /// 转换成原有页面协议：保留 Text/Image/File 名称，隐藏存储层细节。
    fn to_dto(&self,record:ClipRecord)->AppResult<ClipRecordLiteDTO>{
        let mut content;let mut truncated=false;let mut length=None;let mut file_info=vec![];
        let kind=match record.r#type.as_str(){
            TEXT_TYPE=>{
                content=self.context.decoder().decode(record.content.as_deref().ok_or_else(||AppError::Config("文本内容缺失".into()))?)?;
                // 列表最多返回 8KB 预览；截断点退到 UTF-8 字符边界，避免切坏中文。
                if content.len()>8*1024{length=Some(content.len());let mut end=8*1024;while !content.is_char_boundary(end){end-=1;}content.truncate(end);truncated=true;}
                "Text"
            }
            IMAGE_TYPE=>{content=record.resources.first().map(|resource|resource.path.clone()).unwrap_or_default();"Image"}
            FILE_TYPE|DIRECTORY_TYPE=>{
                // 资源名用 JSON 数组表达，同名文件仍通过 resource_id 区分。
                content=serde_json::to_string(&record.resources.iter().map(|resource|&resource.display_name).collect::<Vec<_>>())?;
                file_info=record.resources.iter().map(|resource|{
                    // 实时检查资源是否还在；缺失返回 -1，列表仍可展示该历史记录。
                    let size=resolve_resource(self.context,resource).ok().and_then(|path|std::fs::metadata(path).ok()).map(|meta|meta.len().min(i64::MAX as u64) as i64).unwrap_or(-1);
                    FileInfo{resource_id:resource.id.to_string(),path:resource.display_name.clone(),size,
                        r#type:if resource.resource_kind=="directory"{"directory".into()}else{Path::new(&resource.display_name).extension().and_then(|ext|ext.to_str()).unwrap_or("unknown").to_lowercase()}}
                }).collect();"File"
            }
            _=>return Err(AppError::Config("记录类型无效".into())),
        };
        // IPC 的 ID 使用字符串，避免前端 number 无法精确表示较大的 i64。
        Ok(ClipRecordLiteDTO{id:record.id.to_string(),r#type:kind.into(),content,created:record.last_copied_at.max(0) as u64,pinned_flag:record.pinned,file_info,
            content_truncated:truncated,original_content_length:length,has_image:record.r#type==IMAGE_TYPE})
    }
    /// 只为真实存在的图片返回校验后的路径，前端按需加载图片文件。
    pub async fn get_image_path(&self,id:&str)->AppResult<ImagePathInfo>{
        let record=self.required(id).await?;
        if record.r#type!=IMAGE_TYPE{return Err(AppError::Config("记录不是图片类型".into()));}
        let resource=record.resources.first().ok_or_else(||AppError::Config("图片资源缺失".into()))?;
        let path=resolve_resource(self.context,resource)?;
        if !path.is_file(){return Err(AppError::Config("图片文件不存在".into()));}
        Ok(ImagePathInfo{id:record.id.to_string(),file_path:path.to_string_lossy().into_owned(),protocol_url:String::new()})
    }
    /// 批量返回可用图片信息；单张图片缺失时跳过，不影响其他图片。
    pub async fn get_image_info_batch(&self,ids:Vec<String>)->AppResult<HashMap<String,ImageInfo>>{
        let mut result=HashMap::new();
        for id in ids{
            if let Ok(image)=self.get_image_path(&id).await{
                if let Ok(meta)=std::fs::metadata(&image.file_path){result.insert(id,ImageInfo{path:image.file_path,size:meta.len(),width:None,height:None});}
            }
        }Ok(result)
    }
    /// 用户展开文本时再解密返回全文，不使用列表中被截断的预览。
    pub async fn get_full_text_content(&self,id:String)->AppResult<FullContentResponse>{
        let record=self.required(&id).await?;
        if record.r#type!=TEXT_TYPE{return Err(AppError::Config("记录类型不是文本".into()));}
        let content=self.context.decoder().decode(record.content.as_deref().ok_or_else(||AppError::Config("文本内容缺失".into()))?)?;
        Ok(FullContentResponse{id:record.id.to_string(),content_length:content.len(),content})
    }
    /// 校验主键并切换置顶；与采集、删除共用锁，保持记录状态一致。
    pub async fn set_pinned(&self,id:&str,pinned:i32)->AppResult<String>{
        let _guard=self.context.capture_lock.lock().await;
        self.repository.set_pinned(parse_id(id)?,pinned).await?;Ok(String::new())
    }
    /// 物理删除记录；数据库提交成功后再移除索引并回收受管缓存。
    pub async fn delete_record(&self,id:String)->AppResult<String>{
        let _guard=self.context.capture_lock.lock().await;
        if let Some(record)=self.repository.delete(parse_id(&id)?).await?{
            // 数据库已经提交删除；索引失败不能阻断缓存清理或让页面误以为记录未删除。
            // 查询仍会回查数据库，失效索引不会让已删除记录重新显示。
            if let Err(error)=self.context.search_engine().remove(&[record.id.to_string()]).await {
                log::error!("移除已删除记录的搜索索引失败，ID: {}，错误: {}",record.id,error);
            }
            crate::cleanup::delete_managed_resources(self.context,&record).await;
        }Ok(String::new())
    }
    /// 读取必须存在的记录，统一处理非法 ID 和记录已删除的情况。
    async fn required(&self,id:&str)->AppResult<ClipRecord>{
        self.repository.find_by_id(parse_id(id)?).await?.ok_or_else(||AppError::Config("记录不存在".into()))
    }
}

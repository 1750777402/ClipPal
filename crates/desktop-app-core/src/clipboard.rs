//! 将本地记录解析成系统剪贴板输入。实际系统写入由桌面适配执行。
use std::path::PathBuf;
use clip_core::{domain::clip::{ClipRecord,TEXT_TYPE,IMAGE_TYPE,FILE_TYPE,DIRECTORY_TYPE},errors::{AppError,AppResult}};
use crate::{LocalContext,records::resolve_resource};

/// 已通过本地校验、可交给系统剪贴板层写入的数据，不含数据库或加密细节。
#[derive(Debug,PartialEq)]
pub enum ClipboardPayload{
    /// 解密后的完整文本，保留原始空白和换行。
    Text(String),
    /// 已校验存在的图片路径，不把图片字节放进 IPC 响应。
    Image(PathBuf),
    /// 显示名与实际来源路径一一对应，目录标记供系统适配选择写入方式。
    Files{names:Vec<String>,paths:Vec<String>,has_directories:bool},
}

/// 资源 ID 必须属于当前记录；同名文件使用资源 ID 区分，不按名称猜测。
pub fn prepare_copy(context:&LocalContext,record:&ClipRecord,resource_id:Option<i64>)->AppResult<ClipboardPayload>{
    // 单资源复制只适用于文件/目录；文本和图片只能整条复制。
    match record.r#type.as_str(){
        TEXT_TYPE if resource_id.is_none()=>Ok(ClipboardPayload::Text(context.decoder().decode(record.content.as_deref().ok_or_else(||AppError::Config("文本内容缺失".into()))?)?)),
        IMAGE_TYPE if resource_id.is_none()=>{
            let resource=record.resources.first().ok_or_else(||AppError::Config("图片资源缺失".into()))?;
            let path=resolve_resource(context,resource)?;
            if !path.is_file(){return Err(AppError::Config("图片文件不存在".into()));}
            Ok(ClipboardPayload::Image(path))
        }
        FILE_TYPE|DIRECTORY_TYPE=>{
            // 有资源 ID 时只选属于当前记录的那一项，拒绝跨记录引用。
            let resources=if let Some(id)=resource_id{
                vec![record.resources.iter().find(|resource|resource.id==id).ok_or_else(||AppError::Config("资源不属于该记录".into()))?]
            }else{record.resources.iter().collect::<Vec<_>>()};
            if resources.is_empty(){return Err(AppError::Config("文件资源缺失".into()));}
            let mut names=Vec::new();let mut paths=Vec::new();let mut directories=false;
            for resource in resources{
                // 所有来源都准备成功后才返回，避免系统收到缺项的文件列表。
                let path=resolve_resource(context,resource)?;
                if !path.exists(){return Err(AppError::Config(format!("文件不存在: {}",resource.display_name)));}
                directories|=resource.resource_kind=="directory";
                names.push(resource.display_name.clone());paths.push(path.to_string_lossy().into_owned());
            }
            Ok(ClipboardPayload::Files{names,paths,has_directories:directories})
        }
        _=>Err(AppError::Config("记录类型不支持该复制操作".into())),
    }
}

//! 内容身份规则；与设备、数据库 ID、加密随机数及云端状态无关。
use std::{io::Read, path::Path};
use sha2::{Digest, Sha256};
use crate::{domain::clip::ClipRecordResource, errors::AppResult};

/// 计算原始字节的完整 SHA-256，输出固定长度的小写十六进制摘要。
pub fn bytes_hash(bytes: &[u8]) -> String { format!("{:x}", Sha256::digest(bytes)) }
/// 分块读完整文件；内存只保留一个读缓冲，不把大文件一次性加载进来。
pub fn file_hash(path: &Path) -> AppResult<String> {
    let mut reader = std::fs::File::open(path)?;
    let mut hasher = Sha256::new();
    // 每次读 64KB，避免旧采样方案把仅中间内容不同的文件当成同一个。
    let mut bytes = [0u8; 64 * 1024];
    loop { let n=reader.read(&mut bytes)?; if n==0 { break; } hasher.update(&bytes[..n]); }
    Ok(format!("{:x}",hasher.finalize()))
}
/// 文本以加密前的完整 UTF-8 原文确定身份，首尾空白和换行参与计算。
pub fn text_key(text: &str) -> String { format!("v1:text:{}",bytes_hash(text.as_bytes())) }
/// 图片以文件字节确定身份；版本和类型前缀避免与其他内容类型混淆。
pub fn image_key(bytes: &[u8]) -> String { format!("v1:image:{}",bytes_hash(bytes)) }
/// 把资源清单转成稳定的内容身份；多选顺序变化不产生新的记录。
pub fn resource_key(resources: &[ClipRecordResource]) -> AppResult<String> {
    // 目录只表示本机路径引用，不递归读取目录，也不表示跨设备目录相同。
    if resources.len()==1 && resources[0].resource_kind=="directory" {
        return Ok(format!("v1:directory:{}",bytes_hash(resources[0].path.as_bytes())));
    }
    // 单文件忽略显示名，多资源清单包含名称、类型和完整摘要；排序后保留重复项。
    let mut entries = resources.iter().map(|resource| {
        // 缺失旧附件只能暂用路径占位，不能拿旧 MD5 冒充完整 SHA-256。
        let identity = resource.content_hash.clone().unwrap_or_else(|| format!("path:{}",resource.path));
        (if resources.len()==1 { String::new() } else { resource.display_name.clone() },resource.resource_kind.clone(),identity)
    }).collect::<Vec<_>>();
    // 使用结构化 JSON 避免字符串拼接歧义；排序不去重，保留重复文件项。
    entries.sort();
    Ok(format!("v1:files:{}",bytes_hash(&serde_json::to_vec(&entries)?)))
}

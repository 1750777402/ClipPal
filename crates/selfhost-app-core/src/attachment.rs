//! 单附件协议校验和本地文件准备。这里不会把其他设备的路径当作本机文件路径。
use clip_sync_client::{protocol::{Attachment, SyncLimits, DEFAULT_RECORD_BYTES}, SyncError};
use desktop_app_core::{
    clip_core::{content_key, domain::clip::{ClipRecord, ClipRecordResource, TEXT_TYPE}},
    LocalContext,
};

/// 对象仍以操作标识命名，只追加可安全放入 OSS 路径的后缀。
pub(crate) fn object_id(operation_id: &str, name: &str) -> String {
    use desktop_app_core::clip_core::files::extensions::extract_full_extension_from_str;
    let suffix = extract_full_extension_from_str(name).to_ascii_lowercase();
    if valid_suffix(&suffix) { format!("{operation_id}.{suffix}") } else { operation_id.to_owned() }
}

fn valid_suffix(suffix: &str) -> bool {
    let valid_part = |part: &str| {
        (1..=16).contains(&part.len()) && part.bytes().all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit())
    };
    match suffix.split_once('.') {
        Some((first, last)) => valid_part(first) && valid_part(last),
        None => valid_part(suffix),
    }
}

/// 保留对已上传的无后缀对象的读取能力。
fn valid_object_id(value: &str) -> bool {
    let (id, suffix) = value.split_once('.').map_or((value, None), |(id, suffix)| (id, Some(suffix)));
    id.len() == 32 && id.bytes().all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        && suffix.is_none_or(valid_suffix)
}

/// 采集和记录卡片共用资格判断。同步限制只影响入队，本地记录照常保留。
pub fn skip_reason(record: &ClipRecord, limits: Option<SyncLimits>, text_bytes: usize) -> Option<&'static str> {
    let max = limits.map_or(DEFAULT_RECORD_BYTES, |limits| limits.max_record_bytes);
    if record.r#type == TEXT_TYPE {
        return (text_bytes > max).then_some("文本超过同步大小上限");
    }
    if record.resources.len() > 1 {
        return Some("多文件记录仅保存在本机");
    }
    let [resource] = record.resources.as_slice() else { return Some("记录资源缺失"); };
    if resource.resource_kind == "directory" {
        return Some("目录仅保存在本机");
    }
    if resource.storage_kind != "managed" || resource.byte_size.is_none() {
        return Some("记录缺少可同步的本地副本");
    }
    if !matches!(record.r#type.as_str(), "image" | "files") || !valid_name(&resource.display_name) {
        return Some("记录类型或文件名不支持跨设备同步");
    }
    if resource.byte_size.is_some_and(|size| size < 0 || size as u64 > max as u64) {
        return Some("附件超过同步大小上限");
    }
    limits.is_some_and(|limits| !limits.attachments).then_some("当前部署未启用附件同步")
}

/// 记录类型由 content_key 决定。文件沿用单文件内容身份，忽略显示名。
pub(crate) fn content_identity(kind: &str, hash: &str) -> Option<String> {
    match kind {
        "image" => Some(format!("image:{hash}")),
        "files" => content_key::resource_key(&[ClipRecordResource {
            resource_kind: "file".into(),
            content_hash: Some(hash.into()),
            ..Default::default()
        }]).ok(),
        _ => None,
    }
}

/// 只接受跨平台文件名。远端名字仅用于显示和恢复粘贴文件名。
pub(crate) fn valid_name(name: &str) -> bool {
    if name.is_empty() || name.len() > 255 || name.ends_with([' ', '.'])
        || name.chars().any(|value| value.is_control() || "/\\:*?\"<>|".contains(value))
    {
        return false;
    }
    let stem = name.split('.').next().unwrap_or_default().trim_end().to_ascii_uppercase();
    !matches!(stem.as_str(), "CON" | "PRN" | "AUX" | "NUL" | "CONIN$" | "CONOUT$")
        && !["COM", "LPT"].iter().any(|prefix| stem.strip_prefix(prefix).is_some_and(|tail| {
            matches!(tail, "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9" | "¹" | "²" | "³")
        }))
}

pub(crate) fn digest(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

pub(crate) fn kind(key: &str) -> Result<&str, SyncError> {
    match key.split_once(':') {
        Some((kind @ ("text" | "image" | "files"), hash)) if digest(hash) => Ok(kind),
        _ => Err(SyncError::Blocked("同步内容键无效".into())),
    }
}

/// 验证密文中的稳定对象引用和内容键，下载地址由服务端签发。
pub(crate) fn decode(local: &LocalContext, key: &str, ciphertext: &str) -> Result<Attachment, SyncError> {
    let plaintext = local.decoder().decode(ciphertext)
        .map_err(|_| SyncError::Blocked("无法解密附件描述".into()))?;
    let attachment: Attachment = serde_json::from_str(&plaintext)
        .map_err(|_| SyncError::Blocked("附件描述格式无效".into()))?;
    if !valid_object_id(&attachment.object_id)
        || !valid_name(&attachment.name) || !digest(&attachment.sha256)
        || attachment.size > DEFAULT_RECORD_BYTES
        || content_identity(kind(key)?, &attachment.sha256).as_deref() != Some(key)
    {
        return Err(SyncError::Blocked("附件描述与同步内容不匹配".into()));
    }
    Ok(attachment)
}

/// 先校验字节，再写入随机命名的受管文件。对象 ID 和远端名称均不用作本机路径。
pub(crate) async fn prepare(
    local: &LocalContext, kind: &str, manifest: &Attachment, bytes: Vec<u8>,
) -> Result<Prepared, SyncError> {
    if bytes.len() != manifest.size || content_key::bytes_hash(&bytes) != manifest.sha256 {
        return Err(SyncError::Blocked("OSS 附件与同步记录的摘要不一致".into()));
    }
    let image = kind == "image";
    let directory = if image { "images" } else { "files" };
    let root = if image { local.resources_dir() } else { local.file_backup_dir() }
        .ok_or_else(|| SyncError::Retry("附件备份目录不可用".into()))?;
    let extension = if image { "png".into() } else {
        desktop_app_core::clip_core::files::extensions::extract_full_extension(
            std::path::Path::new(&manifest.name))
    };
    let suffix = if extension.is_empty() { String::new() } else { format!(".{extension}") };
    let relative = format!("{directory}/{}{suffix}", uuid::Uuid::new_v4());
    let folder = root.join(directory);
    let path = root.join(&relative);
    let disk_error = |error| SyncError::Retry(format!("无法写入同步附件：{error}"));
    tokio::fs::create_dir_all(&folder).await.map_err(disk_error)?;
    if !tokio::fs::canonicalize(&folder).await.map_err(disk_error)?
        .starts_with(tokio::fs::canonicalize(&root).await.map_err(disk_error)?)
    {
        return Err(SyncError::Blocked("附件备份目录越界".into()));
    }
    let file = tokio::fs::OpenOptions::new().create_new(true).write(true)
        .open(&path).await.map_err(disk_error)?;
    let prepared = Prepared {
        path: Some(path),
        resource: ClipRecordResource {
            display_name: manifest.name.clone(),
            resource_kind: if image { "image" } else { "file" }.into(),
            storage_kind: "managed".into(), path: relative, byte_size: Some(manifest.size as i64),
            content_hash: Some(manifest.sha256.clone()), ..Default::default()
        },
    };
    use tokio::io::AsyncWriteExt;
    let mut file = file;
    file.write_all(&bytes).await.map_err(disk_error)?;
    file.sync_all().await.map_err(disk_error)?;
    Ok(prepared)
}

/// 补拉失败或取消时回收已准备的文件，SQLite 提交后才留下实际使用的文件。
pub(crate) struct Prepared {
    path: Option<std::path::PathBuf>,
    pub(crate) resource: ClipRecordResource,
}

impl Prepared {
    pub(crate) fn keep(&mut self) { self.path.take(); }
}

impl Drop for Prepared {
    fn drop(&mut self) {
        if let Some(path) = self.path.take() {
            let _ = std::fs::remove_file(path);
        }
    }
}

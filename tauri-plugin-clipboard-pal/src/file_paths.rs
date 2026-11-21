//! 系统剪贴板的路径适配：macOS/Windows 使用本地路径，Linux 使用 file URI。
use std::path::Path;
use url::Url;

pub(crate) fn local_paths(files: Vec<String>) -> Result<Vec<String>, String> {
    if files.is_empty() { return Err("文件列表为空".into()); }
    files.into_iter().map(|file| {
        let path = if file.starts_with("file:") {
            let url = Url::parse(&file).map_err(|error| format!("文件地址无效: {}", error))?;
            if url.host_str().is_some_and(|host| !host.is_empty() && host != "localhost") {
                return Err("只支持本地文件地址".into());
            }
            url.to_file_path().map_err(|_| "文件地址无法转换为本地路径".to_string())?
        } else { Path::new(&file).to_path_buf() };
        if !path.is_absolute() { return Err("文件路径必须是绝对路径".into()); }
        Ok(path.to_string_lossy().into_owned())
    }).collect()
}

pub(crate) fn clipboard_paths(files: Vec<String>) -> Result<Vec<String>, String> {
    let paths = local_paths(files)?;
    #[cfg(target_os = "linux")]
    { paths.into_iter().map(|path| Url::from_file_path(path)
        .map(|url| url.to_string()).map_err(|_| "无法生成文件地址".into())).collect() }
    #[cfg(not(target_os = "linux"))]
    { Ok(paths) }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn roundtrips_spaces_unicode_percent_and_hash_in_local_file_urls() {
        let path = std::env::temp_dir().join("中文 空格%#文件.txt");
        let native = path.to_string_lossy().into_owned();
        let uri = Url::from_file_path(&path).unwrap().to_string();
        assert_eq!(local_paths(vec![uri]).unwrap(), vec![native.clone()]);
        let clipboard = clipboard_paths(vec![native.clone()]).unwrap();
        assert_eq!(local_paths(clipboard).unwrap(), vec![native]);
    }
    #[test]
    fn rejects_relative_remote_and_empty_paths() {
        for files in [vec![], vec!["relative.txt".into()], vec!["file://other-host/share/file".into()]] {
            assert!(local_paths(files).is_err());
        }
    }
}

/// 只回收本应用命名空间内已不再被系统剪贴板引用的临时目录。
pub(crate) fn remove_inactive_temp_dirs(previous: &[String], current: &[String]) {
    let root = std::env::temp_dir().join("clip_pal_temp");
    let Ok(root) = root.canonicalize() else { return; };
    let operation = |file: &str| -> Option<std::path::PathBuf> {
        let path = Path::new(file).parent()?.parent()?;
        uuid::Uuid::parse_str(path.file_name()?.to_str()?).ok()?;
        let metadata = std::fs::symlink_metadata(path).ok()?;
        if metadata.file_type().is_symlink() || !metadata.is_dir() { return None; }
        let path = path.canonicalize().ok()?;
        (path.parent() == Some(root.as_path())).then_some(path)
    };
    let active: std::collections::HashSet<_> = current.iter().filter_map(|path| operation(path)).collect();
    let obsolete: std::collections::HashSet<_> = previous.iter().filter_map(|path| operation(path))
        .filter(|path| !active.contains(path)).collect();
    for path in obsolete {
        if let Err(error) = std::fs::remove_dir_all(&path) { log::warn!("清理旧剪贴板临时文件失败: {}", error); }
    }
}

#[cfg(test)]
mod retention_tests {
    use super::*;
    #[test]
    fn retains_active_copy_and_reclaims_it_only_after_replacement() {
        let root = std::env::temp_dir().join("clip_pal_temp");
        let operation = root.join(uuid::Uuid::new_v4().to_string());
        let file = operation.join("0/中文.txt");
        std::fs::create_dir_all(file.parent().unwrap()).unwrap();
        std::fs::write(&file, "clipboard").unwrap();
        let paths = vec![file.to_string_lossy().into_owned()];
        remove_inactive_temp_dirs(&paths, &paths);
        assert!(file.exists());
        remove_inactive_temp_dirs(&paths, &[]);
        assert!(!operation.exists());
    }
    #[test]
    fn never_removes_external_source_directories() {
        let root = std::env::temp_dir().join(format!("clipboard-source-{}", uuid::Uuid::new_v4()));
        let file = root.join("source.txt");
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(&file, "source").unwrap();
        remove_inactive_temp_dirs(&[file.to_string_lossy().into_owned()], &[]);
        assert!(file.exists());
        std::fs::remove_dir_all(root).unwrap();
    }
}

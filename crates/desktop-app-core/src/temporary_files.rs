//! 本地文件复制时恢复原始显示名称及临时资源清理。
/// 为文件记录创建带显示名称的临时硬链接，必要时退回文件复制。
pub async fn create_named_temp_files(
    display_names: &[String],
    actual_paths: &[String],
) -> Result<Vec<String>, String> {
    use std::path::{Component, Path};

    if display_names.len() != actual_paths.len() {
        return Err("显示名称和实际路径数量不匹配".to_string());
    }

    // 在创建任何文件前验证所有输入，避免处理中途失败后留下部分临时文件。
    let mut file_entries = Vec::new();
    for (display_name, actual_path) in display_names.iter().zip(actual_paths.iter()) {
        let actual_path = actual_path.as_str();
        let display_name = display_name.as_str();

        if actual_path.is_empty() || display_name.is_empty() {
            continue;
        }

        let mut components = Path::new(display_name).components();
        let is_single_file_name = matches!(components.next(), Some(Component::Normal(name)) if name == display_name)
            && components.next().is_none();
        if !is_single_file_name {
            return Err(format!("文件显示名称包含无效路径: {}", display_name));
        }

        let source_path = Path::new(actual_path);
        if !source_path.exists() {
            return Err(format!("源文件不存在: {}", actual_path));
        }

        file_entries.push((display_name, source_path));
    }

    if file_entries.is_empty() {
        return Err("没有可创建的临时文件".to_string());
    }

    let temp_root = std::env::temp_dir().join("clip_pal_temp");
    std::fs::create_dir_all(&temp_root).map_err(|error| format!("创建临时目录失败: {}", error))?;
    let temp_root_metadata = std::fs::symlink_metadata(&temp_root)
        .map_err(|error| format!("检查临时目录失败: {}", error))?;
    if !temp_root_metadata.file_type().is_dir() || temp_root_metadata.file_type().is_symlink() {
        return Err("临时目录不是受支持的本地目录".to_string());
    }

    // 每次复制使用独立目录，避免并发操作和同名文件互相覆盖。
    let operation_temp_dir = temp_root.join(uuid::Uuid::new_v4().to_string());
    std::fs::create_dir(&operation_temp_dir)
        .map_err(|error| format!("创建本次临时目录失败: {}", error))?;

    let mut temp_file_paths = Vec::new();
    for (index, (display_name, source_path)) in file_entries.into_iter().enumerate() {
        // 同名文件分别放入独立子目录，目标应用仍会看到原始显示名称。
        let file_temp_dir = operation_temp_dir.join(index.to_string());
        if let Err(error) = std::fs::create_dir(&file_temp_dir) {
            let _ = std::fs::remove_dir_all(&operation_temp_dir);
            return Err(format!("创建文件临时目录失败: {}", error));
        }
        let temp_file_path = file_temp_dir.join(display_name);

        // 同盘优先使用硬链接，跨盘或文件系统不支持时复制文件。
        match std::fs::hard_link(source_path, &temp_file_path) {
            Ok(_) => temp_file_paths.push(temp_file_path.to_string_lossy().to_string()),
            Err(_) => {
                if let Err(error) = std::fs::copy(source_path, &temp_file_path) {
                    let _ = std::fs::remove_dir_all(&operation_temp_dir);
                    return Err(format!("创建临时文件失败: {}", error));
                }
                temp_file_paths.push(temp_file_path.to_string_lossy().to_string());
            }
        }
    }

    // 临时文件的生命周期由系统剪贴板适配器管理；内容被替换后才回收。
    Ok(temp_file_paths)
}

/// 删除临时文件及目录；目录不存在时视为已经清理完成。
#[cfg(test)]
async fn cleanup_temp_files(temp_dir: &std::path::Path) -> Result<(), String> {
    if !temp_dir.exists() {
        return Ok(());
    }

    std::fs::remove_dir_all(temp_dir).map_err(|error| format!("清理临时目录失败: {}", error))?;

    // 没有其他复制任务时顺带删除空的公共根目录。
    if let Some(temp_root) = temp_dir.parent() {
        let _ = std::fs::remove_dir(temp_root);
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{cleanup_temp_files, create_named_temp_files};
    use std::fs;
    use std::path::PathBuf;

    fn test_path(name: &str) -> PathBuf {
        std::env::temp_dir()
            .join(format!(
                "clip-pal-clipboard-test-{}-{}",
                std::process::id(),
                uuid::Uuid::new_v4()
            ))
            .join(name)
    }

    #[tokio::test]
    async fn rejects_display_names_with_path_components() {
        let source_path = test_path("source.txt");
        fs::create_dir_all(source_path.parent().unwrap()).unwrap();
        fs::write(&source_path, b"test").unwrap();

        let result = create_named_temp_files(
            &["../outside.txt".to_string()],
            &[source_path.to_string_lossy().to_string()],
        )
        .await;

        assert!(result.is_err());
        fs::remove_dir_all(source_path.parent().unwrap()).unwrap();
    }

    #[tokio::test]
    async fn keeps_duplicate_display_names_in_separate_paths() {
        let source_dir = test_path("sources");
        fs::create_dir_all(&source_dir).unwrap();
        let first_source = source_dir.join("first.txt");
        let second_source = source_dir.join("second.txt");
        fs::write(&first_source, b"first").unwrap();
        fs::write(&second_source, b"second").unwrap();

        let result = create_named_temp_files(
            &["same.txt".to_string(), "same.txt".to_string()],
            &[
                first_source.to_string_lossy().to_string(),
                second_source.to_string_lossy().to_string(),
            ],
        )
        .await
        .unwrap();

        assert_eq!(result.len(), 2);
        assert_ne!(result[0], result[1]);
        assert_eq!(fs::read(&result[0]).unwrap(), b"first");
        assert_eq!(fs::read(&result[1]).unwrap(), b"second");

        let operation_dir = PathBuf::from(&result[0])
            .parent()
            .and_then(|path| path.parent())
            .unwrap()
            .to_path_buf();
        cleanup_temp_files(&operation_dir).await.unwrap();
        fs::remove_dir_all(source_dir.parent().unwrap()).unwrap();
    }
}

#[cfg(test)]
mod filename_tests {
    use super::*;
    #[tokio::test]
    async fn preserves_legal_leading_and_trailing_filename_spaces() {
        let dir = std::env::temp_dir().join(format!("clippal-spaces-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let source = dir.join(" leading space.txt ");
        std::fs::write(&source, "original").unwrap();
        let paths = create_named_temp_files(&[" leading space.txt ".into()], &[source.to_string_lossy().into_owned()]).await.unwrap();
        let target = std::path::Path::new(&paths[0]);
        assert_eq!(target.file_name().unwrap(), " leading space.txt ");
        assert_eq!(std::fs::read_to_string(target).unwrap(), "original");
        cleanup_temp_files(target.parent().unwrap().parent().unwrap()).await.unwrap();
        std::fs::remove_dir_all(dir).unwrap();
    }
}

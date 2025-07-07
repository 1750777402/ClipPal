use std::path::Path;

/// 路径编码处理工具
pub struct PathUtils;

impl PathUtils {
    /// 安全地将路径转换为UTF-8字符串
    /// 
    /// # 参数
    /// * `path` - Path引用
    /// 
    /// # 返回值
    /// 返回安全的UTF-8编码的路径字符串
    pub fn to_safe_string(path: &Path) -> String {
        match path.to_string_lossy() {
            std::borrow::Cow::Borrowed(s) => s.to_string(),
            std::borrow::Cow::Owned(s) => s,
        }
    }

    /// 从字符串路径安全地转换为UTF-8字符串
    /// 
    /// # 参数
    /// * `path_str` - 路径字符串
    /// 
    /// # 返回值
    /// 返回安全的UTF-8编码的路径字符串
    pub fn str_to_safe_string(path_str: &str) -> String {
        if path_str.is_empty() {
            return String::new();
        }
        
        Self::to_safe_string(Path::new(path_str))
    }

    /// 生成文件不存在的错误消息
    /// 
    /// # 参数
    /// * `not_found_paths` - 不存在的文件路径列表
    /// 
    /// # 返回值
    /// 返回格式化的错误消息
    pub fn generate_file_not_found_error(not_found_paths: &[String]) -> String {
        if not_found_paths.is_empty() {
            return "文件不存在，无法复制".to_string();
        }

        // 确保所有路径都是有效的UTF-8编码
        let safe_paths: Vec<String> = not_found_paths
            .iter()
            .map(|path| Self::str_to_safe_string(path))
            .collect();

        if safe_paths.len() == 1 {
            format!("文件不存在，无法复制:\n{}", safe_paths[0])
        } else {
            format!(
                "以下 {} 个文件不存在，无法复制:\n{}",
                safe_paths.len(),
                safe_paths.join("\n")
            )
        }
    }
}

/// 安全地将路径转换为UTF-8字符串
/// 适用于日志记录、数据库路径等所有需要安全字符串的场景
pub fn to_safe_string(path: &Path) -> String {
    PathUtils::to_safe_string(path)
}

/// 从字符串路径安全地转换为UTF-8字符串
/// 确保路径字符串的编码正确性，用于错误消息显示
pub fn str_to_safe_string(path_str: &str) -> String {
    PathUtils::str_to_safe_string(path_str)
}

/// 生成文件不存在的错误消息
pub fn generate_file_not_found_error(not_found_paths: &[String]) -> String {
    PathUtils::generate_file_not_found_error(not_found_paths)
} 
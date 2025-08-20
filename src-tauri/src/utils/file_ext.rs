/// 文件扩展名处理工具模块
use std::path::Path;

/// 已知的复合扩展名列表（全部使用小写进行匹配）
const COMPOUND_EXTENSIONS: &[&str] = &[
    "tar.gz", "tar.bz2", "tar.xz", "tar.lz", "tar.z",
    "tar.lzma", "tar.lzo", "tar.zst",
];

/// 提取完整的文件扩展名，支持复合扩展名（如 tar.gz, tar.bz2 等）
/// 
/// # 参数
/// - `file_path`: 文件路径（可以是Path或字符串）
/// 
/// # 返回
/// - 完整的扩展名字符串，如 "pdf", "tar.gz", "txt" 等
/// - 如果没有扩展名，返回空字符串
/// 
/// # 示例
/// ```rust
/// use crate::utils::file_ext::extract_full_extension;
/// 
/// assert_eq!(extract_full_extension(Path::new("file.pdf")), "pdf");
/// assert_eq!(extract_full_extension(Path::new("backup.tar.gz")), "tar.gz");
/// assert_eq!(extract_full_extension(Path::new("/path/to/archive.tar.bz2")), "tar.bz2");
/// assert_eq!(extract_full_extension(Path::new("no_ext")), "");
/// ```
pub fn extract_full_extension(file_path: &Path) -> String {
    // 提取文件名
    let filename = file_path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("");
    
    // 转换为小写进行匹配
    let filename_lower = filename.to_lowercase();
    
    // 检查是否匹配复合扩展名
    for ext in COMPOUND_EXTENSIONS {
        if filename_lower.ends_with(ext) {
            return ext.to_string();
        }
    }
    
    // 如果不是复合扩展名，使用标准方法提取单个扩展名
    file_path
        .extension()
        .and_then(|ext| ext.to_str())
        .unwrap_or("")
        .to_string()
}

/// 从字符串路径提取完整的文件扩展名
/// 
/// # 参数
/// - `file_path_str`: 文件路径字符串
/// 
/// # 返回
/// - 完整的扩展名字符串
/// 
/// # 示例
/// ```rust
/// use crate::utils::file_ext::extract_full_extension_from_str;
/// 
/// assert_eq!(extract_full_extension_from_str("file.pdf"), "pdf");
/// assert_eq!(extract_full_extension_from_str("backup.tar.gz"), "tar.gz");
/// assert_eq!(extract_full_extension_from_str("files/archive.tar.bz2"), "tar.bz2");
/// ```
pub fn extract_full_extension_from_str(file_path_str: &str) -> String {
    extract_full_extension(Path::new(file_path_str))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn test_simple_extensions() {
        assert_eq!(extract_full_extension(Path::new("file.txt")), "txt");
        assert_eq!(extract_full_extension(Path::new("document.pdf")), "pdf");
        assert_eq!(extract_full_extension(Path::new("image.jpg")), "jpg");
    }

    #[test]
    fn test_compound_extensions() {
        assert_eq!(extract_full_extension(Path::new("backup.tar.gz")), "tar.gz");
        assert_eq!(extract_full_extension(Path::new("archive.tar.bz2")), "tar.bz2");
        assert_eq!(extract_full_extension(Path::new("data.tar.xz")), "tar.xz");
        assert_eq!(extract_full_extension(Path::new("old.tar.Z")), "tar.z");
    }

    #[test]
    fn test_case_insensitive() {
        assert_eq!(extract_full_extension(Path::new("BACKUP.TAR.GZ")), "tar.gz");
        assert_eq!(extract_full_extension(Path::new("Archive.TAR.BZ2")), "tar.bz2");
        assert_eq!(extract_full_extension(Path::new("FILE.PDF")), "PDF");
    }

    #[test]
    fn test_with_paths() {
        assert_eq!(extract_full_extension(Path::new("/path/to/file.tar.gz")), "tar.gz");
        assert_eq!(extract_full_extension(Path::new("files/document.pdf")), "pdf");
        assert_eq!(extract_full_extension(Path::new("../backup.tar.bz2")), "tar.bz2");
    }

    #[test]
    fn test_no_extension() {
        assert_eq!(extract_full_extension(Path::new("README")), "");
        assert_eq!(extract_full_extension(Path::new("path/to/file")), "");
    }

    #[test]
    fn test_from_string() {
        assert_eq!(extract_full_extension_from_str("file.tar.gz"), "tar.gz");
        assert_eq!(extract_full_extension_from_str("document.pdf"), "pdf");
        assert_eq!(extract_full_extension_from_str("files/backup.tar.bz2"), "tar.bz2");
    }
}
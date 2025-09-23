use std::path::Path;
use std::fs;
use log::{debug, error, warn};

/// 处理自定义协议请求
pub fn handle_protocol_request(
    request: tauri::http::Request<Vec<u8>>,
    responder: tauri::UriSchemeResponder
) {
    tauri::async_runtime::spawn(async move {
        let uri = request.uri();
        debug!("收到资源请求: {}", uri);

        // 解析URI路径，从 clippal-asset://localhost/path 中提取 path
        let url_path = uri.path();
        debug!("URL路径: {}", url_path);

        // 移除开头的 /，得到实际的文件路径
        let file_path = if url_path.starts_with('/') {
            &url_path[1..]
        } else {
            url_path
        };

        // URL解码，处理路径中的特殊字符
        let decoded_path = match urlencoding::decode(file_path) {
            Ok(decoded) => decoded.to_string(),
            Err(e) => {
                warn!("URL解码失败: {}, 错误: {}", file_path, e);
                let response = tauri::http::Response::builder()
                    .status(400)
                    .header("Content-Type", "text/plain")
                    .body("Bad Request: Invalid URL encoding".as_bytes().to_vec())
                    .unwrap();
                responder.respond(response);
                return;
            }
        };

        debug!("解码后的文件路径: {}", decoded_path);

        // 基本安全检查 - 防止路径遍历攻击
        if decoded_path.contains("..") || decoded_path.contains("../") || decoded_path.contains("..\\") {
            warn!("检测到不安全的文件路径: {}", decoded_path);
            let response = tauri::http::Response::builder()
                .status(403)
                .header("Content-Type", "text/plain")
                .body("Forbidden: Path traversal detected".as_bytes().to_vec())
                .unwrap();
            responder.respond(response);
            return;
        }

        // 创建路径对象
        let path = Path::new(&decoded_path);

        // 检查文件是否存在
        if !path.exists() {
            warn!("文件不存在: {}", decoded_path);
            let response = tauri::http::Response::builder()
                .status(404)
                .header("Content-Type", "text/plain")
                .body("File not found".as_bytes().to_vec())
                .unwrap();
            responder.respond(response);
            return;
        }

        // 检查是否是文件（不是目录）
        if !path.is_file() {
            warn!("路径不是文件: {}", decoded_path);
            let response = tauri::http::Response::builder()
                .status(400)
                .header("Content-Type", "text/plain")
                .body("Bad Request: Not a file".as_bytes().to_vec())
                .unwrap();
            responder.respond(response);
            return;
        }

        // 检查是否是图片文件
        if !is_image_file(path) {
            warn!("不是支持的图片文件: {}", decoded_path);
            let response = tauri::http::Response::builder()
                .status(415)
                .header("Content-Type", "text/plain")
                .body("Unsupported media type".as_bytes().to_vec())
                .unwrap();
            responder.respond(response);
            return;
        }

        // 读取文件内容
        match fs::read(path) {
            Ok(content) => {
                let content_type = get_content_type(path);
                debug!("成功读取文件: {}, 大小: {} bytes, 类型: {}", decoded_path, content.len(), content_type);

                let response = tauri::http::Response::builder()
                    .status(200)
                    .header("Content-Type", content_type)
                    .header("Cache-Control", "public, max-age=31536000")
                    .header("Access-Control-Allow-Origin", "*")
                    .header("Access-Control-Allow-Methods", "GET, HEAD, OPTIONS")
                    .header("Access-Control-Allow-Headers", "*")
                    .body(content)
                    .unwrap();
                responder.respond(response);
            }
            Err(e) => {
                error!("读取文件失败: {}, 错误: {}", decoded_path, e);
                let response = tauri::http::Response::builder()
                    .status(500)
                    .header("Content-Type", "text/plain")
                    .body("Internal server error".as_bytes().to_vec())
                    .unwrap();
                responder.respond(response);
            }
        }
    });
}

/// 检查是否是支持的图片文件
fn is_image_file(path: &Path) -> bool {
    if let Some(ext) = path.extension() {
        let ext = ext.to_string_lossy().to_lowercase();
        matches!(ext.as_str(), "jpg" | "jpeg" | "png" | "gif" | "bmp" | "webp" | "svg" | "ico")
    } else {
        false
    }
}

/// 根据文件扩展名获取Content-Type
fn get_content_type(path: &Path) -> &'static str {
    if let Some(ext) = path.extension() {
        match ext.to_string_lossy().to_lowercase().as_str() {
            "jpg" | "jpeg" => "image/jpeg",
            "png" => "image/png",
            "gif" => "image/gif",
            "bmp" => "image/bmp",
            "webp" => "image/webp",
            "svg" => "image/svg+xml",
            "ico" => "image/x-icon",
            _ => "application/octet-stream",
        }
    } else {
        "application/octet-stream"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_image_file() {
        assert!(is_image_file(Path::new("test.png")));
        assert!(is_image_file(Path::new("test.JPG")));
        assert!(is_image_file(Path::new("test.webp")));
        assert!(!is_image_file(Path::new("test.txt")));
        assert!(!is_image_file(Path::new("test.exe")));
        assert!(!is_image_file(Path::new("test")));
    }

    #[test]
    fn test_get_content_type() {
        assert_eq!(get_content_type(Path::new("test.png")), "image/png");
        assert_eq!(get_content_type(Path::new("test.JPG")), "image/jpeg");
        assert_eq!(get_content_type(Path::new("test.gif")), "image/gif");
        assert_eq!(get_content_type(Path::new("test.unknown")), "application/octet-stream");
    }
}
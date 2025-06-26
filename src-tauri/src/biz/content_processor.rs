use base64::{Engine as _, engine::general_purpose};
use clipboard_listener::ClipType;
use serde_json::Value;
use std::fs;
use std::path::Path;

use crate::utils::aes_util::decrypt_content;

pub struct ContentProcessor;

impl ContentProcessor {
    /// 处理原始内容，将各种类型的 Value 转换为字符串
    pub fn process_raw_content(content: Value) -> String {
        match content {
            Value::String(s) => s,
            Value::Object(obj) => serde_json::to_string(&obj).unwrap_or_default(),
            Value::Array(arr) => serde_json::to_string(&arr).unwrap_or_default(),
            _ => String::new(),
        }
    }

    /// 处理文本内容
    pub fn process_text_content(content: Value) -> String {
        Self::process_raw_content(content)
    }

    /// 处理图片内容，返回 base64 编码的图片数据
    pub fn process_image_content(content: &str) -> Option<String> {
        let base_path = crate::utils::file_dir::get_resources_dir()?;
        let abs_path = base_path.join(content);
        Self::file_to_base64(&abs_path)
    }

    /// 处理文件内容，将文件路径字符串转换为 JSON 数组字符串
    pub fn process_file_content(content: &str) -> String {
        let restored: Vec<String> = content.split(":::").map(|s| s.to_string()).collect();
        serde_json::to_string(&restored).unwrap_or_default()
    }

    /// 将文件转换为 base64 编码
    pub fn file_to_base64(file_path: &Path) -> Option<String> {
        let bytes = fs::read(file_path).ok()?;
        let encoded = general_purpose::STANDARD.encode(&bytes);
        let ext = file_path.extension()?.to_str()?.to_lowercase();

        let mime = match ext.as_str() {
            "png" => "image/png",
            "jpg" | "jpeg" => "image/jpeg",
            "gif" => "image/gif",
            _ => "application/octet-stream",
        };

        Some(format!("data:{};base64,{}", mime, encoded))
    }

    /// 根据剪贴板类型处理内容
    pub fn process_by_clip_type(clip_type: &str, content: Value) -> String {
        match clip_type {
            t if t == ClipType::Text.to_string() => {
                decrypt_content(Self::process_text_content(content).as_str()).unwrap_or_default()
            }
            t if t == ClipType::Image.to_string() => {
                if let Some(path) = content.as_str() {
                    Self::process_image_content(path).unwrap_or_default()
                } else {
                    String::new()
                }
            }
            t if t == ClipType::File.to_string() => {
                if let Some(paths) = content.as_str() {
                    Self::process_file_content(paths)
                } else {
                    String::new()
                }
            }
            _ => String::new(),
        }
    }
}

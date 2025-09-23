use clipboard_listener::ClipType;
use serde_json::Value;

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


    /// 处理文件内容，将文件路径字符串转换为 JSON 数组字符串
    pub fn process_file_content(content: &str) -> String {
        let restored: Vec<String> = content.split(":::").map(|s| s.to_string()).collect();
        serde_json::to_string(&restored).unwrap_or_default()
    }


    /// 根据剪贴板类型处理内容
    pub fn process_by_clip_type(clip_type: &str, content: Value) -> String {
        match clip_type {
            t if t == ClipType::Text.to_string() => {
                match decrypt_content(Self::process_text_content(content).as_str()) {
                    Ok(text) => text,
                    Err(e) => {
                        log::error!("解密文本内容失败: {}", e);
                        String::new()
                    }
                }
            }
            t if t == ClipType::Image.to_string() => {
                // 图片类型直接返回文件路径，不进行base64编码
                if let Some(path) = content.as_str() {
                    path.to_string()
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

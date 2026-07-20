use clipboard_listener::ClipType;
use serde_json::Value;

use crate::utils::aes_util::decrypt_content;

/// 剪贴内容领域处理器，统一完成原始值转换、文本解密和文件列表序列化。
pub struct ContentProcessor;

impl ContentProcessor {
    /// 处理原始内容，将各种类型的 Value 转换为字符串。
    pub fn process_raw_content(content: Value) -> String {
        match content {
            Value::String(value) => value,
            Value::Object(value) => serde_json::to_string(&value).unwrap_or_default(),
            Value::Array(value) => serde_json::to_string(&value).unwrap_or_default(),
            _ => String::new(),
        }
    }

    /// 处理文本内容。
    pub fn process_text_content(content: Value) -> String {
        Self::process_raw_content(content)
    }

    /// 处理文件内容，将文件路径字符串转换为 JSON 数组字符串。
    pub fn process_file_content(content: &str) -> String {
        let restored: Vec<String> = content.split(":::").map(str::to_string).collect();
        serde_json::to_string(&restored).unwrap_or_default()
    }

    /// 根据剪贴板类型处理内容。
    pub fn process_by_clip_type(clip_type: &str, content: Value) -> String {
        // 内容类型决定解密、路径透传或文件列表序列化策略。
        match clip_type {
            value if value == ClipType::Text.to_string() => {
                match decrypt_content(Self::process_text_content(content).as_str()) {
                    Ok(text) => text,
                    Err(error) => {
                        log::error!("解密文本内容失败: {}", error);
                        String::new()
                    }
                }
            }
            value if value == ClipType::Image.to_string() => {
                // 图片类型直接返回文件路径，不进行 base64 编码。
                content.as_str().unwrap_or_default().to_string()
            }
            value if value == ClipType::File.to_string() => content
                .as_str()
                .map(Self::process_file_content)
                .unwrap_or_default(),
            _ => String::new(),
        }
    }
}

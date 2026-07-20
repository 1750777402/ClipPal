use clipboard_listener::ClipType;
use std::{collections::HashMap, fs, path::Path};

use crate::{
    app_context::AppContext,
    domain::clip::ContentProcessor,
    dto::clip_dto::{
        ClipRecordLiteDTO, FileInfo, FullContentResponse, ImageInfo, ImagePathInfo, QueryParam,
    },
    errors::{AppError, AppResult},
    infra::{repositories::ClipRecordRepository, search::SearchEngine},
};

/// 剪贴记录应用服务，负责查询、内容转换、资源定位、置顶和删除流程。
pub struct ClipRecordService<'a> {
    context: &'a AppContext,
    repository: &'a dyn ClipRecordRepository,
    search_engine: &'a dyn SearchEngine,
}

impl<'a> ClipRecordService<'a> {
    /// 从应用上下文注入剪贴记录仓储和搜索引擎。
    pub fn from_context(context: &'a AppContext) -> Self {
        Self {
            context,
            repository: context.repositories().clip_records(),
            search_engine: context.search_engine(),
        }
    }
    /// 按分页和搜索条件查询记录，并转换为适合列表展示的轻量 DTO。
    pub async fn query_clip_records(&self, param: QueryParam) -> AppResult<Vec<ClipRecordLiteDTO>> {
        let offset = (param.page - 1) * param.size;
        // 搜索时先由搜索引擎筛选记录 ID，再由仓储加载完整记录并保持统一排序。
        let query_result = match param.search.as_deref().filter(|search| !search.is_empty()) {
            Some(search) => {
                let ids = self.search_engine.search(search).await;
                self.repository.list_by_ids(&ids, param.size, offset).await
            }
            None => self.repository.list(param.size, offset).await,
        };

        let records = match query_result {
            Ok(records) => records,
            Err(error) => {
                log::error!("查询剪贴记录失败: {:?}", error);
                return Err(error);
            }
        };

        // 不同记录类型需要不同的展示内容和资源信息转换规则。
        Ok(records
            .into_iter()
            .map(|record| {
                // 文件内容转换为 JSON 名称列表，同时补充真实路径对应的文件信息。
                if record.r#type == ClipType::File.to_string() {
                    let content_names = record.content.as_str().unwrap_or_default().to_string();
                    let local_paths = record
                        .local_file_path
                        .as_deref()
                        .unwrap_or_default()
                        .to_string();
                    let content = ContentProcessor::process_by_clip_type(
                        &record.r#type,
                        record.content.clone(),
                    );

                    ClipRecordLiteDTO {
                        id: record.id,
                        r#type: record.r#type,
                        content,
                        os_type: record.os_type,
                        created: record.created,
                        pinned_flag: record.pinned_flag,
                        file_info: get_file_info_with_paths(content_names, local_paths),
                        sync_flag: record.sync_flag,
                        cloud_source: record.cloud_source,
                        content_truncated: false,
                        original_content_length: None,
                        has_image: false,
                    }
                // 图片列表只返回资源路径，具体文件加载由图片路径接口负责。
                } else if record.r#type == ClipType::Image.to_string() {
                    let image_path = record.content.as_str().unwrap_or_default().to_string();

                    ClipRecordLiteDTO {
                        id: record.id,
                        r#type: record.r#type,
                        content: image_path,
                        os_type: record.os_type,
                        created: record.created,
                        pinned_flag: record.pinned_flag,
                        file_info: vec![],
                        sync_flag: record.sync_flag,
                        cloud_source: record.cloud_source,
                        content_truncated: false,
                        original_content_length: None,
                        has_image: true,
                    }
                // 文本内容在领域处理器中解密，并限制列表预览长度。
                } else {
                    let content = ContentProcessor::process_by_clip_type(
                        &record.r#type,
                        record.content.clone(),
                    );
                    let (content, content_truncated, original_content_length) =
                        truncate_large_text(&content);

                    ClipRecordLiteDTO {
                        id: record.id,
                        r#type: record.r#type,
                        content,
                        os_type: record.os_type,
                        created: record.created,
                        pinned_flag: record.pinned_flag,
                        file_info: vec![],
                        sync_flag: record.sync_flag,
                        cloud_source: record.cloud_source,
                        content_truncated,
                        original_content_length,
                        has_image: false,
                    }
                }
            })
            .collect())
    }

    /// 定位图片记录对应的本地文件，优先使用缓存绝对路径，再尝试资源目录相对路径。
    pub async fn get_image_path(&self, record_id: &str) -> AppResult<ImagePathInfo> {
        let record = self
            .repository
            .find_by_id(record_id)
            .await
            .map_err(|error| AppError::General(format!("数据库查询失败: {}", error)))?
            .ok_or_else(|| AppError::General("记录不存在".to_string()))?;

        if record.r#type != ClipType::Image.to_string() {
            return Err(AppError::General("记录不是图片类型".to_string()));
        }

        // 缓存绝对路径存在时直接返回，避免重复拼接资源目录。
        if let Some(cache_file_path) = &record.local_file_path {
            if Path::new(cache_file_path).exists() {
                return Ok(ImagePathInfo {
                    id: record.id,
                    file_path: cache_file_path.clone(),
                    protocol_url: String::new(),
                });
            }
        }

        // 旧记录可能只保存资源目录中的相对文件名，因此保留第二种定位方式。
        if let Some(filename) = record.content.as_str() {
            if let Some(resources_dir) = crate::utils::file_dir::get_resources_dir() {
                let image_path = resources_dir.join(filename);
                if image_path.exists() {
                    return Ok(ImagePathInfo {
                        id: record.id,
                        file_path: image_path.to_string_lossy().to_string(),
                        protocol_url: String::new(),
                    });
                }
            }
        }

        Err(AppError::General("图片文件不存在".to_string()))
    }

    /// 批量读取图片文件大小；单条记录失败不会中断其他记录处理。
    pub async fn get_image_info_batch(
        &self,
        record_ids: Vec<String>,
    ) -> AppResult<HashMap<String, ImageInfo>> {
        let mut result = HashMap::new();

        // 按 ID 独立容错，最终只返回成功解析的图片信息。
        for id in record_ids {
            match self.repository.find_by_id(&id).await {
                Ok(Some(record)) if record.r#type == ClipType::Image.to_string() => {
                    let image_path = record.content.as_str().unwrap_or_default();
                    if let Some(info) = get_image_info(image_path) {
                        result.insert(id, info);
                    }
                }
                Ok(_) => {}
                Err(error) => {
                    log::warn!("获取图片信息失败，记录 ID: {}, 错误: {}", id, error);
                }
            }
        }

        Ok(result)
    }

    /// 获取文本记录的完整解密内容，用于替换列表中的截断预览。
    pub async fn get_full_text_content(&self, record_id: String) -> AppResult<FullContentResponse> {
        let record = self
            .repository
            .find_by_id(&record_id)
            .await
            .map_err(|error| AppError::General(format!("查询记录失败: {}", error)))?
            .ok_or_else(|| AppError::General("记录不存在".to_string()))?;

        if record.r#type != ClipType::Text.to_string() {
            return Err(AppError::General("记录类型不是文本".to_string()));
        }

        let content = ContentProcessor::process_by_clip_type(&record.r#type, record.content);

        Ok(FullContentResponse {
            id: record_id,
            content_length: content.len(),
            content,
        })
    }

    /// 更新记录置顶标记；仓储负责互斥置顶和版本号更新事务。
    pub async fn set_pinned(&self, record_id: &str, pinned_flag: i32) -> AppResult<String> {
        let _ = self.repository.set_pinned(record_id, pinned_flag).await;
        Ok(String::new())
    }

    /// 逻辑删除记录，并同步处理云同步队列和搜索索引。
    pub async fn delete_record(&self, record_id: String) -> AppResult<String> {
        // 先读取记录快照，删除成功后云同步队列需要携带完整记录信息。
        let record = match self.repository.find_by_id(&record_id).await {
            Ok(record) => record,
            Err(_) => return Err(AppError::General("未找到该记录".to_string())),
        };

        let Some(record) = record else {
            return Ok(String::new());
        };

        let ids = vec![record_id];
        // 只有数据库逻辑删除成功后，才执行队列和索引等后续副作用。
        if self.repository.mark_deleted(&ids).await.is_ok() {
            let cloud_sync_enabled = self
                .context
                .with_settings(|settings| settings.cloud_sync == 1)
                .unwrap_or(false);

            // 开启云同步时将删除事件放入队列，由后台任务同步到服务端。
            if cloud_sync_enabled {
                let queue = self.context.clip_record_queue();
                if !queue.is_full() {
                    let _ = queue.send_delete(record).await;
                }
            }

            // 搜索索引与数据库保持一致；索引失败记录日志但不回滚数据库删除。
            if let Err(error) = self.search_engine.remove(&ids).await {
                log::error!("从搜索索引删除记录失败: {}", error);
            }
        }

        Ok(String::new())
    }
}

/// 将文件显示名称和真实路径按索引配对，生成前端展示所需的文件元数据。
pub fn get_file_info_with_paths(content_names: String, local_paths: String) -> Vec<FileInfo> {
    let display_names = content_names.split(":::").collect::<Vec<&str>>();
    let actual_paths = local_paths.split(":::").collect::<Vec<&str>>();
    let min_len = display_names.len().min(actual_paths.len());

    (0..min_len)
        // 文件不存在和元数据读取失败使用不同负值，保留前端现有状态判断协议。
        .filter_map(|index| {
            let display_name = display_names[index].trim();
            let actual_path = actual_paths[index].trim();

            if display_name.is_empty() || actual_path.is_empty() {
                return None;
            }

            let path = Path::new(actual_path);
            let file_type = Path::new(display_name)
                .extension()
                .and_then(|extension| extension.to_str())
                .unwrap_or("unknown")
                .to_lowercase();

            if !path.exists() {
                return Some(FileInfo {
                    path: display_name.to_string(),
                    size: -1,
                    r#type: file_type,
                });
            }

            let metadata = match fs::metadata(path) {
                Ok(metadata) => metadata,
                Err(_) => {
                    return Some(FileInfo {
                        path: display_name.to_string(),
                        size: -2,
                        r#type: file_type,
                    });
                }
            };

            let size = if metadata.len() > i32::MAX as u64 {
                i32::MAX
            } else {
                metadata.len() as i32
            };

            Some(FileInfo {
                path: display_name.to_string(),
                size,
                r#type: file_type,
            })
        })
        .collect()
}

/// 读取资源目录中图片文件的基础元数据；文件不可用时返回 `None`。
pub fn get_image_info(relative_path: &str) -> Option<ImageInfo> {
    if relative_path.is_empty() {
        return None;
    }

    let resources_dir = crate::utils::file_dir::get_resources_dir()?;
    let image_path = resources_dir.join(relative_path);

    if !image_path.exists() {
        return None;
    }

    let metadata = fs::metadata(image_path).ok()?;

    Some(ImageInfo {
        path: relative_path.to_string(),
        size: metadata.len(),
        width: None,
        height: None,
    })
}

/// 将大文本截断为 UTF-8 安全的列表预览，并保留原始字节长度。
fn truncate_large_text(content: &str) -> (String, bool, Option<usize>) {
    const MAX_PREVIEW_SIZE: usize = 8 * 1024;

    if content.len() <= MAX_PREVIEW_SIZE {
        return (content.to_string(), false, None);
    }

    let mut end_pos = MAX_PREVIEW_SIZE;
    // 截断位置必须落在字符边界，避免切断多字节字符导致 panic。
    while end_pos > 0 && !content.is_char_boundary(end_pos) {
        end_pos -= 1;
    }

    if end_pos == 0 {
        end_pos = content
            .char_indices()
            .nth(1000)
            .map(|(index, _)| index)
            .unwrap_or(content.len().min(4096));
    }

    (content[..end_pos].to_string(), true, Some(content.len()))
}

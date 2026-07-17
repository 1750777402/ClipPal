use clipboard_listener::ClipType;
use std::{collections::HashMap, fs, path::Path};

use crate::{
    app_context::AppContext,
    biz::{
        content_processor::ContentProcessor,
        content_search::{remove_ids_from_index, search_ids_by_content},
    },
    dto::clip_dto::{
        ClipRecordLiteDTO, FileInfo, FullContentResponse, ImageInfo, ImagePathInfo, QueryParam,
    },
    errors::{AppError, AppResult},
    infra::repositories::{ClipRecordRepository, SqliteClipRecordRepository},
};

pub struct ClipRecordService<'a, R> {
    context: &'a AppContext,
    repository: R,
}

impl<'a> ClipRecordService<'a, SqliteClipRecordRepository<'a>> {
    pub fn from_context(context: &'a AppContext) -> Self {
        Self {
            context,
            repository: SqliteClipRecordRepository::new(context.db()),
        }
    }
}

impl<R> ClipRecordService<'_, R>
where
    R: ClipRecordRepository,
{
    pub async fn query_clip_records(&self, param: QueryParam) -> AppResult<Vec<ClipRecordLiteDTO>> {
        let offset = (param.page - 1) * param.size;
        let query_result = match param.search.as_deref().filter(|search| !search.is_empty()) {
            Some(search) => {
                let ids = search_ids_by_content(search).await;
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

        Ok(records
            .into_iter()
            .map(|record| {
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

        if let Some(cache_file_path) = &record.local_file_path {
            if Path::new(cache_file_path).exists() {
                return Ok(ImagePathInfo {
                    id: record.id,
                    file_path: cache_file_path.clone(),
                    protocol_url: String::new(),
                });
            }
        }

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

    pub async fn get_image_info_batch(
        &self,
        record_ids: Vec<String>,
    ) -> AppResult<HashMap<String, ImageInfo>> {
        let mut result = HashMap::new();

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

    pub async fn set_pinned(&self, record_id: &str, pinned_flag: i32) -> AppResult<String> {
        let _ = self.repository.set_pinned(record_id, pinned_flag).await;
        Ok(String::new())
    }

    pub async fn delete_record(&self, record_id: String) -> AppResult<String> {
        let record = match self.repository.find_by_id(&record_id).await {
            Ok(record) => record,
            Err(_) => return Err(AppError::General("未找到该记录".to_string())),
        };

        let Some(record) = record else {
            return Ok(String::new());
        };

        let ids = vec![record_id];
        if self.repository.mark_deleted(&ids).await.is_ok() {
            let cloud_sync_enabled = self
                .context
                .with_settings(|settings| settings.cloud_sync == 1)
                .unwrap_or(false);

            if cloud_sync_enabled {
                let queue = self.context.clip_record_queue();
                if !queue.is_full() {
                    let _ = queue.send_delete(record).await;
                }
            }

            tokio::spawn(async move {
                if let Err(error) = remove_ids_from_index(&ids).await {
                    log::error!("从搜索索引删除记录失败: {}", error);
                }
            });
        }

        Ok(String::new())
    }
}

pub fn get_file_info_with_paths(content_names: String, local_paths: String) -> Vec<FileInfo> {
    let display_names = content_names.split(":::").collect::<Vec<&str>>();
    let actual_paths = local_paths.split(":::").collect::<Vec<&str>>();
    let min_len = display_names.len().min(actual_paths.len());

    (0..min_len)
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

fn truncate_large_text(content: &str) -> (String, bool, Option<usize>) {
    const MAX_PREVIEW_SIZE: usize = 8 * 1024;

    if content.len() <= MAX_PREVIEW_SIZE {
        return (content.to_string(), false, None);
    }

    let mut end_pos = MAX_PREVIEW_SIZE;
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

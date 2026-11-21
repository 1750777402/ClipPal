use clip_core::domain::clip::{TEXT_TYPE, IMAGE_TYPE, FILE_TYPE, ClipRecord};
use std::{collections::HashMap, fs, path::Path};

use crate::{LocalContext, content::ContentProcessor, dto::{ClipRecordLiteDTO, FileInfo, FullContentResponse, ImageInfo, ImagePathInfo, QueryParam}};
use clip_core::{errors::{AppError, AppResult}, repositories::ClipRecordRepository, search::SearchEngine};

/// 剪贴记录应用服务，负责查询、内容转换、资源定位、置顶和删除流程。
pub struct ClipRecordService<'a> {
    context: &'a LocalContext,
    repository: &'a dyn ClipRecordRepository,
    search_engine: &'a dyn SearchEngine,
}

impl<'a> ClipRecordService<'a> {
    /// 从应用上下文注入剪贴记录仓储和搜索引擎。
    pub fn new(context: &'a LocalContext) -> Self {
        Self {
            context,
            repository: context.repositories().clip_records(),
            search_engine: context.search_engine(),
        }
    }
    /// 按分页和搜索条件查询记录，并转换为适合列表展示的轻量 DTO。
    pub async fn query_clip_records(&self, param: QueryParam) -> AppResult<Vec<ClipRecordLiteDTO>> {
        if param.page < 1 || param.size < 1 {
            return Err(AppError::General("页码和每页条数必须大于零".into()));
        }
        let offset = (param.page - 1).checked_mul(param.size)
            .ok_or_else(|| AppError::General("分页范围超出限制".into()))?;
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
                if record.r#type == FILE_TYPE.to_owned() {
                    let content_names = record.content.as_str().unwrap_or_default().to_string();
                    let local_paths = record
                        .local_file_path
                        .as_deref()
                        .unwrap_or_default()
                        .to_string();
                    let content = ContentProcessor::process_by_clip_type(
                        &record.r#type,
                        record.content.clone(),
                        self.context.decoder(),
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
                } else if record.r#type == IMAGE_TYPE.to_owned() {
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
                        self.context.decoder(),
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
            ?
            .ok_or_else(|| AppError::General("记录不存在".to_string()))?;

        if record.r#type != IMAGE_TYPE.to_owned() {
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
            if let Some(resources_dir) = self.context.resources_dir() {
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
                Ok(Some(record)) if record.r#type == IMAGE_TYPE.to_owned() => {
                    let image_path = record.content.as_str().unwrap_or_default();
                    if let Some(info) = get_image_info(self.context.resources_dir(), image_path) {
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
            ?
            .ok_or_else(|| AppError::General("记录不存在".to_string()))?;

        if record.r#type != TEXT_TYPE.to_owned() {
            return Err(AppError::General("记录类型不是文本".to_string()));
        }

        let content = ContentProcessor::process_by_clip_type(&record.r#type, record.content, self.context.decoder());

        Ok(FullContentResponse {
            id: record_id,
            content_length: content.len(),
            content,
        })
    }

    /// 更新记录置顶标记；仓储负责互斥置顶和版本号更新事务。
    pub async fn set_pinned(&self, record_id: &str, pinned_flag: i32) -> AppResult<String> {
        let _records = self.context.capture_lock.lock().await;
        if !matches!(pinned_flag, 0 | 1) {
            return Err(AppError::General("置顶状态无效".into()));
        }
        if self.repository.find_by_id(record_id).await?.filter(|record| !record.is_deleted()).is_none() {
            return Err(AppError::General("记录不存在".into()));
        }
        self.repository.set_pinned(record_id, pinned_flag).await?;
        Ok(String::new())
    }

    /// 返回成功删除的快照，宿主可据此执行可选的扩展副作用。
    pub async fn delete_record(&self, record_id: String, retain_tombstone: bool) -> AppResult<Option<ClipRecord>> {
        let _records = self.context.capture_lock.lock().await;
        let Some(record) = self.repository.find_by_id(&record_id).await? else { return Ok(None); };
        let ids = vec![record_id];
        if retain_tombstone { self.repository.mark_deleted(&ids).await?; }
        else { self.repository.delete_permanently(&ids).await?; }
        if let Err(error) = self.search_engine.remove(&ids).await {
            log::error!("从搜索索引删除记录失败: {}", error);
        }
        if !retain_tombstone { crate::cleanup::delete_managed_resources(self.context, &record).await; }
        Ok(Some(record))
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
            let display_name = display_names[index];
            let actual_path = actual_paths[index];

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
pub fn get_image_info(resources_dir: Option<std::path::PathBuf>, relative_path: &str) -> Option<ImageInfo> {
    if relative_path.is_empty() {
        return None;
    }

    let resources_dir = resources_dir?;
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

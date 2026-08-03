#![allow(dead_code)]

pub use crate::domain::clip::ClipRecord;
use crate::errors::{AppError, AppResult};
use rbatis::RBatis;
use rbs::to_value;

impl ClipRecord {
    /// 批量按云端同步时间合并记录，并分别统计成功和失败数量。
    ///
    /// 记录会先按 `sync_time` 从旧到新排序，再逐条使用独立事务写入。
    /// 单条记录失败会进行有限重试，最终失败不会中断后续记录处理。
    pub async fn insert_batch_merge_by_sync_time(
        rb: &RBatis,
        mut records: Vec<ClipRecord>,
    ) -> AppResult<(usize, usize)> {
        if records.is_empty() {
            return Ok((0, 0));
        }

        // 按同步时间从旧到新处理，保证每次插入时已有记录顺序稳定。
        records.sort_by(|a, b| {
            let a_time = a.sync_time.unwrap_or(0);
            let b_time = b.sync_time.unwrap_or(0);
            a_time.cmp(&b_time)
        });

        log::debug!("开始批量时间合并，待合并记录数量: {}", records.len());

        let mut inserted_count = 0;
        let mut failed_count = 0;
        const MAX_RETRIES: usize = 3;

        // 每条记录使用独立事务，避免一条异常导致整批云端记录回滚。
        for (index, mut record) in records.into_iter().enumerate() {
            let cloud_sync_time = record.sync_time.unwrap_or(0);
            let record_id = record.id.clone();
            let mut retry_count = 0;
            let mut record_success = false;

            // 事务提交或并发排序更新失败时进行有限次数重试。
            while retry_count < MAX_RETRIES && !record_success {
                let tx = rb.acquire_begin().await?;

                match Self::insert_single_record_with_merge(&tx, &mut record, cloud_sync_time).await
                {
                    Ok(()) => match tx.commit().await {
                        Ok(()) => {
                            inserted_count += 1;
                            record_success = true;

                            log::debug!(
                                "成功插入记录 {}/{}: id={}, sort={}, sync_time={:?}",
                                index + 1,
                                inserted_count + failed_count,
                                record.id,
                                record.sort,
                                record.sync_time
                            );
                        }
                        Err(e) => {
                            log::warn!(
                                "记录 {} 提交事务失败，重试 {}/{}: {}",
                                record_id,
                                retry_count + 1,
                                MAX_RETRIES,
                                e
                            );
                            retry_count += 1;

                            if retry_count < MAX_RETRIES {
                                tokio::time::sleep(tokio::time::Duration::from_millis(
                                    10 * retry_count as u64,
                                ))
                                .await;
                            }
                        }
                    },
                    Err(e) => {
                        let _ = tx.rollback().await;
                        retry_count += 1;

                        log::warn!(
                            "记录 {}, md5:{},type:{} 插入失败，重试 {}/{}: {}",
                            record_id,
                            record.md5_str,
                            record.r#type,
                            retry_count,
                            MAX_RETRIES,
                            e
                        );

                        if retry_count < MAX_RETRIES {
                            tokio::time::sleep(tokio::time::Duration::from_millis(
                                10 * retry_count as u64,
                            ))
                            .await;
                        }
                    }
                }
            }

            if !record_success {
                failed_count += 1;
                log::error!(
                    "记录 {} (sync_time: {}) 最终插入失败，跳过继续处理后续记录",
                    record_id,
                    cloud_sync_time
                );
            }
        }

        if failed_count > 0 {
            log::warn!(
                "批量时间合并完成，成功: {}条，失败: {}条",
                inserted_count,
                failed_count
            );
        } else {
            log::info!("批量时间合并完成，全部成功插入{}条记录", inserted_count);
        }

        Ok((inserted_count, failed_count))
    }

    /// 在现有事务中计算单条云端记录的排序位置并完成插入。
    async fn insert_single_record_with_merge(
        tx: &rbatis::executor::RBatisTxExecutor,
        record: &mut ClipRecord,
        cloud_sync_time: u64,
    ) -> AppResult<()> {
        #[derive(serde::Deserialize)]
        struct SortRecord {
            sort: i32,
            sync_time: Option<u64>,
        }

        // 查找第一条同步时间不早于云端记录的本地数据，作为插入位置。
        let target_records: Vec<SortRecord> = tx
            .query_decode(
                "SELECT sort, sync_time FROM clip_record WHERE del_flag = 0 AND (sync_time >= ? OR sync_time IS NULL) ORDER BY sync_time ASC, sort ASC LIMIT 1",
                vec![to_value!(cloud_sync_time)],
            )
            .await
            .map_err(AppError::Database)?;

        if let Some(target_record) = target_records.first() {
            let target_sort = target_record.sort;

            log::debug!(
                "找到插入位置，目标sort: {}, 云端sync_time: {}, 目标sync_time: {:?}",
                target_sort,
                cloud_sync_time,
                target_record.sync_time
            );

            // 为中间位置的新记录腾出排序值。
            tx.exec(
                "UPDATE clip_record SET sort = sort + 1 WHERE sort >= ? AND del_flag = 0",
                vec![to_value!(target_sort)],
            )
            .await
            .map_err(AppError::Database)?;

            record.sort = target_sort;
        } else {
            // 没有更晚的本地记录时，将云端记录放到当前列表最前面。
            let max_sort_records: Vec<SortRecord> = tx
                .query_decode(
                    "SELECT sort FROM clip_record WHERE del_flag = 0 ORDER BY sort DESC LIMIT 1",
                    vec![],
                )
                .await
                .map_err(AppError::Database)?;

            let max_sort = max_sort_records.first().map(|item| item.sort).unwrap_or(-1);
            record.sort = max_sort + 1;

            log::debug!(
                "插入到最前面, 新sort: {}, 云端sync_time: {}",
                record.sort,
                cloud_sync_time
            );
        }

        ClipRecord::insert(tx, record)
            .await
            .map_err(AppError::Database)?;

        Ok(())
    }
}

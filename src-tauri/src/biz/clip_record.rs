#![allow(dead_code)]

use crate::errors::{AppError, AppResult};
use rbatis::{Error, RBatis, crud, impl_select};
use rbs::to_value;
use serde::{Deserialize, Serialize};
use serde_json::Value;

pub static NOT_SYNCHRONIZED: i32 = 0; // 未同步
pub static SYNCHRONIZING: i32 = 1; // 同步中
pub static SYNCHRONIZED: i32 = 2; // 已同步
pub static SKIP_SYNC: i32 = 3; // 不支持同步（多文件、超大文件等）

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct ClipRecord {
    pub id: String,
    // 类型
    pub r#type: String,
    // 内容
    pub content: Value,
    // 内容md5值
    pub md5_str: String,
    // 本地文件地址
    pub local_file_path: Option<String>,
    // 时间戳
    pub created: u64,
    // os类型
    pub os_type: String,
    // 排序字段
    pub sort: i32,
    // 是否置顶
    pub pinned_flag: i32,
    // 是否已同步云端  0:未同步，1:同步中，2:已同步
    pub sync_flag: Option<i32>,
    // 同步时间
    pub sync_time: Option<u64>,
    // 设备标识
    pub device_id: Option<String>,
    // 云同步版本号（预留字段）
    pub version: Option<i32>,
    // 是否逻辑删除 0:未删除 1:已删除
    pub del_flag: Option<i32>,
    // 是否是云端同步下来的数据
    pub cloud_source: Option<i32>,
    // 跳过云同步的原因类型  跳过后是否可以再次尝试同步
    pub skip_type: Option<i32>,
}

crud!(ClipRecord {}, "clip_record");
impl_select!(ClipRecord{select_by_id(id: &str) =>"`where id = #{id}`"});
impl_select!(ClipRecord{select_by_pinned_flag(pinned_flag: i32) =>"`where pinned_flag = #{pinned_flag}`"});
impl_select!(ClipRecord{select_order_by() =>"`order by sort desc, created desc`"});
impl_select!(ClipRecord{select_where_order_by_limit(content: &str, limit:i32, offset:i32) =>"` where content like #{content} order by pinned_flag desc, sort desc, created desc limit #{limit} offset #{offset}`"});
//  根据limit和offset 查询   获取limit条数据(-1表示全部)   跳过前offset条数据
impl_select!(ClipRecord{select_order_by_limit(limit:i32, offset:i32) =>"` where del_flag = 0 order by pinned_flag desc, sort desc, created desc limit #{limit} offset #{offset}`"});
// 根据type和content 查看是否有重复的    有的话取出一个
impl_select!(ClipRecord{check_by_type_and_md5(content_type:&str, md5_str:&str) =>"`where type = #{content_type} and md5_str = #{md5_str} limit 1`"});
impl_select!(ClipRecord{check_by_type_and_md5_active(content_type:&str, md5_str:&str) =>"`where type = #{content_type} and md5_str = #{md5_str} and (del_flag is null or del_flag = 0) limit 1`"});
// 取出最大的sort数据
impl_select!(ClipRecord{select_max_sort() =>"`order by sort desc, created desc limit 1`"});
// 根据sync_flag查询记录
impl_select!(ClipRecord{select_by_sync_flag(sync_flag: i32) =>"`where sync_flag = #{sync_flag} and content IS NOT NULL order by created desc`"});
// 根据sync_flag查询记录
impl_select!(ClipRecord{select_by_sync_flag_limit(sync_flag: i32, cloud_source:i32, limit: i32) =>"`where sync_flag = #{sync_flag} and cloud_source = #{cloud_source} order by created desc limit #{limit}`"});
// 根据created时间戳查询下一条记录
impl_select!(ClipRecord{select_order_by_created(created: u64) =>"`where created >= #{created} order by created desc limit 1`"});
// 查询已经逻辑删除并且已同步的数据
impl_select!(ClipRecord{select_invalid() =>"`where sync_flag = 2 and del_flag = 1`"});

impl ClipRecord {
    pub async fn update_content(rb: &RBatis, id: &str, content: &str) -> AppResult<()> {
        let sql = "UPDATE clip_record SET content = ? WHERE id = ?";
        let tx = rb.acquire_begin().await?;
        let _ = tx.exec(sql, vec![to_value!(content), to_value!(id)]).await;
        tx.commit()
            .await
            .map_err(|e| AppError::Database(rbatis::Error::from(e)))
    }

    pub async fn get_next_sort(rb: &RBatis) -> i32 {
        ClipRecord::select_max_sort(rb)
            .await
            .ok()
            .and_then(|records| records.get(0).map(|r| r.sort + 1))
            .unwrap_or(0)
    }

    pub async fn update_sort(rb: &RBatis, id: &str, sort: i32) -> AppResult<()> {
        // 更新排序的时候，同时也要给版本号自增1
        let sql = "UPDATE clip_record SET sort = ?, version = IFNULL(version, 0) + 1 WHERE id = ?";
        let tx = rb.acquire_begin().await?;
        let _ = tx.exec(sql, vec![to_value!(sort), to_value!(id)]).await;
        tx.commit()
            .await
            .map_err(|e| AppError::Database(rbatis::Error::from(e)))
    }

    pub async fn update_pinned(rb: &RBatis, id: &str, pinned_flag: i32) -> AppResult<()> {
        let sql =
            "UPDATE clip_record SET pinned_flag = ?, version = IFNULL(version, 0) + 1 WHERE id = ?";
        let tx = rb.acquire_begin().await?;
        if pinned_flag == 1 {
            // 置顶某一条的时候  先把其他的置顶都取消
            let sql1 = "UPDATE clip_record SET pinned_flag = 0 WHERE pinned_flag = 1";
            let _ = tx.exec(sql1, vec![]).await;
        }
        let _ = tx
            .exec(sql, vec![to_value!(pinned_flag), to_value!(id)])
            .await;
        tx.commit()
            .await
            .map_err(|e| AppError::Database(rbatis::Error::from(e)))
    }

    pub async fn update_sync_flag(
        rb: &RBatis,
        ids: &Vec<String>,
        sync_flag: i32,
        sync_time: u64,
    ) -> AppResult<()> {
        let sql = format!(
            "UPDATE clip_record SET sync_flag = ?, sync_time = ? WHERE id in ({})",
            ids.iter().map(|_| "?").collect::<Vec<_>>().join(",")
        );
        let mut args = vec![to_value!(sync_flag), to_value!(sync_time)];
        for id in ids {
            args.push(to_value!(id));
        }
        let tx = rb.acquire_begin().await?;
        tx.exec(&sql, args).await?;
        tx.commit()
            .await
            .map_err(|e| AppError::Database(rbatis::Error::from(e)))
    }

    /// 更新local_file_path字段
    pub async fn update_local_file_path(rb: &RBatis, id: &str, local_path: &str) -> AppResult<()> {
        let sql = "UPDATE clip_record SET local_file_path = ? WHERE id = ?";
        let tx = rb.acquire_begin().await?;
        let _ = tx
            .exec(sql, vec![to_value!(local_path), to_value!(id)])
            .await;
        tx.commit()
            .await
            .map_err(|e| AppError::Database(rbatis::Error::from(e)))
    }

    /// 更新云文件下载后的记录状态
    pub async fn update_after_cloud_download(
        rb: &RBatis,
        id: &str,
        filename: &str,
        absolute_path: &str,
    ) -> AppResult<()> {
        let sql =
            "UPDATE clip_record SET content = ?, local_file_path = ?, sync_flag = ? WHERE id = ?";

        let tx = rb.acquire_begin().await?;
        tx.exec(
            sql,
            vec![
                to_value!(filename),
                to_value!(absolute_path),
                to_value!(SYNCHRONIZED),
                to_value!(id),
            ],
        )
        .await?;
        tx.commit()
            .await
            .map_err(|e| AppError::Database(rbatis::Error::from(e)))
    }

    /// 获取已逻辑删除且已同步的数据数量
    pub async fn count_invalid(rb: &RBatis) -> i64 {
        let count_res: Result<i64, rbs::Error> = rb
            .query_decode(
                "SELECT COUNT(*) FROM clip_record where del_flag = 1 and sync_flag = 2",
                vec![],
            )
            .await;
        match count_res {
            Ok(count) => return count,
            Err(_) => return 0,
        }
    }

    pub async fn count_effective(rb: &RBatis) -> i64 {
        let count_res: Result<i64, rbs::Error> = rb
            .query_decode(
                "SELECT COUNT(*) FROM clip_record where del_flag = 0",
                vec![],
            )
            .await;
        match count_res {
            Ok(count) => return count,
            Err(_) => return 0,
        }
    }

    /// 逻辑删除 并标记为待同步状态
    pub async fn update_del_by_ids(rb: &RBatis, ids: &Vec<String>) -> AppResult<()> {
        let sql = format!(
            "UPDATE clip_record SET del_flag = 1, sync_flag = 0 WHERE id IN ({})",
            ids.iter().map(|_| "?").collect::<Vec<_>>().join(",")
        );
        let tx = rb.acquire_begin().await?;
        // 转换ids为Vec<Value>
        let params = ids.into_iter().map(|id| to_value!(id)).collect::<Vec<_>>();
        let _ = tx.exec(&sql, params).await?;
        tx.commit()
            .await
            .map_err(|e| AppError::Database(rbatis::Error::from(e)))
    }

    /// 更新已删除记录的所有字段（相当于创建新记录但保持原ID）
    pub async fn update_deleted_record_as_new(
        rb: &RBatis,
        id: &str,
        new_record: &ClipRecord,
    ) -> AppResult<()> {
        let sql = "UPDATE clip_record SET type = ?, content = ?, md5_str = ?, local_file_path = ?, created = ?, os_type = ?, sort = ?, pinned_flag = ?, sync_flag = ?, sync_time = ?, device_id = ?, version = ?, del_flag = ?, cloud_source = ? WHERE id = ?";
        let tx = rb.acquire_begin().await?;
        let params = vec![
            to_value!(&new_record.r#type),
            to_value!(&new_record.content),
            to_value!(&new_record.md5_str),
            to_value!(&new_record.local_file_path),
            to_value!(new_record.created),
            to_value!(&new_record.os_type),
            to_value!(new_record.sort),
            to_value!(new_record.pinned_flag),
            to_value!(&new_record.sync_flag),
            to_value!(&new_record.sync_time),
            to_value!(&new_record.device_id),
            to_value!(&new_record.version),
            to_value!(&new_record.del_flag),
            to_value!(&new_record.cloud_source),
            to_value!(id),
        ];
        let _ = tx.exec(sql, params).await?;
        tx.commit()
            .await
            .map_err(|e| AppError::Database(rbatis::Error::from(e)))
    }

    /// 标记数据为云端已删除的数据  本地数据也需要逻辑删除并且标记为已同步
    pub async fn sync_del_by_ids(rb: &RBatis, ids: &Vec<String>, sync_time: u64) -> AppResult<()> {
        let sql = format!(
            "UPDATE clip_record SET del_flag = 1, sync_flag = 2, sync_time = ? WHERE id IN ({})",
            ids.iter().map(|_| "?").collect::<Vec<_>>().join(",")
        );
        let tx = rb.acquire_begin().await?;
        // 转换ids为Vec<Value>
        let mut params = ids.into_iter().map(|id| to_value!(id)).collect::<Vec<_>>();
        params.insert(0, to_value!(sync_time));
        let _ = tx.exec(&sql, params).await?;
        tx.commit()
            .await
            .map_err(|e| AppError::Database(rbatis::Error::from(e)))
    }

    pub async fn del_by_ids(rb: &RBatis, ids: &Vec<String>) -> AppResult<()> {
        let sql = format!(
            "DELETE FROM clip_record WHERE id IN ({})",
            ids.iter().map(|_| "?").collect::<Vec<_>>().join(",")
        );
        let tx = rb.acquire_begin().await?;
        // 转换ids为Vec<Value>
        let params = ids.into_iter().map(|id| to_value!(id)).collect::<Vec<_>>();
        let _ = tx.exec(&sql, params).await?;
        tx.commit()
            .await
            .map_err(|e| AppError::Database(rbatis::Error::from(e)))
    }

    /// 逻辑删除数据并标记为未同步
    pub async fn tombstone_by_ids(rb: &RBatis, ids: &Vec<String>) -> AppResult<()> {
        let sql = format!(
            "UPDATE clip_record set sync_flag = 0, del_flag = 1 WHERE id IN ({})",
            ids.iter().map(|_| "?").collect::<Vec<_>>().join(",")
        );
        let tx = rb.acquire_begin().await?;
        // 转换ids为Vec<Value>
        let params = ids.into_iter().map(|id| to_value!(id)).collect::<Vec<_>>();
        let _ = tx.exec(&sql, params).await?;
        tx.commit()
            .await
            .map_err(|e| AppError::Database(rbatis::Error::from(e)))
    }

    pub async fn select_by_ids(
        rb: &RBatis,
        ids: &Vec<String>,
        limit: i32,
        offset: i32,
    ) -> Result<Vec<ClipRecord>, Error> {
        if ids.is_empty() {
            return Ok(Vec::new());
        }

        let sql = format!(
            "SELECT * FROM clip_record WHERE id IN ({}) and del_flag = 0 ORDER BY pinned_flag DESC, sort DESC, created DESC",
            ids.iter().map(|_| "?").collect::<Vec<_>>().join(",")
        );
        // 转换ids为Vec<Value>
        let mut params = ids.into_iter().map(|id| to_value!(id)).collect::<Vec<_>>();
        params.push(to_value!(limit));
        params.push(to_value!(offset));
        let limit_sql = format!("{} LIMIT ? OFFSET ?", sql);
        let res: Vec<ClipRecord> = rb.query_decode(limit_sql.as_str(), params).await?;
        Ok(res)
    }

    pub async fn insert_by_created_sort(rb: &RBatis, mut record: ClipRecord) -> AppResult<()> {
        let tx = rb.acquire_begin().await?;
        let next_record = ClipRecord::select_order_by_created(rb, record.created).await?;
        if next_record.is_empty() {
            // 获取最新的排序值
            record.sort = ClipRecord::get_next_sort(rb).await;
            ClipRecord::insert(&tx, &record).await?;
        } else {
            let sql = "UPDATE clip_record SET sort = IFNULL(sort, 0) + 1 WHERE created >= ?";
            tx.exec(sql, vec![to_value!(next_record[0].created)])
                .await?;
            record.sort = next_record[0].sort;
            ClipRecord::insert(&tx, &record).await?;
        }
        tx.commit()
            .await
            .map_err(|e| AppError::Database(rbatis::Error::from(e)))
    }

    // /// 获取已同步记录数量（用于VIP限制检查）- 不再需要条数限制
    // pub async fn select_sync_count(rb: &RBatis) -> AppResult<i64> {
    //     use serde::Deserialize;
    //
    //     #[derive(Deserialize)]
    //     struct CountResult {
    //         count: i64,
    //     }
    //
    //     let sql = "SELECT COUNT(*) as count FROM clip_record WHERE sync_flag = 2 AND del_flag = 0";
    //     let result: Vec<CountResult> = rb.query_decode(sql, vec![]).await?;
    //
    //     if let Some(row) = result.first() {
    //         Ok(row.count)
    //     } else {
    //         Ok(0)
    //     }
    // }

    /// 获取所有记录总数（包括未同步的，用于VIP记录数限制检查）
    pub async fn count_all_records(rb: &RBatis) -> Result<i64, Error> {
        use serde::Deserialize;

        #[derive(Deserialize)]
        struct CountResult {
            count: i64,
        }

        let sql = "SELECT COUNT(*) as count FROM clip_record WHERE del_flag = 0";
        let result: Vec<CountResult> = rb.query_decode(sql, vec![]).await?;

        if let Some(row) = result.first() {
            Ok(row.count)
        } else {
            Ok(0)
        }
    }

    /// 删除最旧的记录（用于VIP记录数限制清理）
    pub async fn delete_oldest_records(rb: &RBatis, count: i32) -> Result<(), Error> {
        let sql = "DELETE FROM clip_record WHERE id IN (
            SELECT id FROM clip_record 
            WHERE del_flag = 0 AND pinned_flag = 0 
            ORDER BY sort ASC, created ASC 
            LIMIT ?
        )";
        rb.exec(sql, vec![to_value!(count)]).await?;
        Ok(())
    }
}

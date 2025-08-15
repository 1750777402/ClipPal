use crate::errors::{AppError, AppResult};
use rbatis::{Error, RBatis, crud, impl_select};
use rbs::to_value;
use serde::{Deserialize, Serialize};
use serde_json::Value;

pub static NOT_SYNCHRONIZED: i32 = 0; // 未同步
pub static SYNCHRONIZING: i32 = 1; // 同步中
pub static SYNCHRONIZED: i32 = 2; // 已同步
pub static SKIP_SYNC: i32 = 3; // 跳过同步（文件过大等原因）

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct ClipRecord {
    pub id: String,
    // 类型
    pub r#type: String,
    // 内容
    pub content: Value,
    // 内容md5值
    pub md5_str: String,
    // 时间戳
    pub created: u64,
    // 用户id
    pub user_id: i32,
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
    // 云同步版本号
    pub version: Option<i32>,
    // 是否逻辑删除 0:未删除 1:已删除
    pub del_flag: Option<i32>,
}

crud!(ClipRecord {}, "clip_record");
impl_select!(ClipRecord{select_by_id(id: &str) =>"`where id = #{id}`"});
impl_select!(ClipRecord{select_by_pinned_flag(pinned_flag: i32) =>"`where pinned_flag = #{pinned_flag}`"});
impl_select!(ClipRecord{select_order_by() =>"`order by sort desc, created desc`"});
impl_select!(ClipRecord{select_where_order_by_limit(content: &str, limit:i32, offset:i32) =>"` where content like #{content} order by pinned_flag desc, sort desc, created desc limit #{limit} offset #{offset}`"});
//  根据limit和offset 查询   获取limit条数据(-1表示全部)   跳过前offset条数据
impl_select!(ClipRecord{select_order_by_limit(limit:i32, offset:i32) =>"` where del_flag = 0 order by pinned_flag desc, sort desc, created desc limit #{limit} offset #{offset}`"});
// 根据type和content 查看是否有重复的    有的话取出一个
impl_select!(ClipRecord{check_by_type_and_content(content_type:&str, content:&str) =>"`where type = #{content_type} and content = #{content} limit 1`"});
// 根据type和content 查看是否有重复的    有的话取出一个
impl_select!(ClipRecord{check_by_type_and_md5(content_type:&str, md5_str:&str) =>"`where type = #{content_type} and md5_str = #{md5_str} limit 1`"});
// 取出最大的sort数据
impl_select!(ClipRecord{select_max_sort(user_id: i32) =>"`where user_id = #{user_id} order by sort desc, created desc limit 1`"});
// 根据sync_flag查询记录
impl_select!(ClipRecord{select_by_sync_flag(sync_flag: i32) =>"`where sync_flag = #{sync_flag} order by created desc`"});
// 根据sync_flag查询记录
impl_select!(ClipRecord{select_by_sync_flag_limit(sync_flag: i32, limit: i32) =>"`where sync_flag = #{sync_flag} order by created desc limit #{limit}`"});
// 根据created时间戳查询下一条记录
impl_select!(ClipRecord{select_order_by_created(created: u64) =>"`where created >= #{created} order by created desc limit 1`"});
// 查询已经逻辑删除并且已同步的数据
impl_select!(ClipRecord{select_invalid() =>"`where sync_flag = 2 and del_flag = 1`"});

impl ClipRecord {
    pub async fn update_content(rb: &RBatis, id: &str, content: &String) -> AppResult<()> {
        let sql = "UPDATE clip_record SET content = ? WHERE id = ?";
        let tx = rb.acquire_begin().await?;
        let _ = tx.exec(sql, vec![to_value!(content), to_value!(id)]).await;
        tx.commit()
            .await
            .map_err(|e| AppError::Database(rbatis::Error::from(e)))
    }

    pub async fn get_next_sort(rb: &RBatis) -> i32 {
        ClipRecord::select_max_sort(rb, 0)
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
}

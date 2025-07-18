use crate::{
    CONTEXT,
    utils::{file_dir::get_data_dir, path_utils::to_safe_string},
};
use anyhow::{Error, Ok};
use rbatis::RBatis;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ColumnInfo {
    pub name: String,
    pub r#type: String,
    pub not_null: bool,
    pub default_value: Option<String>,
    pub primary_key: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TableSchema {
    pub name: String,
    pub columns: Vec<ColumnInfo>,
}

/// 当前代码中期望的数据库结构
fn get_expected_schema() -> HashMap<String, TableSchema> {
    let mut schema = HashMap::new();

    // clip_record 表的期望结构
    let clip_record_columns = vec![
        ColumnInfo {
            name: "id".to_string(),
            r#type: "TEXT".to_string(),
            not_null: true,
            default_value: None,
            primary_key: true,
        },
        ColumnInfo {
            name: "type".to_string(),
            r#type: "TEXT".to_string(),
            not_null: true,
            default_value: None,
            primary_key: false,
        },
        ColumnInfo {
            name: "content".to_string(),
            r#type: "TEXT".to_string(),
            not_null: true,
            default_value: None,
            primary_key: false,
        },
        ColumnInfo {
            name: "md5_str".to_string(),
            r#type: "TEXT".to_string(),
            not_null: true,
            default_value: None,
            primary_key: false,
        },
        ColumnInfo {
            name: "created".to_string(),
            r#type: "INTEGER".to_string(),
            not_null: false,
            default_value: None,
            primary_key: false,
        },
        ColumnInfo {
            name: "user_id".to_string(),
            r#type: "INTEGER".to_string(),
            not_null: false,
            default_value: None,
            primary_key: false,
        },
        ColumnInfo {
            name: "os_type".to_string(),
            r#type: "TEXT".to_string(),
            not_null: false,
            default_value: None,
            primary_key: false,
        },
        ColumnInfo {
            name: "sort".to_string(),
            r#type: "INTEGER".to_string(),
            not_null: false,
            default_value: None,
            primary_key: false,
        },
        ColumnInfo {
            name: "pinned_flag".to_string(),
            r#type: "INTEGER".to_string(),
            not_null: false,
            default_value: None,
            primary_key: false,
        },
        ColumnInfo {
            name: "sync_flag".to_string(),
            r#type: "INTEGER".to_string(),
            not_null: false,
            default_value: None,
            primary_key: false,
        },
        ColumnInfo {
            name: "sync_time".to_string(),
            r#type: "INTEGER".to_string(),
            not_null: false,
            default_value: None,
            primary_key: false,
        },
        ColumnInfo {
            name: "device_id".to_string(),
            r#type: "TEXT".to_string(),
            not_null: false,
            default_value: None,
            primary_key: false,
        },
        ColumnInfo {
            name: "version".to_string(),
            r#type: "INTEGER".to_string(),
            not_null: false,
            default_value: Some("0".to_string()),
            primary_key: false,
        },
        ColumnInfo {
            name: "del_flag".to_string(),
            r#type: "INTEGER".to_string(),
            not_null: true,
            default_value: Some("0".to_string()),
            primary_key: false,
        },
    ];

    schema.insert(
        "clip_record".to_string(),
        TableSchema {
            name: "clip_record".to_string(),
            columns: clip_record_columns,
        },
    );

    schema
}

#[derive(Debug, Deserialize)]
struct TableName {
    name: String,
}

#[derive(Debug, Deserialize)]
struct RawColumnInfo {
    #[serde(rename = "cid")]
    pub _cid: i32,
    pub name: String,
    #[serde(rename = "type")]
    pub r#type: String,
    pub notnull: i32,
    pub dflt_value: Option<String>,
    pub pk: i32,
}

/// 获取数据库中实际存在的表结构
async fn get_actual_schema(rb: &RBatis) -> Result<HashMap<String, TableSchema>, Error> {
    let mut schema = HashMap::new();

    // 获取所有表名
    let tables: Vec<TableName> = rb
        .query_decode(
            "SELECT name FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%'",
            vec![],
        )
        .await?;

    for table_name in tables {
        // 查询 PRAGMA 表结构信息
        let raw_columns: Vec<RawColumnInfo> = rb
            .query_decode(&format!("PRAGMA table_info({})", table_name.name), vec![])
            .await?;

        let columns: Vec<ColumnInfo> = raw_columns
            .into_iter()
            .map(|raw| ColumnInfo {
                name: raw.name,
                r#type: raw.r#type,
                not_null: raw.notnull != 0,
                default_value: raw.dflt_value,
                primary_key: raw.pk != 0,
            })
            .collect();

        schema.insert(
            table_name.name.clone(),
            TableSchema {
                name: table_name.name,
                columns,
            },
        );
    }

    Ok(schema)
}

/// 比较期望结构和实际结构，返回需要执行的迁移操作
fn compare_schemas(
    expected: &HashMap<String, TableSchema>,
    actual: &HashMap<String, TableSchema>,
) -> Vec<String> {
    let mut migrations = Vec::new();

    for (table_name, expected_table) in expected {
        if let Some(actual_table) = actual.get(table_name) {
            // 表存在，检查字段
            let expected_columns: HashMap<&String, &ColumnInfo> = expected_table
                .columns
                .iter()
                .map(|col| (&col.name, col))
                .collect();

            let actual_columns: HashMap<&String, &ColumnInfo> = actual_table
                .columns
                .iter()
                .map(|col| (&col.name, col))
                .collect();

            // 检查缺失的字段
            for (col_name, expected_col) in &expected_columns {
                if !actual_columns.contains_key(col_name) {
                    let sql = format!(
                        "ALTER TABLE {} ADD COLUMN {} {}",
                        table_name, col_name, expected_col.r#type
                    );
                    migrations.push(sql);
                }
            }
        } else {
            // 表不存在，创建表
            let mut create_sql = format!("CREATE TABLE {} (", table_name);
            let column_definitions: Vec<String> = expected_table
                .columns
                .iter()
                .map(|col| {
                    let mut def = format!("{} {}", col.name, col.r#type);
                    if col.not_null {
                        def.push_str(" NOT NULL");
                    }
                    if col.primary_key {
                        def.push_str(" PRIMARY KEY");
                    }
                    if let Some(ref default_val) = col.default_value {
                        def.push_str(&format!(" DEFAULT {}", default_val));
                    }
                    def
                })
                .collect();
            create_sql.push_str(&column_definitions.join(", "));
            create_sql.push(')');
            migrations.push(create_sql);
        }
    }

    migrations
}

/// 执行数据库迁移
async fn execute_migrations(rb: &RBatis, migrations: Vec<String>) -> Result<(), Error> {
    for migration in migrations {
        log::debug!("执行数据库迁移: {}", migration);
        rb.acquire().await?.exec(&migration, vec![]).await?;
    }
    Ok(())
}

/// 创建索引
async fn create_indexes(rb: &RBatis) -> Result<(), Error> {
    let index_sqls = vec![
        "CREATE INDEX IF NOT EXISTS idx_clip_record_md5_str ON clip_record(md5_str)",
        "CREATE INDEX IF NOT EXISTS idx_clip_record_created ON clip_record(created)",
        "CREATE INDEX IF NOT EXISTS idx_clip_record_sort ON clip_record(sort)",
        "CREATE INDEX IF NOT EXISTS idx_clip_record_pinned ON clip_record(pinned_flag)",
    ];

    for sql in index_sqls {
        rb.acquire().await?.exec(sql, vec![]).await?;
    }

    Ok(())
}

/// 检查并修复数据库结构
async fn check_and_fix_database_schema(rb: &RBatis) -> Result<(), Error> {
    log::debug!("检查数据库结构...");

    // 获取期望的结构
    let expected_schema = get_expected_schema();

    // 获取实际的结构
    let actual_schema = get_actual_schema(rb).await?;

    // 比较结构并生成迁移操作
    let migrations = compare_schemas(&expected_schema, &actual_schema);

    if migrations.is_empty() {
        log::debug!("数据库结构检查完成，无需迁移");
    } else {
        log::debug!("发现 {} 个需要执行的迁移操作", migrations.len());

        // 执行迁移
        execute_migrations(rb, migrations).await?;

        log::debug!("数据库迁移完成");
    }

    // 创建索引
    create_indexes(rb).await?;

    Ok(())
}

pub async fn init_sqlite() -> Result<RBatis, Error> {
    // 创建sqlite链接
    let rb = RBatis::new();

    // 安全地处理数据库路径，确保中文字符正确处理
    let db_path = get_data_dir()
        .ok_or_else(|| anyhow::anyhow!("无法获取数据目录"))?
        .join("clip_record.db");

    // 使用工具函数安全地处理路径
    let db_path_str = to_safe_string(&db_path);

    rb.init(rbdc_sqlite::Driver {}, &format!("sqlite://{}", db_path_str))
        .map_err(|e| anyhow::anyhow!("数据库连接初始化失败: {}", e))?;

    // 检查并修复数据库结构
    check_and_fix_database_schema(&rb).await?;

    // 把sqlite链接放入全局变量中
    CONTEXT.set(rb.clone());

    log::info!("数据库初始化完成");

    Ok(rb)
}

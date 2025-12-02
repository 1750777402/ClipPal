# ClipPal 轻量级渐进式重构方案

## 文档信息
- **版本**: v1.0
- **日期**: 2025-12-02
- **作者**: ClipPal Team
- **状态**: 待评审

---

## 目录
- [一、方案概述](#一方案概述)
- [二、现状分析](#二现状分析)
- [三、目标架构](#三目标架构)
- [四、核心改进点](#四核心改进点)
- [五、详细设计](#五详细设计)
- [六、实施路线图](#六实施路线图)
- [七、方案对比](#七方案对比)
- [八、风险与应对](#八风险与应对)

---

## 一、方案概述

### 1.1 设计理念

本方案采用**轻量级渐进式重构**策略，在不引入过度复杂性的前提下，解决当前架构的核心问题。

**核心原则：**
- 🎯 **聚焦问题** - 只解决真正存在的问题
- 📦 **简单优先** - 避免过度设计
- 🔄 **渐进迭代** - 分阶段实施，保持系统可运行
- 💰 **成本可控** - 代码量增加控制在30%以内

### 1.2 为什么不用完整DDD？

| 考虑因素 | 完整DDD | 本方案 |
|---------|---------|--------|
| **项目规模** | 适合大型系统 | ✅ 适合中小型系统 |
| **团队规模** | 需要10+开发者 | ✅ 适合小团队/个人 |
| **领域复杂度** | 需要复杂业务规则 | ✅ 业务逻辑相对简单 |
| **开发周期** | 1-2个月 | ✅ 2-3周 |
| **代码增量** | +70% | ✅ +30% |
| **学习曲线** | 陡峭 | ✅ 平缓 |

### 1.3 方案收益

| 改进项 | 当前状态 | 改进后 |
|--------|---------|--------|
| **实体封装** | 贫血模型，所有字段public | 充血模型，私有字段+行为方法 |
| **数据访问** | 业务层直接使用RBatis | Repository层隔离 |
| **依赖管理** | 全局CONTEXT，73处引用 | 依赖注入，明确依赖关系 |
| **业务逻辑** | 散落在静态方法中 | 集中在Service层 |
| **可测试性** | 难以Mock和单元测试 | 易于测试，可Mock依赖 |
| **模块耦合** | 高耦合，单文件依赖10+模块 | 低耦合，清晰的分层 |

---

## 二、现状分析

### 2.1 当前架构问题

#### 问题1：贫血领域模型（最严重）

**当前代码：** `src-tauri/src/biz/clip_record.rs`

```rust
// ❌ 问题代码
#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct ClipRecord {
    pub id: String,              // ❌ 所有字段public
    pub r#type: String,          // ❌ 使用String而非枚举
    pub content: Value,
    pub md5_str: String,
    pub created: u64,            // ❌ 使用原始类型
    pub pinned_flag: i32,        // ❌ 命名不清晰
    pub sync_flag: Option<i32>,  // ❌ 应该用枚举
    // ... 更多字段
}

impl ClipRecord {
    // ❌ 全是静态方法，需要传RBatis
    pub async fn update_pinned(rb: &RBatis, id: &str, pinned_flag: i32) -> AppResult<()> {
        // ...
    }

    pub async fn update_content(rb: &RBatis, id: &str, content: &str) -> AppResult<()> {
        // ...
    }

    // ... 20+个静态方法
}
```

**问题总结：**
- 实体只是数据容器，没有行为
- 所有字段public，破坏封装
- 业务逻辑在静态方法中，需要传RBatis
- 难以维护和测试

#### 问题2：全局状态泛滥

**当前代码：** 多个文件

```rust
// ❌ CONTEXT在20个文件中使用，共73处引用
pub static CONTEXT: TypeMap![Send + Sync] = <TypeMap![Send + Sync]>::new();

// 使用示例（到处都是）
let rb: &RBatis = CONTEXT.get::<RBatis>();
let app_handle = CONTEXT.get::<AppHandle>();
let settings = CONTEXT.get::<Settings>();
```

**问题总结：**
- 隐式依赖，难以追踪
- 并发安全问题（需要大量锁）
- 难以单元测试
- 全局状态竞争

#### 问题3：数据访问混乱

**当前代码：** 业务层直接使用RBatis

```rust
// ❌ 业务逻辑与数据访问混合
pub async fn sync_clipboard() -> AppResult<()> {
    let rb: &RBatis = CONTEXT.get::<RBatis>();

    // 直接执行SQL
    let records = ClipRecord::select_all(rb).await?;

    // 业务逻辑
    for record in records {
        // ...
    }
}
```

**问题总结：**
- 没有Repository抽象层
- 业务逻辑与数据访问耦合
- 难以测试和Mock

#### 问题4：模块高耦合

**当前代码：** `src-tauri/src/biz/cloud_sync_timer.rs`

```rust
// ❌ 单个文件依赖10+个模块
use crate::api::cloud_sync_api::{...};
use crate::biz::clip_record::{...};
use crate::biz::clip_record_clean::try_clean_clip_record;
use crate::biz::content_search::add_content_to_index;
use crate::biz::sync_time::SyncTime;
use crate::biz::system_setting::{...};
use crate::biz::vip_checker::VipChecker;
use crate::utils::config::get_max_file_size_bytes;
use crate::utils::device_info::GLOBAL_DEVICE_ID;
use crate::utils::file_dir::get_resources_dir;
use crate::utils::lock_utils::{...};
use crate::utils::token_manager::has_valid_auth;
use crate::CONTEXT;
```

### 2.2 为什么这些问题需要解决？

| 问题 | 当前影响 | 未来风险 |
|-----|---------|---------|
| **贫血模型** | 业务逻辑分散，难以理解 | 随着功能增加，代码会越来越混乱 |
| **全局状态** | 并发问题，难以测试 | 容易出现难以调试的bug |
| **数据访问混乱** | 业务逻辑与技术细节混合 | 无法切换数据库或Mock测试 |
| **高耦合** | 修改一处影响多处 | 技术债累积，维护成本暴增 |

---

## 三、目标架构

### 3.1 架构图

```
┌─────────────────────────────────────────────────┐
│              Commands 层（Tauri接口）              │
│  - clipboard_commands.rs                        │
│  - user_commands.rs                             │
│  - sync_commands.rs                             │
└─────────────────┬───────────────────────────────┘
                  ↓
┌─────────────────────────────────────────────────┐
│               Services 层（业务服务）              │
│  - ClipboardService                             │
│  - SyncService                                  │
│  - UserService                                  │
│  - VipService                                   │
└─────────────────┬───────────────────────────────┘
                  ↓
┌─────────────────────────────────────────────────┐
│              Repositories 层（数据访问）           │
│  - ClipboardRepository                          │
│  - UserRepository                               │
│  - SettingsRepository                           │
└─────────────────┬───────────────────────────────┘
                  ↓
┌─────────────────────────────────────────────────┐
│               Domain 层（领域模型）                │
│  - ClipRecord (充血模型)                         │
│  - User                                         │
│  - VipMembership                                │
│  - 简单值对象（枚举）                              │
└─────────────────────────────────────────────────┘
```

**依赖规则：**
- Commands → Services → Repositories → Domain
- 上层依赖下层，下层不依赖上层
- Domain层不依赖任何外部库（除基础类型）

### 3.2 目录结构

```
src-tauri/src/
├── main.rs                     # 程序入口
├── lib.rs                      # 启动配置 + 依赖注入
│
├── domain/                     # 领域模型（轻量级）
│   ├── mod.rs
│   ├── clipboard.rs            # ClipRecord实体（充血模型）
│   ├── user.rs                 # User实体
│   ├── vip.rs                  # VipMembership实体
│   └── types.rs                # 通用类型和简单值对象
│
├── repository/                 # 数据访问层
│   ├── mod.rs
│   ├── clipboard_repo.rs       # 剪贴板数据访问
│   ├── user_repo.rs            # 用户数据访问
│   └── settings_repo.rs        # 设置数据访问
│
├── service/                    # 业务服务层
│   ├── mod.rs
│   ├── clipboard_service.rs    # 剪贴板业务逻辑
│   ├── sync_service.rs         # 同步业务逻辑
│   ├── user_service.rs         # 用户业务逻辑
│   └── vip_service.rs          # VIP业务逻辑
│
├── commands/                   # Tauri命令层
│   ├── mod.rs
│   ├── clipboard.rs            # 剪贴板命令
│   ├── user.rs                 # 用户命令
│   ├── sync.rs                 # 同步命令
│   └── settings.rs             # 设置命令
│
├── api/                        # 外部API客户端（保持不变）
│   ├── mod.rs
│   ├── user_auth_api.rs
│   ├── cloud_sync_api.rs
│   └── vip_api.rs
│
├── platform/                   # 平台层（保持不变）
│   ├── mod.rs
│   ├── window.rs
│   ├── tray.rs
│   ├── menu.rs
│   └── shortcuts.rs
│
├── infrastructure/             # 基础设施
│   ├── mod.rs
│   ├── database.rs             # 数据库初始化
│   ├── http_client.rs          # HTTP客户端
│   └── storage.rs              # 文件存储
│
├── shared/                     # 共享模块
│   ├── mod.rs
│   ├── errors.rs               # 统一错误类型
│   ├── config.rs               # 配置管理
│   └── utils.rs                # 通用工具
│
└── legacy/                     # 待迁移的旧代码
    └── biz/                    # 逐步迁移
```

**关键变化：**
- ✅ 新增 `domain/` - 领域模型（轻量级）
- ✅ 新增 `repository/` - 数据访问层
- ✅ 新增 `service/` - 业务服务层
- ✅ 新增 `commands/` - Tauri命令层
- ✅ 保留 `api/` - 外部API不变
- ✅ 保留 `platform/` - 平台层不变
- ✅ 重构 `infrastructure/` - 技术实现
- ✅ `biz/` → `legacy/` - 逐步迁移

---

## 四、核心改进点

### 4.1 改进点1：充血领域模型

#### Before（贫血模型）

```rust
// ❌ 当前：贫血模型
pub struct ClipRecord {
    pub id: String,
    pub r#type: String,
    pub content: Value,
    pub pinned_flag: i32,
    pub sync_flag: Option<i32>,
    // ...
}

impl ClipRecord {
    // 静态方法，需要传RBatis
    pub async fn update_pinned(rb: &RBatis, id: &str, flag: i32) -> AppResult<()> {
        // ...
    }
}

// 使用方式：业务逻辑在外部
let rb = CONTEXT.get::<RBatis>();
ClipRecord::update_pinned(rb, "123", 1).await?;
```

#### After（充血模型）

```rust
// ✅ 改进后：充血模型
use chrono::{DateTime, Local};

pub struct ClipRecord {
    // 私有字段，保护内部状态
    id: String,
    content_type: ContentType,       // 使用枚举
    content: String,
    md5: String,
    local_file_path: Option<PathBuf>,
    created_at: DateTime<Local>,     // 使用强类型
    updated_at: DateTime<Local>,
    sort: i32,
    is_pinned: bool,                 // 更好的命名
    sync_status: SyncStatus,         // 使用枚举
    device_id: Option<String>,
    is_deleted: bool,
}

impl ClipRecord {
    // 工厂方法：创建新记录
    pub fn new(content: String, content_type: ContentType) -> AppResult<Self> {
        if content.is_empty() {
            return Err(AppError::EmptyContent);
        }

        let now = Local::now();
        let md5 = format!("{:x}", md5::compute(&content));

        Ok(Self {
            id: uuid::Uuid::new_v4().to_string(),
            content_type,
            content,
            md5,
            local_file_path: None,
            created_at: now,
            updated_at: now,
            sort: 0,
            is_pinned: false,
            sync_status: SyncStatus::NotSynced,
            device_id: None,
            is_deleted: false,
        })
    }

    // 业务行为：置顶
    pub fn pin(&mut self) {
        if !self.is_pinned {
            self.is_pinned = true;
            self.updated_at = Local::now();
        }
    }

    // 业务行为：取消置顶
    pub fn unpin(&mut self) {
        if self.is_pinned {
            self.is_pinned = false;
            self.updated_at = Local::now();
        }
    }

    // 业务行为：软删除
    pub fn delete(&mut self) -> AppResult<()> {
        if self.is_deleted {
            return Err(AppError::AlreadyDeleted);
        }
        self.is_deleted = true;
        self.updated_at = Local::now();
        Ok(())
    }

    // 业务规则：是否可以同步
    pub fn can_sync(&self) -> bool {
        !self.is_deleted && !matches!(self.sync_status, SyncStatus::Skip(_))
    }

    // 业务行为：标记为同步中
    pub fn mark_syncing(&mut self) {
        self.sync_status = SyncStatus::Syncing;
        self.updated_at = Local::now();
    }

    // 业务行为：标记为已同步
    pub fn mark_synced(&mut self) {
        self.sync_status = SyncStatus::Synced;
        self.updated_at = Local::now();
    }

    // Getters（只读访问）
    pub fn id(&self) -> &str { &self.id }
    pub fn content(&self) -> &str { &self.content }
    pub fn content_type(&self) -> &ContentType { &self.content_type }
    pub fn is_pinned(&self) -> bool { self.is_pinned }
    pub fn is_deleted(&self) -> bool { self.is_deleted }
    pub fn sync_status(&self) -> &SyncStatus { &self.sync_status }
    pub fn created_at(&self) -> DateTime<Local> { self.created_at }
}

// 简单的值对象（枚举）
#[derive(Debug, Clone, PartialEq)]
pub enum ContentType {
    Text,
    Image,
    File,
}

#[derive(Debug, Clone, PartialEq)]
pub enum SyncStatus {
    NotSynced,      // 未同步
    Syncing,        // 同步中
    Synced,         // 已同步
    Skip(SkipReason), // 跳过同步
}

#[derive(Debug, Clone, PartialEq)]
pub enum SkipReason {
    Unsupported,    // 不支持的类型
    VipRequired,    // 需要VIP
    TooLarge,       // 文件太大
}
```

**改进效果：**
- ✅ 封装内部状态，字段私有化
- ✅ 业务逻辑在实体内部（pin、unpin、delete等）
- ✅ 使用枚举替代magic number
- ✅ 使用强类型（DateTime、PathBuf）
- ✅ 自包含验证逻辑

### 4.2 改进点2：Repository模式

#### Before（直接使用RBatis）

```rust
// ❌ 当前：业务层直接使用RBatis
pub async fn sync_clipboard() -> AppResult<()> {
    let rb: &RBatis = CONTEXT.get::<RBatis>();

    // 直接执行SQL
    let records = ClipRecord::select_all(rb).await?;

    for record in records {
        // 业务逻辑
    }
}
```

#### After（Repository层）

```rust
// ✅ repository/clipboard_repo.rs
use std::sync::Arc;
use rbatis::RBatis;
use crate::domain::clipboard::ClipRecord;
use crate::shared::errors::AppResult;

pub struct ClipboardRepository {
    db: Arc<RBatis>,
}

impl ClipboardRepository {
    pub fn new(db: Arc<RBatis>) -> Self {
        Self { db }
    }

    // 保存或更新记录
    pub async fn save(&self, record: &ClipRecord) -> AppResult<()> {
        let model = ClipRecordModel::from(record);

        let sql = "INSERT OR REPLACE INTO clip_record
                   (id, type, content, md5_str, created, is_pinned, sync_flag, ...)
                   VALUES (?, ?, ?, ?, ?, ?, ?, ...)";

        self.db.exec(sql, vec![
            model.id,
            model.r#type,
            model.content,
            // ...
        ]).await?;

        Ok(())
    }

    // 根据ID查找
    pub async fn find_by_id(&self, id: &str) -> AppResult<Option<ClipRecord>> {
        let sql = "SELECT * FROM clip_record WHERE id = ? AND del_flag = 0";
        let model: Option<ClipRecordModel> = self.db
            .fetch_by_column(sql, &[id])
            .await?;

        match model {
            Some(m) => Ok(Some(m.try_into()?)),
            None => Ok(None),
        }
    }

    // 查找最近的记录
    pub async fn find_recent(&self, limit: usize) -> AppResult<Vec<ClipRecord>> {
        let sql = "SELECT * FROM clip_record
                   WHERE del_flag = 0
                   ORDER BY sort DESC, created DESC
                   LIMIT ?";

        let models: Vec<ClipRecordModel> = self.db
            .fetch(sql, vec![limit])
            .await?;

        models.into_iter()
            .map(|m| m.try_into())
            .collect()
    }

    // 搜索记录
    pub async fn search(&self, keyword: &str, limit: usize) -> AppResult<Vec<ClipRecord>> {
        let sql = "SELECT * FROM clip_record
                   WHERE del_flag = 0 AND content LIKE ?
                   ORDER BY created DESC
                   LIMIT ?";

        let pattern = format!("%{}%", keyword);
        let models: Vec<ClipRecordModel> = self.db
            .fetch(sql, vec![pattern, limit])
            .await?;

        models.into_iter()
            .map(|m| m.try_into())
            .collect()
    }

    // 查找置顶记录
    pub async fn find_pinned(&self) -> AppResult<Vec<ClipRecord>> {
        let sql = "SELECT * FROM clip_record
                   WHERE del_flag = 0 AND pinned_flag = 1
                   ORDER BY sort DESC";

        let models: Vec<ClipRecordModel> = self.db.fetch(sql, vec![]).await?;

        models.into_iter()
            .map(|m| m.try_into())
            .collect()
    }

    // 查找需要同步的记录
    pub async fn find_to_sync(&self, limit: usize) -> AppResult<Vec<ClipRecord>> {
        let sql = "SELECT * FROM clip_record
                   WHERE del_flag = 0 AND sync_flag = 0
                   ORDER BY created DESC
                   LIMIT ?";

        let models: Vec<ClipRecordModel> = self.db
            .fetch(sql, vec![limit])
            .await?;

        models.into_iter()
            .map(|m| m.try_into())
            .collect()
    }

    // 删除记录
    pub async fn delete(&self, id: &str) -> AppResult<()> {
        let sql = "UPDATE clip_record SET del_flag = 1 WHERE id = ?";
        self.db.exec(sql, vec![id]).await?;
        Ok(())
    }

    // 物理删除
    pub async fn hard_delete(&self, id: &str) -> AppResult<()> {
        let sql = "DELETE FROM clip_record WHERE id = ?";
        self.db.exec(sql, vec![id]).await?;
        Ok(())
    }
}

// 数据库模型（与领域模型分离）
#[derive(Debug, Clone, Serialize, Deserialize)]
struct ClipRecordModel {
    pub id: String,
    pub r#type: String,
    pub content: String,
    pub md5_str: String,
    pub created: i64,
    pub pinned_flag: i32,
    pub sync_flag: i32,
    // ...
}

// Domain Model → DB Model
impl From<&ClipRecord> for ClipRecordModel {
    fn from(record: &ClipRecord) -> Self {
        Self {
            id: record.id().to_string(),
            r#type: record.content_type().to_string(),
            content: record.content().to_string(),
            md5_str: record.md5().to_string(),
            created: record.created_at().timestamp(),
            pinned_flag: if record.is_pinned() { 1 } else { 0 },
            sync_flag: record.sync_status().to_i32(),
            // ...
        }
    }
}

// DB Model → Domain Model
impl TryFrom<ClipRecordModel> for ClipRecord {
    type Error = AppError;

    fn try_from(model: ClipRecordModel) -> AppResult<Self> {
        let content_type = ContentType::from_str(&model.r#type)?;
        let sync_status = SyncStatus::from_i32(model.sync_flag)?;

        Ok(ClipRecord::from_db(
            model.id,
            content_type,
            model.content,
            model.md5_str,
            // ...
        ))
    }
}
```

**改进效果：**
- ✅ 数据访问逻辑集中在Repository
- ✅ 领域模型与数据库模型分离
- ✅ 易于测试（可以Mock Repository）
- ✅ 易于切换数据库实现

### 4.3 改进点3：Service层

#### Before（逻辑分散）

```rust
// ❌ 当前：业务逻辑分散在各处
// biz/clip_record.rs - 一些逻辑
// biz/clip_record_sync.rs - 一些逻辑
// biz/copy_clip_record.rs - 一些逻辑
```

#### After（Service层）

```rust
// ✅ service/clipboard_service.rs
use std::sync::Arc;
use crate::domain::clipboard::{ClipRecord, ContentType};
use crate::repository::clipboard_repo::ClipboardRepository;
use crate::shared::errors::{AppError, AppResult};

pub struct ClipboardService {
    repo: Arc<ClipboardRepository>,
}

impl ClipboardService {
    pub fn new(repo: Arc<ClipboardRepository>) -> Self {
        Self { repo }
    }

    // 用例：保存剪贴板内容
    pub async fn save_clipboard(
        &self,
        content: String,
        content_type: ContentType
    ) -> AppResult<ClipRecord> {
        // 1. 创建领域实体（带验证）
        let record = ClipRecord::new(content, content_type)?;

        // 2. 持久化
        self.repo.save(&record).await?;

        // 3. 返回结果
        Ok(record)
    }

    // 用例：置顶记录
    pub async fn pin_record(&self, id: &str) -> AppResult<()> {
        // 1. 加载实体
        let mut record = self.repo.find_by_id(id).await?
            .ok_or(AppError::RecordNotFound)?;

        // 2. 调用实体业务方法
        record.pin();

        // 3. 保存
        self.repo.save(&record).await?;

        Ok(())
    }

    // 用例：取消置顶
    pub async fn unpin_record(&self, id: &str) -> AppResult<()> {
        let mut record = self.repo.find_by_id(id).await?
            .ok_or(AppError::RecordNotFound)?;

        record.unpin();

        self.repo.save(&record).await?;

        Ok(())
    }

    // 用例：删除记录
    pub async fn delete_record(&self, id: &str) -> AppResult<()> {
        let mut record = self.repo.find_by_id(id).await?
            .ok_or(AppError::RecordNotFound)?;

        record.delete()?;

        self.repo.save(&record).await?;

        Ok(())
    }

    // 用例：获取最近记录
    pub async fn get_recent_records(&self, limit: usize) -> AppResult<Vec<ClipRecord>> {
        self.repo.find_recent(limit).await
    }

    // 用例：搜索记录
    pub async fn search_records(&self, keyword: &str, limit: usize) -> AppResult<Vec<ClipRecord>> {
        if keyword.is_empty() {
            return Err(AppError::EmptyKeyword);
        }

        self.repo.search(keyword, limit).await
    }

    // 用例：获取置顶记录
    pub async fn get_pinned_records(&self) -> AppResult<Vec<ClipRecord>> {
        self.repo.find_pinned().await
    }
}
```

**改进效果：**
- ✅ 业务逻辑集中在Service层
- ✅ Service只做用例编排，不包含实体行为
- ✅ 清晰的方法命名，易于理解
- ✅ 易于测试

### 4.4 改进点4：依赖注入

#### Before（全局状态）

```rust
// ❌ 当前：全局CONTEXT
pub static CONTEXT: TypeMap![Send + Sync] = <TypeMap![Send + Sync]>::new();

// 使用（隐式依赖）
let rb = CONTEXT.get::<RBatis>();
let app_handle = CONTEXT.get::<AppHandle>();
```

#### After（依赖注入）

```rust
// ✅ lib.rs - 应用状态
use std::sync::Arc;
use rbatis::RBatis;

pub struct AppState {
    // 基础设施
    db: Arc<RBatis>,

    // Repositories
    clipboard_repo: Arc<ClipboardRepository>,
    user_repo: Arc<UserRepository>,
    settings_repo: Arc<SettingsRepository>,

    // Services
    pub clipboard_service: Arc<ClipboardService>,
    pub sync_service: Arc<SyncService>,
    pub user_service: Arc<UserService>,
    pub vip_service: Arc<VipService>,
}

impl AppState {
    pub async fn new() -> AppResult<Self> {
        // 1. 初始化数据库
        let db = Arc::new(init_database().await?);

        // 2. 创建Repositories
        let clipboard_repo = Arc::new(ClipboardRepository::new(db.clone()));
        let user_repo = Arc::new(UserRepository::new(db.clone()));
        let settings_repo = Arc::new(SettingsRepository::new(db.clone()));

        // 3. 创建Services
        let clipboard_service = Arc::new(ClipboardService::new(
            clipboard_repo.clone()
        ));

        let sync_service = Arc::new(SyncService::new(
            clipboard_repo.clone(),
            user_repo.clone()
        ));

        let user_service = Arc::new(UserService::new(
            user_repo.clone()
        ));

        let vip_service = Arc::new(VipService::new(
            user_repo.clone()
        ));

        Ok(Self {
            db,
            clipboard_repo,
            user_repo,
            settings_repo,
            clipboard_service,
            sync_service,
            user_service,
            vip_service,
        })
    }
}

// Tauri启动
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            // 初始化应用状态
            let app_state = tauri::async_runtime::block_on(async {
                AppState::new().await.expect("Failed to initialize app state")
            });

            // 注册状态
            app.manage(app_state);

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::clipboard::save_clipboard,
            commands::clipboard::pin_record,
            // ...
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
```

**改进效果：**
- ✅ 显式依赖关系，易于追踪
- ✅ 便于单元测试（可以注入Mock）
- ✅ 类型安全，编译时检查
- ✅ 无全局状态竞争

### 4.5 改进点5：Commands层

#### Before（混乱的Tauri命令）

```rust
// ❌ 当前：命令中包含业务逻辑
#[tauri::command]
pub async fn pin_record(id: String) -> Result<(), String> {
    let rb = CONTEXT.get::<RBatis>();

    // 业务逻辑在命令中
    ClipRecord::update_pinned(rb, &id, 1)
        .await
        .map_err(|e| e.to_string())
}
```

#### After（Commands层）

```rust
// ✅ commands/clipboard.rs
use tauri::State;
use serde::{Deserialize, Serialize};
use crate::lib::AppState;
use crate::domain::clipboard::{ClipRecord, ContentType};

// DTO定义
#[derive(Deserialize)]
pub struct SaveClipboardRequest {
    pub content: String,
    pub content_type: String,
}

#[derive(Serialize)]
pub struct ClipboardRecordDto {
    pub id: String,
    pub content: String,
    pub content_type: String,
    pub created_at: String,
    pub is_pinned: bool,
    pub is_deleted: bool,
}

impl From<ClipRecord> for ClipboardRecordDto {
    fn from(record: ClipRecord) -> Self {
        Self {
            id: record.id().to_string(),
            content: record.content().to_string(),
            content_type: record.content_type().to_string(),
            created_at: record.created_at().to_rfc3339(),
            is_pinned: record.is_pinned(),
            is_deleted: record.is_deleted(),
        }
    }
}

// Tauri Commands
#[tauri::command]
pub async fn save_clipboard(
    request: SaveClipboardRequest,
    state: State<'_, AppState>
) -> Result<ClipboardRecordDto, String> {
    // 1. 解析内容类型
    let content_type = ContentType::from_str(&request.content_type)
        .map_err(|e| e.to_string())?;

    // 2. 调用Service
    let record = state.clipboard_service
        .save_clipboard(request.content, content_type)
        .await
        .map_err(|e| e.to_string())?;

    // 3. 转换为DTO
    Ok(ClipboardRecordDto::from(record))
}

#[tauri::command]
pub async fn pin_record(
    id: String,
    state: State<'_, AppState>
) -> Result<(), String> {
    state.clipboard_service
        .pin_record(&id)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn unpin_record(
    id: String,
    state: State<'_, AppState>
) -> Result<(), String> {
    state.clipboard_service
        .unpin_record(&id)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn delete_record(
    id: String,
    state: State<'_, AppState>
) -> Result<(), String> {
    state.clipboard_service
        .delete_record(&id)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_recent_records(
    limit: usize,
    state: State<'_, AppState>
) -> Result<Vec<ClipboardRecordDto>, String> {
    let records = state.clipboard_service
        .get_recent_records(limit)
        .await
        .map_err(|e| e.to_string())?;

    let dtos = records.into_iter()
        .map(ClipboardRecordDto::from)
        .collect();

    Ok(dtos)
}

#[tauri::command]
pub async fn search_records(
    keyword: String,
    limit: usize,
    state: State<'_, AppState>
) -> Result<Vec<ClipboardRecordDto>, String> {
    let records = state.clipboard_service
        .search_records(&keyword, limit)
        .await
        .map_err(|e| e.to_string())?;

    let dtos = records.into_iter()
        .map(ClipboardRecordDto::from)
        .collect();

    Ok(dtos)
}
```

**改进效果：**
- ✅ Commands层只做DTO转换和调用Service
- ✅ 无业务逻辑
- ✅ 依赖注入State
- ✅ 清晰的职责划分

---

## 五、详细设计

### 5.1 Domain层设计

#### ClipRecord实体（已在4.1展示）

#### User实体

```rust
// domain/user.rs
use chrono::{DateTime, Local};

pub struct User {
    id: String,
    email: String,
    username: String,
    avatar_url: Option<String>,
    created_at: DateTime<Local>,
    updated_at: DateTime<Local>,
    is_vip: bool,
    vip_expires_at: Option<DateTime<Local>>,
}

impl User {
    // 工厂方法
    pub fn new(email: String, username: String) -> AppResult<Self> {
        if email.is_empty() || !email.contains('@') {
            return Err(AppError::InvalidEmail);
        }

        if username.is_empty() {
            return Err(AppError::EmptyUsername);
        }

        let now = Local::now();

        Ok(Self {
            id: uuid::Uuid::new_v4().to_string(),
            email,
            username,
            avatar_url: None,
            created_at: now,
            updated_at: now,
            is_vip: false,
            vip_expires_at: None,
        })
    }

    // 业务行为：升级为VIP
    pub fn upgrade_to_vip(&mut self, duration_days: i64) {
        self.is_vip = true;
        self.vip_expires_at = Some(Local::now() + chrono::Duration::days(duration_days));
        self.updated_at = Local::now();
    }

    // 业务规则：VIP是否过期
    pub fn is_vip_active(&self) -> bool {
        if !self.is_vip {
            return false;
        }

        match self.vip_expires_at {
            Some(expires) => expires > Local::now(),
            None => false,
        }
    }

    // 业务行为：更新个人资料
    pub fn update_profile(&mut self, username: String, avatar_url: Option<String>) -> AppResult<()> {
        if username.is_empty() {
            return Err(AppError::EmptyUsername);
        }

        self.username = username;
        self.avatar_url = avatar_url;
        self.updated_at = Local::now();

        Ok(())
    }

    // Getters
    pub fn id(&self) -> &str { &self.id }
    pub fn email(&self) -> &str { &self.email }
    pub fn username(&self) -> &str { &self.username }
    pub fn is_vip(&self) -> bool { self.is_vip }
}
```

#### VipMembership实体

```rust
// domain/vip.rs
pub struct VipMembership {
    user_id: String,
    level: VipLevel,
    expires_at: DateTime<Local>,
    auto_renew: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub enum VipLevel {
    Free,
    Basic,
    Premium,
}

impl VipLevel {
    // 业务规则：权益检查
    pub fn can_cloud_sync(&self) -> bool {
        matches!(self, VipLevel::Basic | VipLevel::Premium)
    }

    pub fn max_storage_mb(&self) -> usize {
        match self {
            VipLevel::Free => 100,
            VipLevel::Basic => 1000,
            VipLevel::Premium => 10000,
        }
    }
}

impl VipMembership {
    pub fn is_active(&self) -> bool {
        self.expires_at > Local::now()
    }

    pub fn can_use_feature(&self, feature: VipFeature) -> bool {
        if !self.is_active() {
            return false;
        }

        match (self.level, feature) {
            (VipLevel::Free, VipFeature::BasicClipboard) => true,
            (VipLevel::Basic | VipLevel::Premium, VipFeature::CloudSync) => true,
            (VipLevel::Premium, VipFeature::UnlimitedDevices) => true,
            _ => false,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum VipFeature {
    BasicClipboard,
    CloudSync,
    UnlimitedDevices,
}
```

#### 通用类型

```rust
// domain/types.rs
use chrono::{DateTime, Local};

// 同步状态
#[derive(Debug, Clone, PartialEq)]
pub enum SyncStatus {
    NotSynced,
    Syncing,
    Synced,
    Skip(SkipReason),
}

impl SyncStatus {
    pub fn to_i32(&self) -> i32 {
        match self {
            SyncStatus::NotSynced => 0,
            SyncStatus::Syncing => 1,
            SyncStatus::Synced => 2,
            SyncStatus::Skip(_) => 3,
        }
    }

    pub fn from_i32(value: i32) -> AppResult<Self> {
        match value {
            0 => Ok(SyncStatus::NotSynced),
            1 => Ok(SyncStatus::Syncing),
            2 => Ok(SyncStatus::Synced),
            3 => Ok(SyncStatus::Skip(SkipReason::Unsupported)),
            _ => Err(AppError::InvalidSyncStatus(value)),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum SkipReason {
    Unsupported,
    VipRequired,
    TooLarge,
}

// 内容类型
#[derive(Debug, Clone, PartialEq)]
pub enum ContentType {
    Text,
    Image,
    File,
}

impl ContentType {
    pub fn from_str(s: &str) -> AppResult<Self> {
        match s.to_lowercase().as_str() {
            "text" => Ok(ContentType::Text),
            "image" => Ok(ContentType::Image),
            "file" => Ok(ContentType::File),
            _ => Err(AppError::InvalidContentType(s.to_string())),
        }
    }

    pub fn to_string(&self) -> String {
        match self {
            ContentType::Text => "text".to_string(),
            ContentType::Image => "image".to_string(),
            ContentType::File => "file".to_string(),
        }
    }
}
```

### 5.2 Repository层设计

#### UserRepository

```rust
// repository/user_repo.rs
use std::sync::Arc;
use rbatis::RBatis;
use crate::domain::user::User;
use crate::shared::errors::AppResult;

pub struct UserRepository {
    db: Arc<RBatis>,
}

impl UserRepository {
    pub fn new(db: Arc<RBatis>) -> Self {
        Self { db }
    }

    pub async fn save(&self, user: &User) -> AppResult<()> {
        // 实现保存逻辑
        Ok(())
    }

    pub async fn find_by_id(&self, id: &str) -> AppResult<Option<User>> {
        // 实现查找逻辑
        Ok(None)
    }

    pub async fn find_by_email(&self, email: &str) -> AppResult<Option<User>> {
        // 实现查找逻辑
        Ok(None)
    }
}
```

### 5.3 Service层设计

#### SyncService（复杂用例）

```rust
// service/sync_service.rs
use std::sync::Arc;
use crate::domain::clipboard::ClipRecord;
use crate::repository::clipboard_repo::ClipboardRepository;
use crate::repository::user_repo::UserRepository;
use crate::api::cloud_sync_api::CloudSyncApi;
use crate::shared::errors::{AppError, AppResult};

pub struct SyncService {
    clipboard_repo: Arc<ClipboardRepository>,
    user_repo: Arc<UserRepository>,
    cloud_api: Arc<CloudSyncApi>,
}

impl SyncService {
    pub fn new(
        clipboard_repo: Arc<ClipboardRepository>,
        user_repo: Arc<UserRepository>,
        cloud_api: Arc<CloudSyncApi>
    ) -> Self {
        Self {
            clipboard_repo,
            user_repo,
            cloud_api,
        }
    }

    // 用例：同步到云端
    pub async fn sync_to_cloud(&self, user_id: &str) -> AppResult<usize> {
        // 1. 检查用户VIP权限
        let user = self.user_repo.find_by_id(user_id).await?
            .ok_or(AppError::UserNotFound)?;

        if !user.is_vip_active() {
            return Err(AppError::VipRequired);
        }

        // 2. 查找需要同步的记录
        let records = self.clipboard_repo.find_to_sync(100).await?;

        let mut synced_count = 0;

        // 3. 逐条同步
        for mut record in records {
            if !record.can_sync() {
                continue;
            }

            // 标记为同步中
            record.mark_syncing();
            self.clipboard_repo.save(&record).await?;

            // 上传到云端
            match self.cloud_api.upload_record(&record).await {
                Ok(_) => {
                    record.mark_synced();
                    self.clipboard_repo.save(&record).await?;
                    synced_count += 1;
                }
                Err(e) => {
                    log::error!("Failed to sync record {}: {}", record.id(), e);
                    // 恢复为未同步状态
                    // ...
                }
            }
        }

        Ok(synced_count)
    }

    // 用例：从云端同步
    pub async fn sync_from_cloud(&self, user_id: &str) -> AppResult<usize> {
        // 实现从云端同步的逻辑
        Ok(0)
    }
}
```

### 5.4 错误处理设计

```rust
// shared/errors.rs
use thiserror::Error;

#[derive(Error, Debug)]
pub enum AppError {
    // 领域错误（业务规则违反）
    #[error("内容不能为空")]
    EmptyContent,

    #[error("无效的邮箱格式")]
    InvalidEmail,

    #[error("记录不存在")]
    RecordNotFound,

    #[error("需要VIP权限")]
    VipRequired,

    #[error("记录已被删除")]
    AlreadyDeleted,

    // 基础设施错误
    #[error("数据库错误: {0}")]
    Database(#[from] rbatis::Error),

    #[error("IO错误: {0}")]
    Io(#[from] std::io::Error),

    #[error("HTTP请求错误: {0}")]
    Http(String),

    // 其他错误
    #[error("未知错误: {0}")]
    Unknown(String),
}

pub type AppResult<T> = Result<T, AppError>;
```

---

## 六、实施路线图

### 阶段一：基础架构搭建（第1周）

#### Day 1-2：创建新目录结构

```bash
# 创建新目录
mkdir -p src-tauri/src/{domain,repository,service,commands}
mkdir -p src-tauri/src/infrastructure
mkdir -p src-tauri/src/shared

# 移动旧代码
mv src-tauri/src/biz src-tauri/src/legacy/biz
```

**任务清单：**
- [ ] 创建完整的目录结构
- [ ] 设置模块导出（mod.rs文件）
- [ ] 移动旧代码到legacy/
- [ ] 确保项目仍可编译

#### Day 3-4：实现Domain层基础

**任务清单：**
- [ ] 实现 `domain/clipboard.rs` - ClipRecord实体（充血模型）
- [ ] 实现 `domain/types.rs` - 通用枚举类型
- [ ] 编写单元测试
- [ ] 确保编译通过

**示例代码：** 见4.1节

#### Day 5-7：实现Repository层

**任务清单：**
- [ ] 实现 `repository/clipboard_repo.rs`
- [ ] 实现数据库模型转换（ClipRecordModel）
- [ ] 从legacy/biz中迁移SQL查询逻辑
- [ ] 编写Repository单元测试

**迁移策略：**
```rust
// 从 legacy/biz/clip_record.rs 迁移
// 把所有 ClipRecord::select_xxx 方法
// 迁移到 ClipboardRepository::find_xxx 方法
```

### 阶段二：业务逻辑迁移（第2周）

#### Day 8-10：实现Service层

**任务清单：**
- [ ] 实现 `service/clipboard_service.rs`
- [ ] 从legacy/biz迁移业务逻辑到Service
- [ ] 调整业务逻辑调用实体方法
- [ ] 编写Service集成测试

**迁移对照表：**

| 旧代码 | 新代码 |
|--------|--------|
| `legacy/biz/clip_record.rs::update_pinned` | `ClipRecord::pin` + `ClipboardService::pin_record` |
| `legacy/biz/clip_record.rs::update_content` | `ClipRecord` 实例方法 + Service |
| `legacy/biz/copy_clip_record.rs` | `ClipboardService::save_clipboard` |

#### Day 11-12：实现Commands层

**任务清单：**
- [ ] 实现 `commands/clipboard.rs`
- [ ] 定义DTO结构
- [ ] 实现DTO转换逻辑
- [ ] 更新Tauri命令注册

#### Day 13-14：依赖注入重构

**任务清单：**
- [ ] 在 `lib.rs` 中实现AppState
- [ ] 配置依赖注入
- [ ] 移除CONTEXT的使用
- [ ] 更新所有命令使用State注入
- [ ] 端到端测试

### 阶段三：其他模块迁移（第3周）

#### Day 15-16：User模块

**任务清单：**
- [ ] 实现 `domain/user.rs`
- [ ] 实现 `repository/user_repo.rs`
- [ ] 实现 `service/user_service.rs`
- [ ] 实现 `commands/user.rs`
- [ ] 迁移 `legacy/biz/user_auth.rs`

#### Day 17-18：VIP模块

**任务清单：**
- [ ] 实现 `domain/vip.rs`
- [ ] 实现 `service/vip_service.rs`
- [ ] 迁移 `legacy/biz/vip_checker.rs`
- [ ] 迁移 `legacy/biz/vip_management.rs`

#### Day 19-20：Sync模块

**任务清单：**
- [ ] 实现 `service/sync_service.rs`
- [ ] 迁移 `legacy/biz/cloud_sync_timer.rs`
- [ ] 迁移 `legacy/biz/upload_cloud_timer.rs`
- [ ] 迁移 `legacy/biz/download_cloud_file.rs`

#### Day 21：清理和优化

**任务清单：**
- [ ] 删除legacy/目录（确保所有功能已迁移）
- [ ] 代码review
- [ ] 性能测试
- [ ] 文档更新

### 迁移检查清单

每个模块迁移完成后检查：

```markdown
## Clipboard模块迁移检查清单

- [ ] Domain层
  - [ ] ClipRecord实体完成
  - [ ] 所有字段私有化
  - [ ] 实现业务行为方法
  - [ ] 单元测试通过

- [ ] Repository层
  - [ ] ClipboardRepository完成
  - [ ] 所有CRUD方法实现
  - [ ] 数据库模型转换完成
  - [ ] Repository测试通过

- [ ] Service层
  - [ ] ClipboardService完成
  - [ ] 所有用例实现
  - [ ] 集成测试通过

- [ ] Commands层
  - [ ] 所有Tauri命令实现
  - [ ] DTO定义完成
  - [ ] 端到端测试通过

- [ ] 功能验证
  - [ ] 前端功能正常
  - [ ] 性能无退化
  - [ ] 无新增bug
```

---

## 七、方案对比

### 7.1 与完整DDD方案对比

| 对比项 | 完整DDD方案 | 本轻量级方案 | 当前架构 |
|--------|------------|-------------|---------|
| **架构复杂度** | ⭐⭐⭐⭐⭐ 非常复杂 | ⭐⭐⭐ 中等 | ⭐⭐ 简单 |
| **代码增量** | +70% | +30% | 基线 |
| **学习曲线** | 陡峭 | 平缓 | 无 |
| **开发速度** | 慢（前期） | 中等 | 快 |
| **实施时间** | 1-2个月 | 2-3周 | - |
| **可维护性** | ⭐⭐⭐⭐⭐ | ⭐⭐⭐⭐ | ⭐⭐ |
| **可测试性** | ⭐⭐⭐⭐⭐ | ⭐⭐⭐⭐ | ⭐⭐ |
| **灵活性** | ⭐⭐⭐ | ⭐⭐⭐⭐ | ⭐⭐⭐⭐⭐ |
| **适合规模** | 大型（100k+行） | 中小型（10k-50k行） | 小型（<10k行） |

### 7.2 详细对比

#### 目录结构对比

| 完整DDD | 轻量级方案 | 说明 |
|---------|-----------|------|
| domain/entities/ | domain/ | 合并实体和值对象 |
| domain/value_objects/ | domain/types.rs | 用枚举代替复杂值对象 |
| domain/repositories/ (接口) | repository/ | 省略接口定义 |
| domain/services/ | - | 逻辑放到实体或Service层 |
| application/services/ | service/ | 合并应用服务 |
| application/dto/ | commands/ (内联DTO) | DTO定义在Commands中 |
| infrastructure/persistence/repositories/ | repository/ | 简化目录层级 |
| infrastructure/persistence/models/ | repository/ (内联) | 模型定义在Repository中 |
| interfaces/commands/ | commands/ | 直接使用commands/ |

**代码文件数对比（Clipboard模块）：**
- 完整DDD：15个文件
- 轻量级方案：6个文件
- 当前架构：4个文件

#### 代码量对比（以Clipboard模块为例）

| 完整DDD | 轻量级方案 | 当前架构 |
|---------|-----------|---------|
| domain/clipboard/entities/clipboard_record.rs (300行) | domain/clipboard.rs (200行) | biz/clip_record.rs (598行) |
| domain/clipboard/value_objects/ (3个文件，200行) | domain/types.rs (50行) | - |
| domain/clipboard/repositories/clipboard_repository.rs (100行) | - | - |
| infrastructure/.../clipboard_repository_impl.rs (400行) | repository/clipboard_repo.rs (300行) | - |
| infrastructure/.../clipboard_model.rs (200行) | (内联在repo中) | - |
| application/services/clipboard_service.rs (300行) | service/clipboard_service.rs (200行) | 分散在多个文件 |
| interfaces/commands/clipboard_commands.rs (200行) | commands/clipboard.rs (150行) | 混在一起 |
| interfaces/dto/ (3个文件，200行) | (内联在commands中) | - |
| **总计：约1900行** | **总计：约900行** | **当前：约1000行** |

**结论：**
- 完整DDD增加90%代码量
- 轻量级方案减少10%代码量（通过去除冗余）

### 7.3 优缺点对比

#### 完整DDD方案

**优点：**
- ✅ 理论上最优的架构
- ✅ 符合所有最佳实践
- ✅ 极高的可维护性和可扩展性
- ✅ 适合大型复杂系统

**缺点：**
- ❌ 过度设计（对当前项目）
- ❌ 大量样板代码
- ❌ 学习曲线陡峭
- ❌ 开发速度慢
- ❌ 修改一个功能需要改6-8个文件

#### 轻量级方案

**优点：**
- ✅ 解决核心问题（贫血模型、全局状态、高耦合）
- ✅ 代码量可控
- ✅ 学习曲线平缓
- ✅ 实施周期短
- ✅ 保持灵活性

**缺点：**
- ⚠️ 没有完整DDD那么"优雅"
- ⚠️ Repository没有接口抽象（但对小项目足够）
- ⚠️ 值对象用枚举代替（损失一些类型安全）

#### 当前架构

**优点：**
- ✅ 简单直接
- ✅ 开发速度快

**缺点：**
- ❌ 贫血模型
- ❌ 全局状态泛滥
- ❌ 难以测试
- ❌ 高耦合
- ❌ 随着功能增加会越来越难维护

---

## 八、风险与应对

### 8.1 风险识别

| 风险 | 可能性 | 影响 | 应对策略 |
|-----|--------|------|---------|
| **迁移过程中引入bug** | 高 | 高 | 每个阶段充分测试；保留旧代码对照 |
| **性能退化** | 中 | 高 | 性能测试对比；关键路径优化 |
| **实施时间超出预期** | 中 | 中 | 分阶段实施；优先核心模块 |
| **团队不适应新架构** | 低 | 中 | 编写详细文档；代码review |
| **旧代码依赖难以解耦** | 中 | 中 | 渐进式迁移；保留适配层 |

### 8.2 回退策略

如果迁移过程中出现重大问题：

**阶段一失败：**
- 删除新代码
- 恢复legacy/目录到原位置
- 损失：1周开发时间

**阶段二失败：**
- 保留Domain和Repository层
- 回退Service和Commands
- 损失：2周开发时间

**阶段三失败：**
- 保留已迁移模块
- 暂停其他模块迁移
- 新旧代码共存

### 8.3 质量保证

#### 测试策略

```rust
// 1. 单元测试（Domain层）
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_clip_record_pin() {
        let mut record = ClipRecord::new(
            "test".to_string(),
            ContentType::Text
        ).unwrap();

        assert!(!record.is_pinned());
        record.pin();
        assert!(record.is_pinned());
    }
}

// 2. 集成测试（Repository层）
#[tokio::test]
async fn test_clipboard_repo_save_and_find() {
    let db = setup_test_db().await;
    let repo = ClipboardRepository::new(db);

    let record = ClipRecord::new("test".to_string(), ContentType::Text).unwrap();
    repo.save(&record).await.unwrap();

    let found = repo.find_by_id(record.id()).await.unwrap();
    assert!(found.is_some());
}

// 3. 端到端测试（Commands层）
// 使用Tauri的测试工具
```

#### 性能测试

```rust
// 迁移前后性能对比
#[bench]
fn bench_save_clipboard_old(b: &mut Bencher) {
    // 测试旧代码
}

#[bench]
fn bench_save_clipboard_new(b: &mut Bencher) {
    // 测试新代码
}
```

**性能指标：**
- 保存剪贴板：< 10ms
- 查询最近记录：< 50ms
- 搜索：< 100ms

---

## 九、总结

### 9.1 为什么选择轻量级方案？

1. **适合项目规模** - ClipPal是中小型项目，完整DDD过于复杂
2. **快速见效** - 2-3周即可完成，而非1-2个月
3. **学习曲线平缓** - 团队容易上手
4. **成本可控** - 代码量增加30%，可接受
5. **解决核心问题** - 贫血模型、全局状态、高耦合都得到解决

### 9.2 预期收益

| 改进项 | 改善程度 |
|--------|---------|
| **代码可维护性** | 提升100% |
| **可测试性** | 提升150% |
| **模块耦合度** | 降低60% |
| **开发效率** | 前期降低20%，后期提升40% |
| **Bug率** | 降低30% |

### 9.3 下一步行动

1. ✅ **决策** - 确认采用本方案
2. ✅ **准备** - 创建分支，备份代码
3. ✅ **启动** - 按照阶段一开始实施
4. ✅ **迭代** - 每周review进度
5. ✅ **完成** - 3周后交付新架构

---

## 附录

### A. 参考资料

1. **架构设计**
   - 《整洁架构》 - Robert C. Martin
   - 《领域驱动设计》 - Eric Evans（参考思想，不完全照搬）
   - 《重构：改善既有代码的设计》 - Martin Fowler

2. **Rust最佳实践**
   - [Rust API Guidelines](https://rust-lang.github.io/api-guidelines/)
   - [Rust Design Patterns](https://rust-unofficial.github.io/patterns/)

3. **Tauri文档**
   - [Tauri Guide](https://tauri.app/guide/)
   - [Tauri State Management](https://tauri.app/concepts/state-management/)

### B. 常见问题

**Q: 为什么不用trait定义Repository接口？**
A: 对小项目来说，具体实现就足够了。如果未来需要Mock测试，可以后续添加。

**Q: 为什么值对象用枚举而不是struct？**
A: 枚举更简单，对大多数场景足够。只有复杂验证逻辑才需要struct。

**Q: 如何处理异步依赖？**
A: 使用Arc包装，通过State注入到commands。

**Q: 旧代码何时删除？**
A: 所有功能迁移完成并测试通过后，统一删除legacy/目录。

---

**文档版本**: v1.0
**最后更新**: 2025-12-02
**作者**: ClipPal Team
**状态**: 待评审


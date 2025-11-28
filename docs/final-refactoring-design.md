# ClipPal DDD 架构重构最终设计方案

## 目录
- [一、设计理念](#一设计理念)
- [二、架构概览](#二架构概览)
- [三、完整目录结构](#三完整目录结构)
- [四、各层详细设计](#四各层详细设计)
- [五、核心模块示例](#五核心模块示例)
- [六、统一错误处理与响应包装](#六统一错误处理与响应包装)
- [七、实施指南](#七实施指南)
- [八、最佳实践](#八最佳实践)
- [九、总结](#九总结)

---

## 一、设计理念

### 1.1 核心思想

本重构方案基于 **领域驱动设计（DDD）** 原则，但不照搬传统 Spring Boot 的贫血模型，而是采用 **富领域模型**：

| 传统三层架构 | 本方案（DDD） |
|-------------|--------------|
| ❌ Entity 只是数据容器 | ✅ Entity 包含业务逻辑和行为 |
| ❌ 所有逻辑在 Service 层 | ✅ 业务规则在领域对象中 |
| ❌ 技术与业务混在一起 | ✅ 清晰的依赖倒置 |
| ❌ 数据库模型 = 领域模型 | ✅ 领域模型与持久化分离 |

### 1.2 设计原则

1. **单一职责原则（SRP）**：每层、每模块只做一件事
2. **依赖倒置原则（DIP）**：Domain 层不依赖任何外层
3. **开闭原则（OCP）**：对扩展开放，对修改关闭
4. **领域驱动**：业务逻辑集中在 Domain 层
5. **渐进式重构**：保持系统可运行，逐步迁移

### 1.3 为什么不是 Spring Boot？

Spring Boot 框架本身是优秀的，但很多 Spring Boot 项目存在以下问题：

```rust
// ❌ 贫血模型（Anemic Model）- 很多 Spring Boot 项目的做法
pub struct User {
    pub id: String,
    pub email: String,
    pub password: String,
}

pub struct UserService {
    pub fn register(&self, email: String, password: String) -> Result<User> {
        // 所有业务逻辑都在 Service 层
        if email.is_empty() {
            return Err("邮箱不能为空");
        }
        if !email.contains('@') {
            return Err("邮箱格式错误");
        }
        if password.len() < 6 {
            return Err("密码太短");
        }
        // ... 更多业务逻辑
    }
}
```

```rust
// ✅ 富领域模型（Rich Domain Model）- 本方案的做法
pub struct User {
    id: UserId,
    email: Email,  // 值对象，自带验证
    password: Password,  // 值对象，自带加密
}

impl User {
    // 业务逻辑在实体中
    pub fn register(email: Email, password: Password) -> Result<Self> {
        // Email 和 Password 的值对象已经保证了有效性
        Ok(Self {
            id: UserId::new(),
            email,
            password,
        })
    }

    pub fn change_password(&mut self, old: &str, new: Password) -> Result<()> {
        if !self.password.verify(old) {
            return Err(DomainError::PasswordMismatch);
        }
        self.password = new;
        Ok(())
    }
}

// 值对象：自带验证逻辑
pub struct Email(String);

impl Email {
    pub fn new(value: String) -> Result<Self> {
        if !value.contains('@') {
            return Err(DomainError::InvalidEmail);
        }
        Ok(Self(value))
    }
}
```

**关键区别**：
- 贫血模型：实体是 **数据容器**，业务逻辑在 Service
- 富领域模型：实体是 **业务对象**，包含状态和行为

---

## 二、架构概览

### 2.1 分层架构图

```
┌─────────────────────────────────────────────────────────────┐
│                      Platform 层                             │
│  (Tauri 系统配置：窗口、托盘、菜单、系统事件)                    │
└─────────────────────────────────────────────────────────────┘
                              ↓
┌─────────────────────────────────────────────────────────────┐
│                    Interfaces 层                             │
│  (前端交互接口：Tauri Commands、DTO 转换)                       │
└─────────────────────────────────────────────────────────────┘
                              ↓
┌─────────────────────────────────────────────────────────────┐
│                    Application 层                            │
│  (应用服务：用例编排、事务协调、跨领域协调)                       │
└─────────────────────────────────────────────────────────────┘
                              ↓
┌─────────────────────────────────────────────────────────────┐
│                      Domain 层                               │
│  (核心业务逻辑：实体、值对象、领域服务、仓储接口)                  │
│  - Clipboard Domain  - User Domain                           │
│  - VIP Domain        - Sync Domain                           │
└─────────────────────────────────────────────────────────────┘
                              ↑
┌─────────────────────────────────────────────────────────────┐
│                  Infrastructure 层                           │
│  (技术实现：数据库、HTTP 客户端、文件系统、加密等)                │
└─────────────────────────────────────────────────────────────┘
                              ↑
┌─────────────────────────────────────────────────────────────┐
│                      Shared 层                               │
│  (共享内核：全局上下文、错误类型、通用类型)                       │
└─────────────────────────────────────────────────────────────┘
```

### 2.2 依赖规则

```
Platform → Interfaces → Application → Domain ← Infrastructure
                                        ↑
                                      Shared
```

**关键点**：
- Domain 层不依赖任何外层（依赖倒置）
- Infrastructure 层实现 Domain 层定义的接口
- 所有层都可以依赖 Shared 层

---

## 三、完整目录结构

```
src-tauri/src/
├── main.rs                          # 程序入口
├── lib.rs                           # 库入口，依赖注入配置
│
├── platform/                        # 平台层：Tauri 系统级配置
│   ├── mod.rs
│   ├── window.rs                    # 窗口管理（位置、大小、事件）
│   ├── tray.rs                      # 托盘图标和菜单
│   ├── menu.rs                      # 应用菜单栏
│   └── shortcuts.rs                 # 全局快捷键
│
├── interfaces/                      # 接口层：前端交互
│   ├── mod.rs
│   ├── commands/                    # Tauri Commands（类似 Controller）
│   │   ├── mod.rs
│   │   ├── clipboard_commands.rs    # 剪贴板相关命令
│   │   ├── user_commands.rs         # 用户相关命令
│   │   ├── sync_commands.rs         # 同步相关命令
│   │   ├── vip_commands.rs          # VIP 相关命令
│   │   └── settings_commands.rs     # 设置相关命令
│   └── dto/                         # 数据传输对象
│       ├── mod.rs
│       ├── request.rs               # 请求 DTO
│       ├── response.rs              # 响应 DTO
│       └── mapper.rs                # Domain Entity ↔ DTO 转换
│
├── application/                     # 应用层：用例编排
│   ├── mod.rs
│   └── services/                    # 应用服务
│       ├── mod.rs
│       ├── clipboard_service.rs     # 剪贴板应用服务
│       ├── search_service.rs        # 搜索应用服务
│       ├── sync_service.rs          # 同步应用服务
│       ├── user_service.rs          # 用户应用服务
│       └── vip_service.rs           # VIP 应用服务
│
├── domain/                          # 领域层：核心业务逻辑
│   ├── mod.rs
│   │
│   ├── clipboard/                   # 剪贴板领域
│   │   ├── mod.rs
│   │   ├── entities/                # 实体（有身份标识）
│   │   │   ├── mod.rs
│   │   │   └── clipboard_record.rs  # 剪贴板记录实体
│   │   ├── value_objects/           # 值对象（无身份标识）
│   │   │   ├── mod.rs
│   │   │   ├── content_type.rs      # 内容类型（Text/Image/File）
│   │   │   ├── record_id.rs         # 记录 ID
│   │   │   └── content.rs           # 内容值对象
│   │   ├── repositories/            # 仓储接口（不是实现！）
│   │   │   ├── mod.rs
│   │   │   └── clipboard_repository.rs
│   │   └── services/                # 领域服务
│   │       ├── mod.rs
│   │       └── duplicate_detector.rs # 去重检测服务
│   │
│   ├── user/                        # 用户领域
│   │   ├── mod.rs
│   │   ├── entities/
│   │   │   ├── mod.rs
│   │   │   ├── user.rs              # 用户实体
│   │   │   └── user_profile.rs      # 用户资料实体
│   │   ├── value_objects/
│   │   │   ├── mod.rs
│   │   │   ├── email.rs             # 邮箱（带验证）
│   │   │   ├── password.rs          # 密码（带加密）
│   │   │   └── user_id.rs           # 用户 ID
│   │   └── repositories/
│   │       ├── mod.rs
│   │       └── user_repository.rs   # 用户仓储接口
│   │
│   ├── vip/                         # VIP 领域
│   │   ├── mod.rs
│   │   ├── entities/
│   │   │   ├── mod.rs
│   │   │   └── membership.rs        # 会员资格实体
│   │   ├── value_objects/
│   │   │   ├── mod.rs
│   │   │   ├── membership_level.rs  # 会员等级（Free/VIP/SVIP）
│   │   │   ├── privilege.rs         # 权益枚举
│   │   │   └── expiry_date.rs       # 到期日期
│   │   └── services/
│   │       ├── mod.rs
│   │       └── privilege_checker.rs # 权益检查服务
│   │
│   └── sync/                        # 同步领域
│       ├── mod.rs
│       ├── entities/
│       │   ├── mod.rs
│       │   └── sync_task.rs         # 同步任务实体
│       └── value_objects/
│           ├── mod.rs
│           ├── sync_status.rs       # 同步状态（Pending/Syncing/Success/Failed）
│           └── sync_direction.rs    # 同步方向（Upload/Download/Both）
│
├── infrastructure/                  # 基础设施层：技术实现
│   ├── mod.rs
│   │
│   ├── persistence/                 # 数据持久化
│   │   ├── mod.rs
│   │   ├── database.rs              # RBatis 数据库连接配置
│   │   ├── repositories/            # 仓储实现
│   │   │   ├── mod.rs
│   │   │   ├── clipboard_repository_impl.rs
│   │   │   └── user_repository_impl.rs
│   │   └── models/                  # 数据库模型（与领域实体分离）
│   │       ├── mod.rs
│   │       ├── clipboard_model.rs   # 对应 clipboard_records 表
│   │       └── user_model.rs        # 对应 users 表
│   │
│   ├── api/                         # 外部 API 客户端
│   │   ├── mod.rs
│   │   ├── client.rs                # HTTP 客户端配置（reqwest）
│   │   ├── auth_api.rs              # 认证 API
│   │   ├── sync_api.rs              # 同步 API
│   │   └── vip_api.rs               # VIP API
│   │
│   └── utils/                       # 技术工具（非业务）
│       ├── mod.rs
│       ├── logger.rs                # 日志配置
│       ├── config.rs                # 配置文件读取
│       └── crypto.rs                # 加密解密
│
├── shared/                          # 共享内核
│   ├── mod.rs
│   ├── context.rs                   # 全局上下文（State 管理）
│   ├── errors.rs                    # 统一错误类型
│   ├── events.rs                    # 领域事件定义
│   └── types.rs                     # 通用类型（Result、DateTime 等）
│
└── legacy/                          # 遗留代码（待迁移）
    ├── mod.rs
    ├── biz/                         # 旧业务逻辑
    ├── dao/                         # 旧数据访问
    └── utils/                       # 旧工具类
```

---

## 四、各层详细设计

### 4.1 Platform 层（平台层）

**职责**：处理 Tauri 系统级功能，与操作系统交互

**特点**：
- 不包含业务逻辑
- 与 Tauri API 直接交互
- 跨平台适配（macOS/Windows）

**示例**：`platform/window.rs`
```rust
use tauri::{App, Manager, WindowEvent};

/// 初始化主窗口
pub fn init_main_window(app: &App) -> tauri::Result<()> {
    let main_window = app.get_webview_window("main")?;

    // 获取屏幕信息
    let monitor = main_window.primary_monitor()?.unwrap();
    let screen_size = monitor.size();

    // 计算窗口位置和大小
    let window_width = calculate_optimal_width(screen_size.width);
    let window_height = screen_size.height;

    // 设置窗口
    main_window.set_size(PhysicalSize::new(window_width, window_height))?;
    main_window.set_position(PhysicalPosition::new(x, y))?;

    // macOS 特定配置
    #[cfg(target_os = "macos")]
    main_window.set_always_on_top(true)?;

    Ok(())
}
```

### 4.2 Interfaces 层（接口层）

**职责**：前端与后端的桥梁，处理数据转换

**特点**：
- 接收前端请求（Tauri Commands）
- 调用 Application 层服务
- 将 Domain Entity 转换为 DTO 返回前端

**示例**：`interfaces/commands/clipboard_commands.rs`
```rust
use tauri::State;
use crate::application::services::ClipboardService;
use crate::interfaces::dto::{ClipboardRecordDto, SaveClipboardRequest};

/// 保存剪贴板内容
#[tauri::command]
pub async fn save_clipboard(
    request: SaveClipboardRequest,
    state: State<'_, AppState>
) -> Result<ClipboardRecordDto, String> {
    // 1. 调用应用服务
    let record = state.clipboard_service
        .save_clipboard_content(request.content, request.content_type)
        .await
        .map_err(|e| e.to_string())?;

    // 2. 领域实体转换为 DTO
    let dto = ClipboardRecordDto::from(record);

    Ok(dto)
}

/// 获取最近的剪贴板记录
#[tauri::command]
pub async fn get_recent_clipboards(
    limit: usize,
    state: State<'_, AppState>
) -> Result<Vec<ClipboardRecordDto>, String> {
    let records = state.clipboard_service
        .get_recent_records(limit)
        .await
        .map_err(|e| e.to_string())?;

    // 批量转换
    let dtos = records.into_iter()
        .map(ClipboardRecordDto::from)
        .collect();

    Ok(dtos)
}
```

**DTO 定义**：`interfaces/dto/response.rs`
```rust
use serde::Serialize;

/// 前端响应 DTO
#[derive(Serialize)]
pub struct ClipboardRecordDto {
    pub id: String,
    pub content: String,
    pub content_type: String,
    pub created_at: String,
    pub is_favorite: bool,
}

impl From<ClipboardRecord> for ClipboardRecordDto {
    fn from(record: ClipboardRecord) -> Self {
        Self {
            id: record.id().to_string(),
            content: record.content().clone(),
            content_type: record.content_type().to_string(),
            created_at: record.created_at().to_rfc3339(),
            is_favorite: record.is_favorite(),
        }
    }
}
```

### 4.3 Application 层（应用层）

**职责**：用例编排，协调多个领域对象

**特点**：
- 无业务逻辑（业务逻辑在 Domain 层）
- 协调多个领域对象和仓储
- 管理事务边界

**示例**：`application/services/clipboard_service.rs`
```rust
use std::sync::Arc;
use crate::domain::clipboard::{
    entities::ClipboardRecord,
    value_objects::ContentType,
    repositories::ClipboardRepository,
    services::DuplicateDetector,
};

/// 剪贴板应用服务
pub struct ClipboardService {
    repository: Arc<dyn ClipboardRepository>,
    duplicate_detector: Arc<DuplicateDetector>,
}

impl ClipboardService {
    pub fn new(
        repository: Arc<dyn ClipboardRepository>,
        duplicate_detector: Arc<DuplicateDetector>
    ) -> Self {
        Self { repository, duplicate_detector }
    }

    /// 保存剪贴板内容（用例编排）
    pub async fn save_clipboard_content(
        &self,
        content: String,
        content_type_str: String
    ) -> Result<ClipboardRecord> {
        // 1. 创建值对象（会自动验证）
        let content_type = ContentType::from_str(&content_type_str)?;

        // 2. 检测是否重复（领域服务）
        if self.duplicate_detector.is_duplicate(&content).await? {
            return Err(AppError::DuplicateContent);
        }

        // 3. 创建领域实体（业务逻辑在实体内）
        let record = ClipboardRecord::new(content, content_type)?;

        // 4. 持久化
        self.repository.save(&record).await?;

        // 5. 发布领域事件（可选）
        // event_bus.publish(ClipboardSavedEvent::new(record.id()));

        Ok(record)
    }

    /// 获取最近记录
    pub async fn get_recent_records(&self, limit: usize) -> Result<Vec<ClipboardRecord>> {
        self.repository.find_recent(limit).await
    }

    /// 标记为收藏
    pub async fn mark_as_favorite(&self, id: &str) -> Result<()> {
        // 1. 从仓储加载实体
        let mut record = self.repository
            .find_by_id(id)
            .await?
            .ok_or(AppError::RecordNotFound)?;

        // 2. 调用实体的业务方法
        record.mark_favorite();

        // 3. 保存更新
        self.repository.save(&record).await?;

        Ok(())
    }
}
```

### 4.4 Domain 层（领域层）

**职责**：核心业务逻辑，系统的心脏

**特点**：
- 包含所有业务规则
- 不依赖外层（纯 Rust，无外部依赖）
- 实体拥有行为（富领域模型）

#### 4.4.1 实体（Entity）

**定义**：有唯一标识的业务对象

**示例**：`domain/clipboard/entities/clipboard_record.rs`
```rust
use chrono::{DateTime, Local};
use crate::domain::clipboard::value_objects::{ContentType, RecordId, Content};
use crate::shared::errors::DomainError;

/// 剪贴板记录实体
#[derive(Debug, Clone)]
pub struct ClipboardRecord {
    id: RecordId,
    content: Content,
    content_type: ContentType,
    created_at: DateTime<Local>,
    updated_at: DateTime<Local>,
    is_favorite: bool,
    is_deleted: bool,
}

impl ClipboardRecord {
    /// 创建新记录（工厂方法）
    pub fn new(content: String, content_type: ContentType) -> Result<Self, DomainError> {
        let content = Content::new(content)?;
        let now = Local::now();

        Ok(Self {
            id: RecordId::generate(),
            content,
            content_type,
            created_at: now,
            updated_at: now,
            is_favorite: false,
            is_deleted: false,
        })
    }

    /// 从仓储重建（不经过验证）
    pub fn from_repository(
        id: RecordId,
        content: Content,
        content_type: ContentType,
        created_at: DateTime<Local>,
        updated_at: DateTime<Local>,
        is_favorite: bool,
        is_deleted: bool,
    ) -> Self {
        Self {
            id,
            content,
            content_type,
            created_at,
            updated_at,
            is_favorite,
            is_deleted,
        }
    }

    // === 业务行为（这是关键！）===

    /// 标记为收藏
    pub fn mark_favorite(&mut self) {
        if !self.is_favorite {
            self.is_favorite = true;
            self.updated_at = Local::now();
        }
    }

    /// 取消收藏
    pub fn unmark_favorite(&mut self) {
        if self.is_favorite {
            self.is_favorite = false;
            self.updated_at = Local::now();
        }
    }

    /// 软删除
    pub fn soft_delete(&mut self) -> Result<(), DomainError> {
        if self.is_deleted {
            return Err(DomainError::AlreadyDeleted);
        }
        self.is_deleted = true;
        self.updated_at = Local::now();
        Ok(())
    }

    /// 是否可以被同步
    pub fn can_sync(&self) -> bool {
        !self.is_deleted && self.content.is_valid_for_sync()
    }

    // === Getters（只读访问）===

    pub fn id(&self) -> &RecordId { &self.id }
    pub fn content(&self) -> &Content { &self.content }
    pub fn content_type(&self) -> &ContentType { &self.content_type }
    pub fn created_at(&self) -> DateTime<Local> { self.created_at }
    pub fn is_favorite(&self) -> bool { self.is_favorite }
    pub fn is_deleted(&self) -> bool { self.is_deleted }
}
```

#### 4.4.2 值对象（Value Object）

**定义**：无唯一标识，由属性值决定相等性

**示例1**：`domain/clipboard/value_objects/content_type.rs`
```rust
use serde::{Serialize, Deserialize};
use crate::shared::errors::DomainError;

/// 内容类型值对象
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ContentType {
    Text,
    Image,
    File,
    Url,
}

impl ContentType {
    /// 从字符串解析（带验证）
    pub fn from_str(s: &str) -> Result<Self, DomainError> {
        match s.to_lowercase().as_str() {
            "text" => Ok(Self::Text),
            "image" => Ok(Self::Image),
            "file" => Ok(Self::File),
            "url" => Ok(Self::Url),
            _ => Err(DomainError::InvalidContentType(s.to_string())),
        }
    }

    /// 转换为字符串
    pub fn as_str(&self) -> &str {
        match self {
            Self::Text => "text",
            Self::Image => "image",
            Self::File => "file",
            Self::Url => "url",
        }
    }

    /// 是否支持搜索
    pub fn is_searchable(&self) -> bool {
        matches!(self, Self::Text | Self::Url)
    }
}

impl std::fmt::Display for ContentType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}
```

**示例2**：`domain/user/value_objects/email.rs`
```rust
use crate::shared::errors::DomainError;

/// 邮箱值对象（自带验证）
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Email(String);

impl Email {
    /// 创建邮箱（带验证）
    pub fn new(value: String) -> Result<Self, DomainError> {
        Self::validate(&value)?;
        Ok(Self(value))
    }

    /// 验证邮箱格式
    fn validate(value: &str) -> Result<(), DomainError> {
        if value.is_empty() {
            return Err(DomainError::EmptyEmail);
        }

        if !value.contains('@') {
            return Err(DomainError::InvalidEmailFormat);
        }

        let parts: Vec<&str> = value.split('@').collect();
        if parts.len() != 2 || parts[0].is_empty() || parts[1].is_empty() {
            return Err(DomainError::InvalidEmailFormat);
        }

        Ok(())
    }

    /// 获取值
    pub fn value(&self) -> &str {
        &self.0
    }

    /// 获取域名
    pub fn domain(&self) -> &str {
        self.0.split('@').nth(1).unwrap()
    }
}

impl std::fmt::Display for Email {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}
```

#### 4.4.3 仓储接口（Repository）

**定义**：Domain 层定义接口，Infrastructure 层实现

**示例**：`domain/clipboard/repositories/clipboard_repository.rs`
```rust
use async_trait::async_trait;
use crate::domain::clipboard::entities::ClipboardRecord;
use crate::domain::clipboard::value_objects::RecordId;
use crate::shared::errors::Result;

/// 剪贴板仓储接口（Domain 层定义）
#[async_trait]
pub trait ClipboardRepository: Send + Sync {
    /// 保存记录（新增或更新）
    async fn save(&self, record: &ClipboardRecord) -> Result<()>;

    /// 根据 ID 查找
    async fn find_by_id(&self, id: &RecordId) -> Result<Option<ClipboardRecord>>;

    /// 获取最近的记录
    async fn find_recent(&self, limit: usize) -> Result<Vec<ClipboardRecord>>;

    /// 搜索记录
    async fn search(&self, keyword: &str, limit: usize) -> Result<Vec<ClipboardRecord>>;

    /// 获取收藏列表
    async fn find_favorites(&self) -> Result<Vec<ClipboardRecord>>;

    /// 删除记录
    async fn delete(&self, id: &RecordId) -> Result<()>;

    /// 检查内容是否存在
    async fn exists_by_content(&self, content: &str) -> Result<bool>;
}
```

#### 4.4.4 领域服务（Domain Service）

**定义**：无法归属到单个实体的业务逻辑

**示例**：`domain/clipboard/services/duplicate_detector.rs`
```rust
use std::sync::Arc;
use crate::domain::clipboard::repositories::ClipboardRepository;
use crate::shared::errors::Result;

/// 去重检测领域服务
pub struct DuplicateDetector {
    repository: Arc<dyn ClipboardRepository>,
}

impl DuplicateDetector {
    pub fn new(repository: Arc<dyn ClipboardRepository>) -> Self {
        Self { repository }
    }

    /// 检测内容是否重复
    pub async fn is_duplicate(&self, content: &str) -> Result<bool> {
        self.repository.exists_by_content(content).await
    }

    /// 智能去重（相似度检测）
    pub async fn is_similar(&self, content: &str, threshold: f64) -> Result<bool> {
        // 获取最近的记录
        let recent_records = self.repository.find_recent(10).await?;

        // 计算相似度
        for record in recent_records {
            let similarity = self.calculate_similarity(
                content,
                record.content().value()
            );

            if similarity >= threshold {
                return Ok(true);
            }
        }

        Ok(false)
    }

    /// 计算两个字符串的相似度（简化版）
    fn calculate_similarity(&self, s1: &str, s2: &str) -> f64 {
        // 使用编辑距离或其他算法
        // 这里简化为长度比较
        let len1 = s1.len() as f64;
        let len2 = s2.len() as f64;
        1.0 - ((len1 - len2).abs() / len1.max(len2))
    }
}
```

### 4.5 Infrastructure 层（基础设施层）

**职责**：技术实现，实现 Domain 层定义的接口

#### 4.5.1 仓储实现

**示例**：`infrastructure/persistence/repositories/clipboard_repository_impl.rs`
```rust
use std::sync::Arc;
use async_trait::async_trait;
use rbatis::RBatis;
use crate::domain::clipboard::{
    entities::ClipboardRecord,
    repositories::ClipboardRepository,
    value_objects::RecordId,
};
use crate::infrastructure::persistence::models::ClipboardModel;
use crate::shared::errors::Result;

/// 剪贴板仓储实现（Infrastructure 层）
pub struct ClipboardRepositoryImpl {
    db: Arc<RBatis>,
}

impl ClipboardRepositoryImpl {
    pub fn new(db: Arc<RBatis>) -> Self {
        Self { db }
    }
}

#[async_trait]
impl ClipboardRepository for ClipboardRepositoryImpl {
    async fn save(&self, record: &ClipboardRecord) -> Result<()> {
        // 领域实体 -> 数据库模型
        let model = ClipboardModel::from(record);

        // 插入或更新
        let sql = "INSERT OR REPLACE INTO clipboard_records
                   (id, content, content_type, created_at, updated_at, is_favorite, is_deleted)
                   VALUES (?, ?, ?, ?, ?, ?, ?)";

        self.db.exec(sql, vec![
            model.id,
            model.content,
            model.content_type,
            model.created_at,
            model.updated_at,
            model.is_favorite,
            model.is_deleted,
        ]).await?;

        Ok(())
    }

    async fn find_by_id(&self, id: &RecordId) -> Result<Option<ClipboardRecord>> {
        let sql = "SELECT * FROM clipboard_records WHERE id = ? AND is_deleted = 0";
        let model: Option<ClipboardModel> = self.db
            .fetch_by_column(sql, &[id.to_string()])
            .await?;

        // 数据库模型 -> 领域实体
        match model {
            Some(m) => Ok(Some(m.try_into()?)),
            None => Ok(None),
        }
    }

    async fn find_recent(&self, limit: usize) -> Result<Vec<ClipboardRecord>> {
        let sql = "SELECT * FROM clipboard_records
                   WHERE is_deleted = 0
                   ORDER BY created_at DESC
                   LIMIT ?";

        let models: Vec<ClipboardModel> = self.db
            .fetch(sql, vec![limit])
            .await?;

        // 批量转换
        models.into_iter()
            .map(|m| m.try_into())
            .collect()
    }

    async fn exists_by_content(&self, content: &str) -> Result<bool> {
        let sql = "SELECT COUNT(*) FROM clipboard_records
                   WHERE content = ? AND is_deleted = 0";

        let count: i64 = self.db
            .fetch_page(sql, vec![content], &PageRequest::new(1, 1))
            .await?
            .total;

        Ok(count > 0)
    }

    // ... 其他方法实现
}
```

#### 4.5.2 数据库模型

**示例**：`infrastructure/persistence/models/clipboard_model.rs`
```rust
use rbatis::crud_table;
use serde::{Serialize, Deserialize};
use crate::domain::clipboard::entities::ClipboardRecord;
use crate::domain::clipboard::value_objects::{ContentType, RecordId, Content};
use crate::shared::errors::{Result, DomainError};

/// 数据库模型（与领域实体分离）
#[crud_table(table_name: "clipboard_records")]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClipboardModel {
    pub id: String,
    pub content: String,
    pub content_type: String,
    pub created_at: String,
    pub updated_at: String,
    pub is_favorite: i32,
    pub is_deleted: i32,
}

// 领域实体 -> 数据库模型
impl From<&ClipboardRecord> for ClipboardModel {
    fn from(record: &ClipboardRecord) -> Self {
        Self {
            id: record.id().to_string(),
            content: record.content().value().to_string(),
            content_type: record.content_type().as_str().to_string(),
            created_at: record.created_at().to_rfc3339(),
            updated_at: record.updated_at().to_rfc3339(),
            is_favorite: if record.is_favorite() { 1 } else { 0 },
            is_deleted: if record.is_deleted() { 1 } else { 0 },
        }
    }
}

// 数据库模型 -> 领域实体
impl TryFrom<ClipboardModel> for ClipboardRecord {
    type Error = DomainError;

    fn try_from(model: ClipboardModel) -> Result<Self, Self::Error> {
        let id = RecordId::from_str(&model.id)?;
        let content = Content::new(model.content)?;
        let content_type = ContentType::from_str(&model.content_type)?;
        let created_at = DateTime::parse_from_rfc3339(&model.created_at)
            .map_err(|_| DomainError::InvalidDateFormat)?
            .with_timezone(&Local);
        let updated_at = DateTime::parse_from_rfc3339(&model.updated_at)
            .map_err(|_| DomainError::InvalidDateFormat)?
            .with_timezone(&Local);

        Ok(ClipboardRecord::from_repository(
            id,
            content,
            content_type,
            created_at,
            updated_at,
            model.is_favorite == 1,
            model.is_deleted == 1,
        ))
    }
}
```

### 4.6 Shared 层（共享内核）

#### 4.6.1 错误类型

**示例**：`shared/errors.rs`
```rust
use thiserror::Error;

/// 统一错误类型
#[derive(Error, Debug)]
pub enum AppError {
    #[error("领域错误: {0}")]
    Domain(#[from] DomainError),

    #[error("基础设施错误: {0}")]
    Infrastructure(#[from] InfraError),

    #[error("应用错误: {0}")]
    Application(String),
}

/// 领域错误（业务规则违反）
#[derive(Error, Debug)]
pub enum DomainError {
    #[error("内容不能为空")]
    EmptyContent,

    #[error("无效的内容类型: {0}")]
    InvalidContentType(String),

    #[error("邮箱不能为空")]
    EmptyEmail,

    #[error("邮箱格式错误")]
    InvalidEmailFormat,

    #[error("密码太短，至少需要 {0} 位")]
    PasswordTooShort(usize),

    #[error("记录已被删除")]
    AlreadyDeleted,

    #[error("权限不足")]
    InsufficientPrivilege,
}

/// 基础设施错误（技术问题）
#[derive(Error, Debug)]
pub enum InfraError {
    #[error("数据库错误: {0}")]
    Database(String),

    #[error("网络错误: {0}")]
    Network(String),

    #[error("文件系统错误: {0}")]
    FileSystem(String),
}

/// 统一 Result 类型
pub type Result<T> = std::result::Result<T, AppError>;
```

---

## 五、核心模块示例

### 5.1 完整的用户注册流程

展示从前端到数据库的完整流程：

#### 前端调用
```typescript
// frontend/src/api/user.ts
import { invoke } from '@tauri-apps/api/tauri';

export async function register(email: string, password: string) {
  return await invoke<UserDto>('register_user', {
    email,
    password
  });
}
```

#### Interfaces 层
```rust
// interfaces/commands/user_commands.rs
#[tauri::command]
pub async fn register_user(
    email: String,
    password: String,
    state: State<'_, AppState>
) -> Result<UserDto, String> {
    let user = state.user_service
        .register(email, password)
        .await
        .map_err(|e| e.to_string())?;

    Ok(UserDto::from(user))
}
```

#### Application 层
```rust
// application/services/user_service.rs
pub struct UserService {
    user_repo: Arc<dyn UserRepository>,
}

impl UserService {
    pub async fn register(
        &self,
        email_str: String,
        password_str: String
    ) -> Result<User> {
        // 1. 创建值对象（自动验证）
        let email = Email::new(email_str)?;
        let password = Password::new(password_str)?;

        // 2. 检查邮箱是否已注册
        if self.user_repo.exists_by_email(&email).await? {
            return Err(AppError::Application("邮箱已注册".into()));
        }

        // 3. 创建用户实体
        let user = User::register(email, password)?;

        // 4. 持久化
        self.user_repo.save(&user).await?;

        // 5. 发布领域事件（可选）
        // EventBus::publish(UserRegisteredEvent::new(user.id()));

        Ok(user)
    }
}
```

#### Domain 层
```rust
// domain/user/entities/user.rs
pub struct User {
    id: UserId,
    email: Email,
    password: Password,
    profile: UserProfile,
    created_at: DateTime<Local>,
}

impl User {
    /// 用户注册（工厂方法）
    pub fn register(email: Email, password: Password) -> Result<Self, DomainError> {
        Ok(Self {
            id: UserId::generate(),
            email,
            password: password.hash()?,  // 密码值对象自带加密
            profile: UserProfile::default(),
            created_at: Local::now(),
        })
    }

    /// 修改密码
    pub fn change_password(
        &mut self,
        old_password: &str,
        new_password: Password
    ) -> Result<(), DomainError> {
        // 验证旧密码
        if !self.password.verify(old_password) {
            return Err(DomainError::PasswordMismatch);
        }

        // 更新密码
        self.password = new_password.hash()?;
        Ok(())
    }
}

// domain/user/value_objects/password.rs
pub struct Password(String);

impl Password {
    pub fn new(raw: String) -> Result<Self, DomainError> {
        if raw.len() < 6 {
            return Err(DomainError::PasswordTooShort(6));
        }
        Ok(Self(raw))
    }

    /// 加密密码
    pub fn hash(self) -> Result<Self, DomainError> {
        // 使用 bcrypt 或其他加密算法
        let hashed = bcrypt::hash(&self.0, bcrypt::DEFAULT_COST)
            .map_err(|_| DomainError::PasswordHashFailed)?;
        Ok(Self(hashed))
    }

    /// 验证密码
    pub fn verify(&self, raw: &str) -> bool {
        bcrypt::verify(raw, &self.0).unwrap_or(false)
    }
}
```

#### Infrastructure 层
```rust
// infrastructure/persistence/repositories/user_repository_impl.rs
pub struct UserRepositoryImpl {
    db: Arc<RBatis>,
}

#[async_trait]
impl UserRepository for UserRepositoryImpl {
    async fn save(&self, user: &User) -> Result<()> {
        let model = UserModel::from(user);
        model.insert(&self.db).await?;
        Ok(())
    }

    async fn exists_by_email(&self, email: &Email) -> Result<bool> {
        let sql = "SELECT COUNT(*) FROM users WHERE email = ?";
        let count: i64 = self.db
            .fetch_count(sql, vec![email.value()])
            .await?;
        Ok(count > 0)
    }
}
```

### 5.2 VIP 权益检查示例

展示领域服务的使用：

```rust
// domain/vip/services/privilege_checker.rs
pub struct PrivilegeChecker;

impl PrivilegeChecker {
    /// 检查用户是否有某个权益
    pub fn has_privilege(membership: &Membership, privilege: Privilege) -> bool {
        if membership.is_expired() {
            return false;
        }

        membership.level().includes_privilege(privilege)
    }

    /// 检查用户是否可以使用云同步
    pub fn can_use_cloud_sync(membership: &Membership) -> bool {
        Self::has_privilege(membership, Privilege::CloudSync)
    }
}

// domain/vip/value_objects/membership_level.rs
#[derive(Debug, Clone, PartialEq)]
pub enum MembershipLevel {
    Free,
    VIP,
    SVIP,
}

impl MembershipLevel {
    /// 判断是否包含某个权益
    pub fn includes_privilege(&self, privilege: Privilege) -> bool {
        match (self, privilege) {
            (_, Privilege::BasicClipboard) => true,  // 所有用户都有
            (MembershipLevel::VIP | MembershipLevel::SVIP, Privilege::CloudSync) => true,
            (MembershipLevel::SVIP, Privilege::UnlimitedStorage) => true,
            _ => false,
        }
    }
}
```

---

## 六、统一错误处理与响应包装

### 6.1 统一响应格式

#### 设计原则

所有前端与后端的交互都应该返回统一的响应格式：

```typescript
// 成功响应
{
  "success": true,
  "data": { ... },
  "timestamp": "2025-11-28T10:30:00Z"
}

// 失败响应
{
  "success": false,
  "error": {
    "code": "DOMAIN_001",
    "message": "用户友好的错误消息",
    "details": "开发者调试信息（可选）"
  },
  "timestamp": "2025-11-28T10:30:00Z"
}
```

#### Rust 类型定义

**文件路径**：`interfaces/dto/response.rs`

```rust
[统一响应包装器代码 - 见前文]
```

---

## 七、实施指南

### 7.1 依赖注入配置

在 `lib.rs` 中配置依赖注入：

```rust
// lib.rs
use std::sync::Arc;
use rbatis::RBatis;

pub struct AppState {
    // 仓储实例（使用 trait object）
    clipboard_repo: Arc<dyn ClipboardRepository>,
    user_repo: Arc<dyn UserRepository>,

    // 领域服务
    duplicate_detector: Arc<DuplicateDetector>,
    privilege_checker: Arc<PrivilegeChecker>,

    // 应用服务
    pub clipboard_service: Arc<ClipboardService>,
    pub user_service: Arc<UserService>,
    pub vip_service: Arc<VipService>,
}

impl AppState {
    pub async fn new() -> Result<Self> {
        // 1. 初始化数据库
        let db = Arc::new(init_database().await?);

        // 2. 创建仓储实例
        let clipboard_repo = Arc::new(ClipboardRepositoryImpl::new(db.clone()))
            as Arc<dyn ClipboardRepository>;
        let user_repo = Arc::new(UserRepositoryImpl::new(db.clone()))
            as Arc<dyn UserRepository>;

        // 3. 创建领域服务
        let duplicate_detector = Arc::new(
            DuplicateDetector::new(clipboard_repo.clone())
        );
        let privilege_checker = Arc::new(PrivilegeChecker);

        // 4. 创建应用服务
        let clipboard_service = Arc::new(ClipboardService::new(
            clipboard_repo.clone(),
            duplicate_detector.clone()
        ));
        let user_service = Arc::new(UserService::new(user_repo.clone()));
        let vip_service = Arc::new(VipService::new(privilege_checker.clone()));

        Ok(Self {
            clipboard_repo,
            user_repo,
            duplicate_detector,
            privilege_checker,
            clipboard_service,
            user_service,
            vip_service,
        })
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            // 初始化应用状态
            let app_state = tauri::async_runtime::block_on(async {
                AppState::new().await.expect("Failed to initialize app state")
            });

            app.manage(app_state);

            // 初始化平台层
            platform::window::init_main_window(app)?;
            platform::tray::init_tray(app)?;
            platform::menu::init_menu(app)?;

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            // 剪贴板命令
            interfaces::commands::clipboard_commands::save_clipboard,
            interfaces::commands::clipboard_commands::get_recent_clipboards,
            // 用户命令
            interfaces::commands::user_commands::register_user,
            interfaces::commands::user_commands::login_user,
            // ... 其他命令
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
```

### 6.2 迁移策略

#### 阶段一：基础设施（第 1-2 天）

1. 创建目录结构
2. 定义 `shared/errors.rs`
3. 定义 `shared/types.rs`
4. 迁移 `context.rs` 到 `shared/context.rs`

#### 阶段二：剪贴板模块（第 3-5 天）

1. 实现 `domain/clipboard/` 所有文件
2. 实现 `infrastructure/persistence/repositories/clipboard_repository_impl.rs`
3. 实现 `application/services/clipboard_service.rs`
4. 实现 `interfaces/commands/clipboard_commands.rs`
5. 测试完整流程

#### 阶段三：其他模块（第 6-12 天）

按以下顺序依次迁移：
1. User 模块
2. VIP 模块
3. Sync 模块
4. Search 模块

#### 阶段四：平台层（第 13-14 天）

1. 移动 `window.rs`、`tray.rs`、`menu.rs` 到 `platform/`
2. 更新导入路径

#### 阶段五：测试与优化（第 15-17 天）

1. 单元测试
2. 集成测试
3. 性能测试

### 6.3 测试策略

#### 单元测试示例

```rust
// domain/clipboard/entities/clipboard_record.rs
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_record() {
        let content = "Hello World".to_string();
        let content_type = ContentType::Text;

        let record = ClipboardRecord::new(content, content_type);
        assert!(record.is_ok());
    }

    #[test]
    fn test_empty_content_should_fail() {
        let content = "".to_string();
        let content_type = ContentType::Text;

        let record = ClipboardRecord::new(content, content_type);
        assert!(record.is_err());
    }

    #[test]
    fn test_mark_favorite() {
        let mut record = ClipboardRecord::new(
            "Test".to_string(),
            ContentType::Text
        ).unwrap();

        assert!(!record.is_favorite());
        record.mark_favorite();
        assert!(record.is_favorite());
    }
}
```

## 八、最佳实践

### 8.1 DO's（应该做的）

✅ **业务逻辑放在 Domain 层**
```rust
// ✅ 正确：实体包含行为
impl User {
    pub fn change_email(&mut self, new_email: Email) -> Result<()> {
        // 业务规则：验证邮箱
        if self.email == new_email {
            return Err(DomainError::SameEmail);
        }
        self.email = new_email;
        Ok(())
    }
}
```

✅ **使用值对象封装验证**
```rust
// ✅ 正确：值对象自带验证
pub struct Email(String);
impl Email {
    pub fn new(value: String) -> Result<Self> {
        validate_email(&value)?;
        Ok(Self(value))
    }
}
```

✅ **通过仓储接口解耦**
```rust
// ✅ 正确：依赖接口而非实现
pub struct ClipboardService {
    repo: Arc<dyn ClipboardRepository>,  // trait object
}
```

✅ **明确的错误处理**
```rust
// ✅ 正确：区分领域错误和技术错误
pub enum DomainError {
    InvalidEmail,
    PasswordTooShort,
}

pub enum InfraError {
    DatabaseError(String),
    NetworkError(String),
}
```

### 7.2 DON'Ts（不应该做的）

❌ **不要在 Service 层写业务逻辑**
```rust
// ❌ 错误：Service 层不应该包含业务规则
impl UserService {
    pub async fn change_email(&self, user_id: &str, email: String) -> Result<()> {
        // ❌ 业务逻辑不应该在这里
        if !email.contains('@') {
            return Err("Invalid email");
        }
        // ...
    }
}

// ✅ 正确：Service 只做编排
impl UserService {
    pub async fn change_email(&self, user_id: &str, email_str: String) -> Result<()> {
        let email = Email::new(email_str)?;  // 验证在值对象中
        let mut user = self.repo.find_by_id(user_id).await?.unwrap();
        user.change_email(email)?;  // 业务逻辑在实体中
        self.repo.save(&user).await?;
        Ok(())
    }
}
```

❌ **不要让 Domain 层依赖外层**
```rust
// ❌ 错误：Domain 层不应该依赖 Infrastructure
use crate::infrastructure::database::Database;  // ❌

pub struct User {
    db: Arc<Database>,  // ❌ 不应该直接依赖数据库
}
```

❌ **不要混淆领域模型和数据库模型**
```rust
// ❌ 错误：领域实体不应该包含 ORM 注解
#[crud_table(table_name: "users")]  // ❌
pub struct User {
    pub id: String,  // ❌ 应该用值对象
    pub email: String,  // ❌ 应该用值对象
}

// ✅ 正确：分离领域模型和数据库模型
// Domain 层
pub struct User {
    id: UserId,
    email: Email,
}

// Infrastructure 层
#[crud_table(table_name: "users")]
pub struct UserModel {
    pub id: String,
    pub email: String,
}
```

### 7.3 性能优化建议

1. **避免过度克隆**
   ```rust
   // ❌ 不必要的克隆
   let user = user_repo.find_by_id(id).await?.unwrap().clone();

   // ✅ 使用引用
   let user = user_repo.find_by_id(id).await?;
   ```

2. **使用 Arc 共享数据**
   ```rust
   // ✅ 仓储使用 Arc 避免克隆
   pub struct ClipboardService {
       repo: Arc<dyn ClipboardRepository>,
   }
   ```

3. **批量操作优化**
   ```rust
   // ✅ 批量插入而非循环单条
   async fn save_batch(&self, records: Vec<ClipboardRecord>) -> Result<()> {
       let models: Vec<ClipboardModel> = records
           .iter()
           .map(|r| ClipboardModel::from(r))
           .collect();

       self.db.exec_batch("INSERT INTO ...", models).await?;
       Ok(())
   }
   ```

---

## 九、总结

### 9.1 核心收益

| 方面 | 重构前 | 重构后 |
|-----|--------|--------|
| **可维护性** | 业务逻辑分散在 Service 层 | 业务逻辑集中在 Domain 层 |
| **可测试性** | 难以单元测试业务规则 | Domain 层纯 Rust，易测试 |
| **可扩展性** | 修改需要改动多处 | 符合开闭原则，易扩展 |
| **团队协作** | 职责不清晰 | 每层职责明确 |
| **技术债** | 逐渐累积 | 架构清晰，债务少 |

### 9.2 关键差异：DDD vs 传统架构

| 特性 | 传统三层架构 | DDD 架构 |
|-----|-------------|----------|
| Entity | 数据容器（贫血模型） | 业务对象（富领域模型） |
| Service | 包含所有业务逻辑 | 只做用例编排 |
| 验证 | 在 Service 层 | 在值对象中 |
| 数据库模型 | 与实体混在一起 | 分离（Domain vs Infrastructure） |
| 依赖方向 | Domain 依赖 Infrastructure | Infrastructure 依赖 Domain |

### 9.3 下一步行动

1. ✅ 阅读并理解本文档
2. ✅ 从剪贴板模块开始重构（示范作用）
3. ✅ 逐步迁移其他模块
4. ✅ 编写单元测试
5. ✅ 性能测试对比

---

## 附录

### A. 术语表

- **Entity（实体）**：有唯一标识的业务对象
- **Value Object（值对象）**：无唯一标识，由属性值决定相等性
- **Aggregate（聚合）**：一组相关对象的集合，有一个聚合根
- **Repository（仓储）**：封装数据访问的接口
- **Domain Service（领域服务）**：无法归属到单个实体的业务逻辑
- **Application Service（应用服务）**：用例编排，协调多个领域对象
- **DTO（数据传输对象）**：用于层与层之间传输数据

### B. 参考资料

1. 《领域驱动设计》（Eric Evans）
2. 《实现领域驱动设计》（Vaughn Vernon）
3. 《整洁架构》（Robert C. Martin）
4. [Rust 官方文档](https://doc.rust-lang.org/)
5. [Tauri 官方文档](https://tauri.app/)

---

**文档版本**：v1.0
**最后更新**：2025-11-28
**作者**：ClipPal Team
**状态**：待实施

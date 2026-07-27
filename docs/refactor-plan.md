# ClipPal 重构方案

## 1. 重构目标

本次重构的目标不是重写项目，而是在保持现有功能可运行的前提下，逐步把代码整理成更清晰、更模块化、更容易扩展的结构。

目标包括：

- 代码结构清晰：能快速判断一段逻辑应该放在前端、后端、系统层、业务层还是基础设施层。
- 模块职责单一：一个模块尽量只负责一类问题，减少大文件持续膨胀。
- 前后端交互统一：所有 Tauri command 返回统一响应结构，前端不再到处写散落的错误判断。
- 易扩展：搜索、同步、剪贴板写入、支付、更新、存储等可能替换方案的逻辑，尽量通过接口或适配层隔离。
- 易测试：业务逻辑尽量从 Tauri command、全局状态、系统 API 中剥离出来，便于单元测试。
- 可渐进迁移：每一步都能构建通过，避免一次性大改造成不可控风险。

## 2. 当前主要问题

### 2.1 后端职责边界不够清晰

当前后端模块大致已经按文件拆分，但有些文件仍然混合了多种职责：

- `src-tauri/src/lib.rs`：承担了较多启动编排职责，包括数据库、搜索索引、监听器、定时器、VIP 检查、队列等启动顺序。
- `src-tauri/src/biz/clip_record.rs`：同时承担数据模型、数据库查询、排序规则、同步状态更新等职责。
- `src-tauri/src/biz/copy_clip_record.rs`：同时承担 Tauri command、数据库查询、剪贴板写入、自动粘贴、文件临时处理、系统权限提示。
- `src-tauri/src/sqlite_storage.rs`：数据库连接、schema 描述、迁移、索引创建都放在一个文件里。

这些问题不一定导致功能错误，但会让后续加功能和替换实现变困难。

### 2.2 前端组件过大

当前前端组件中存在明显的大组件：

- `frontend/src/components/ScrollContainer.vue`
- `frontend/src/components/ClipCard.vue`
- `frontend/src/components/LoginDialog.vue`
- `frontend/src/components/SettingsDialog.vue`

其中 `ScrollContainer.vue` 混合了列表展示、搜索、防抖、分页、事件监听、云同步状态、登录入口、VIP 入口和设置入口。

`ClipCard.vue` 混合了不同类型内容展示、复制、删除、置顶、图片懒加载、文件处理、自动粘贴提示、确认弹窗和大量样式。

### 2.3 前后端响应格式不统一

后端 command 当前有的返回 `Result<T, String>`，有的返回字符串，有的返回不同 DTO。前端虽然封装了 `apiInvoke`，但错误级别主要靠 command 名称映射，后端没有统一表达：

- 是否成功
- 错误码
- 用户可读错误信息
- 错误严重程度
- 是否需要前端提示

这会导致前端错误处理越来越分散。

### 2.4 全局上下文依赖较重

当前通过全局 `CONTEXT` 获取 `RBatis`、`AppHandle`、设置、队列等对象。短期方便，但依赖关系隐藏，不利于测试和替换实现。

重构目标不是立即删除 `CONTEXT`，而是逐步引入显式 `AppContext`，让新的代码优先依赖明确状态。

## 3. 后端目标架构

建议后端逐步整理为以下结构。注意：这是 Tauri 项目，`main.rs` 和 `lib.rs` 必须保留。重构不是取消入口文件，而是让入口文件更薄、更清晰。

```text
src-tauri/src/
  main.rs                         // Tauri 桌面入口，保持极薄，只调用 clip_pal_lib::run()
  lib.rs                          // 应用组装入口：注册插件、注册 commands、挂载 AppContext、连接启动流程
  app_context.rs                  // 应用状态和依赖聚合                  
  errors.rs                       // 后端内部统一错误 AppError / AppResult
  response.rs                     // Tauri command 统一响应 CommandResponse<T>

  bootstrap/
    mod.rs                        // 对外暴露 init_core、setup_app、start_runtime_tasks
    core.rs                       // 日志、配置、数据库、搜索索引等核心依赖初始化
    plugins.rs                    // Tauri 插件注册封装
    setup.rs                      // setup 阶段：窗口、托盘、菜单、快捷键、监听器
    runtime.rs                    // RunEvent 处理、退出清理、Ready 后任务

  commands/
    mod.rs                        // 统一导出 command handler 列表
    clip_commands.rs              // 剪贴记录查询、复制、删除、置顶、图片路径等 command
    settings_commands.rs          // 设置读取、保存、快捷键校验
    auth_commands.rs              // 登录、注册、登出、用户信息
    vip_commands.rs               // VIP 状态、权益、支付
    update_commands.rs            // 版本检查、下载安装更新

  domain/
    mod.rs
    clip/
      mod.rs
      clip_record.rs              // 剪贴记录领域模型
      clip_type.rs                // ClipType 领域枚举，避免散落字符串
      sync_status.rs              // 同步状态枚举，替代 0/1/2/3 魔法值
      skip_sync_reason.rs         // 跳过同步原因枚举
      rules.rs                    // 剪贴记录相关纯业务规则
    settings/
      mod.rs
      settings.rs                 // Settings 领域模型
      rules.rs                    // 设置校验规则
    user/
      mod.rs
      user_info.rs
      auth_state.rs
    vip/
      mod.rs
      vip_info.rs
      entitlement.rs              // VIP 权益判断模型
    sync/
      mod.rs
      sync_record.rs
      conflict.rs                 // 同步冲突决策模型

  services/
    mod.rs
    clip_record_service.rs        // 查询、删除、置顶、完整内容、图片路径
    clipboard_capture_service.rs  // 处理系统剪贴板事件并生成记录
    clipboard_copy_service.rs     // 根据记录写入系统剪贴板，可触发自动粘贴
    settings_service.rs           // 设置保存、校验、应用
    auth_service.rs               // 登录、登出、token 验证、用户状态
    vip_service.rs                // VIP 状态、权益、支付流程
    cloud_sync_service.rs         // 云同步编排
    update_service.rs             // 更新检查与安装编排

  infra/
    mod.rs
    db/
      mod.rs
      sqlite.rs                   // SQLite 连接初始化
      migrations.rs               // 表结构迁移
      schema.rs                   // 表结构描述
    repositories/
      mod.rs
      clip_record_repository.rs   // ClipRecordRepository trait
      sqlite_clip_record_repo.rs  // RBatis/SQLite 实现
      settings_repository.rs      // 设置文件读写 trait
      file_settings_repo.rs       // settings.json 实现
      sync_time_repository.rs
    search/
      mod.rs
      search_engine.rs            // SearchEngine trait
      in_memory_search_engine.rs  // 当前 Bloom Filter 内存搜索实现
    http/
      mod.rs
      http_client.rs              // HTTP 客户端适配
      cloud_sync_client.rs        // CloudSyncClient trait + 默认实现
      auth_client.rs
      vip_client.rs
    storage/
      mod.rs
      resource_storage.rs         // 图片/文件资源存储 trait
      local_resource_storage.rs   // 本地 resources 目录实现
    security/
      mod.rs
      crypto.rs                   // 加密解密适配
      token_store.rs              // token 存取适配
      secure_store.rs

  system/
    mod.rs
    logging.rs                    // 日志初始化
    window.rs                     // 窗口创建、显示隐藏
    tray.rs                       // 托盘
    menu.rs                       // 菜单
    shortcut.rs                   // 全局快捷键
    clipboard_listener.rs         // 系统剪贴板监听接入
    clipboard_writer.rs           // ClipboardWriter trait
    tauri_clipboard_writer.rs     // Tauri 剪贴板插件适配
    auto_paste.rs                 // 自动粘贴系统能力
    updater.rs                    // Tauri updater 接入
    autostart.rs                  // 开机自启
    dialogs.rs                    // 系统对话框封装

  dto/
    mod.rs
    clip_dto.rs                   // ClipRecordDTO / QueryParam / ImagePathInfo
    settings_dto.rs
    auth_dto.rs
    vip_dto.rs
    update_dto.rs
```

### 3.1 入口文件职责

`main.rs` 保持最薄：

```rust
fn main() {
    tauri::async_runtime::block_on(clip_pal_lib::run()).expect("failed to run app");
}
```

`lib.rs` 负责应用组装，但不承载具体业务细节：

```rust
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub async fn run() -> Result<(), Box<dyn std::error::Error>> {
    system::logging::init();

    let core = bootstrap::init_core().await?;

    tauri::Builder::default()
        .plugin(bootstrap::plugins::updater())
        .plugin(bootstrap::plugins::dialog())
        .plugin(bootstrap::plugins::clipboard())
        .manage(core.app_context)
        .setup(move |app| bootstrap::setup_app(app, core.clone()))
        .invoke_handler(commands::handler())
        .build(tauri::generate_context!())?
        .run(move |app, event| bootstrap::handle_run_event(app, event, core.clone()));

    Ok(())
}
```

`lib.rs` 可以保留这些职责：

- 注册 Tauri 插件。
- 注册 command handler。
- 创建并挂载 `AppContext`。
- 串联启动阶段。
- 连接 Tauri 生命周期事件。

`lib.rs` 不建议继续承载这些职责：

- 直接写数据库 schema 或迁移规则。
- 直接写剪贴板记录处理规则。
- 直接写云同步、VIP、搜索索引的业务细节。
- 直接访问大量全局状态并编排复杂业务。

### 3.2 模块分类

| 分类 | 内容 | 示例 |
| --- | --- | --- |
| 系统能力 | 和操作系统、Tauri 生命周期强相关 | 日志、窗口、托盘、快捷键、剪贴板监听、自动粘贴、软件更新 |
| 基础设施 | 具体技术实现，可替换 | SQLite、HTTP、文件系统、加密、安全存储、搜索索引底层 |
| 业务领域 | 业务概念和规则 | 剪贴记录、同步状态、用户登录状态、VIP 权益、设置规则 |
| 应用服务 | 编排多个领域对象和基础设施 | 复制剪贴记录、保存设置、云同步、用户登录、VIP 刷新 |
| 命令边界 | 前端调用入口 | Tauri command |

### 3.3 依赖方向

推荐依赖方向：

```text
commands -> services -> domain
services -> infra traits
infra implementations -> external libraries
system -> Tauri / OS APIs
```

原则：

- `commands` 不直接写复杂业务逻辑。
- `domain` 不依赖 Tauri、RBatis、HTTP、文件系统。
- `services` 负责业务流程编排。
- `infra` 负责具体技术实现。
- `system` 负责系统级能力封装。

## 4. 前后端统一响应格式

这是建议最先做的重构，因为它能稳定前后端边界，降低后续重构风险。

### 4.1 后端内部仍然使用 Result

后端内部业务代码继续使用：

```rust
pub type AppResult<T> = Result<T, AppError>;
```

不要让业务层返回前端响应格式。业务层只关心成功或失败，不关心前端如何展示。

### 4.2 Tauri command 返回统一结构

新增 `src-tauri/src/response.rs`：

```rust
use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct CommandResponse<T>
where
    T: Serialize,
{
    pub success: bool,
    pub data: Option<T>,
    pub error: Option<CommandErrorBody>,
}

#[derive(Debug, Serialize)]
pub struct CommandErrorBody {
    pub code: String,
    pub message: String,
    pub severity: ErrorSeverity,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorSeverity {
    Silent,
    Info,
    Warning,
    Critical,
}

pub fn ok<T>(data: T) -> CommandResponse<T>
where
    T: Serialize,
{
    CommandResponse {
        success: true,
        data: Some(data),
        error: None,
    }
}

pub fn empty_ok() -> CommandResponse<()> {
    CommandResponse {
        success: true,
        data: Some(()),
        error: None,
    }
}
```

错误转换统一放在 `AppError -> CommandErrorBody`：

```rust
impl AppError {
    pub fn code(&self) -> &'static str {
        match self {
            AppError::Database(_) => "database_error",
            AppError::Io(_) => "io_error",
            AppError::Config(_) => "config_error",
            AppError::Clipboard(_) => "clipboard_error",
            AppError::Network(_) => "network_error",
            AppError::ClipSync(_) => "sync_error",
            AppError::Crypto(_) => "crypto_error",
            _ => "internal_error",
        }
    }

    pub fn severity(&self) -> ErrorSeverity {
        match self {
            AppError::Database(_) => ErrorSeverity::Warning,
            AppError::Config(_) => ErrorSeverity::Warning,
            AppError::Clipboard(_) => ErrorSeverity::Critical,
            AppError::Network(_) => ErrorSeverity::Warning,
            AppError::ClipSync(_) => ErrorSeverity::Info,
            _ => ErrorSeverity::Info,
        }
    }
}
```

command 层示例：

```rust
#[tauri::command]
pub async fn get_clip_records(param: QueryParam) -> CommandResponse<Vec<ClipRecordLiteDTO>> {
    command_result(clip_record_service::query(param).await)
}
```

### 4.3 前端统一响应类型

前端统一为：

```ts
export interface ApiResponse<T> {
  success: boolean
  data: T | null
  error: ApiError | null
}

export interface ApiError {
  code: string
  message: string
  severity: 'silent' | 'info' | 'warning' | 'critical'
}
```

`apiInvoke` 只处理统一协议：

```ts
export async function apiInvoke<T>(
  command: string,
  args?: unknown
): Promise<ApiResponse<T>> {
  const response = await invoke<ApiResponse<T>>(command, args)

  if (!response.success && response.error?.severity !== 'silent') {
    globalErrorHandler?.(response.error.message, response.error.severity, command)
  }

  return response
}
```

迁移期间可以保留兼容逻辑，允许旧 command 继续返回 `Result<T, String>`，但新 command 必须使用统一响应。

## 5. AppContext 与依赖注入

### 5.1 目标

逐步把全局 `CONTEXT` 替换为明确的应用上下文资源：

```rust
pub struct AppContext {
    pub db: RBatis,
    pub settings: Arc<RwLock<Settings>>,
    sync_lock: GlobalSyncLock,
    clip_record_queue: AsyncQueue<ClipRecord>,
    window_focus_count: WindowFocusCount,
    window_hide_flag: WindowHideFlag,
}
```

Tauri 中注册：

```rust
let app_context = Arc::new(AppContextConfig::create(db, settings));
builder.manage(app_context.clone())
```

command 中使用：

```rust
pub async fn get_clip_records(
    state: tauri::State<'_, Arc<AppContext>>,
    param: QueryParam,
) -> CommandResponse<Vec<ClipRecordLiteDTO>> {
    command_result(query_clip_records(&state.db, param).await)
}
```

### 5.2 迁移策略

不要一次性替换所有 `CONTEXT`。

推荐顺序：

1. 新增 `AppContext`，先放 `RBatis` 和 `Settings`。
2. 新 command 优先使用 `State<Arc<AppContext>>`。
3. 旧模块仍可暂时使用 `CONTEXT`。
4. 每重构一个 service，就把它依赖的状态从 `CONTEXT` 迁移到 `AppContext`。
5. 最后再收缩 `CONTEXT` 的使用范围。

## 6. 可扩展接口设计

有多种实现方案、未来可能替换的能力，建议抽象接口。接口不是越多越好，只在存在明确替换可能时引入。

### 6.1 搜索引擎

当前搜索使用内存索引和 Bloom Filter。未来可能切换为 SQLite FTS、Tantivy 或其他方案。

建议接口：

```rust
#[async_trait::async_trait]
pub trait SearchEngine: Send + Sync {
    async fn initialize(&self, records: Vec<ClipRecord>) -> AppResult<()>;
    async fn add(&self, id: &str, content: &str) -> AppResult<()>;
    async fn remove(&self, ids: &[String]) -> AppResult<()>;
    async fn search(&self, query: &str) -> AppResult<Vec<String>>;
}
```

实现：

```text
infra/search/in_memory_search_engine.rs
infra/search/sqlite_fts_search_engine.rs   // 后续可选
```

使用的设计模式：Strategy。

### 6.2 剪贴板写入

文本、图片、文件写入系统剪贴板涉及 Tauri 插件和不同平台能力。

建议接口：

```rust
#[async_trait::async_trait]
pub trait ClipboardWriter: Send + Sync {
    async fn write_text(&self, text: String) -> AppResult<()>;
    async fn write_image(&self, bytes: Vec<u8>) -> AppResult<()>;
    async fn write_files(&self, paths: Vec<String>) -> AppResult<()>;
}
```

实现：

```text
system/clipboard/tauri_clipboard_writer.rs
```

使用的设计模式：Adapter。

### 6.3 剪贴记录仓储

把 `ClipRecord` 中的数据库操作移到 repository：

```rust
#[async_trait::async_trait]
pub trait ClipRecordRepository: Send + Sync {
    async fn find_by_id(&self, id: &str) -> AppResult<Option<ClipRecord>>;
    async fn list(&self, query: ClipRecordQuery) -> AppResult<Vec<ClipRecord>>;
    async fn insert(&self, record: &ClipRecord) -> AppResult<()>;
    async fn update_sort(&self, id: &str, sort: i32) -> AppResult<()>;
    async fn set_pinned(&self, id: &str, pinned: bool) -> AppResult<()>;
    async fn tombstone(&self, ids: &[String]) -> AppResult<()>;
}
```

实现：

```text
infra/repositories/sqlite_clip_record_repository.rs
```

使用的设计模式：Repository。

### 6.4 云同步

云同步采用“HTTP 负责写入和补拉，SSE 负责实时通知”的组合方案。SSE 是服务端到客户端的单向通道，不承担记录上传；本地新增或修改记录后，仍通过 HTTP 提交到云端，服务端持久化成功后再向该用户的全部在线客户端广播 SSE 事件。

推荐链路：

```text
本地新增记录
  -> 写入本地数据库和持久化 outbox
  -> HTTP 幂等上传
  -> 服务端持久化
  -> SSE 广播变更事件
  -> 各客户端去重并合并，必要时通过 HTTP 拉取完整内容
```

登录后的初始化链路：

1. 从服务端分页拉取近期记录快照，同时取得服务端同步游标 `cursor`。
2. 在本地事务中合并快照、删除墓碑和同步游标。
3. 使用该 `cursor` 建立 SSE 连接；服务端必须能够重放游标之后的事件，避免“拉取结束到 SSE 建连之间”丢失消息。
4. SSE 断开后使用最后成功提交的事件游标重连，并采用带抖动的指数退避。

SSE 事件只携带同步所需的最小信息：

```text
event_id / cursor     服务端单调递增事件游标
operation             upsert / delete
record_id             记录 ID
version               记录版本，用于幂等和冲突判断
created_at            记录产生时间
content_mode          inline / reference
record_type           text / image / file
content               可选，仅短内容内联
checksum              可选，用于校验拉取结果
```

- 短文本可以直接内联在 SSE 事件中，阈值由服务端统一配置。
- 长文本、图片和文件只广播记录元数据，客户端收到后通过 HTTP 获取完整记录或下载资源。
- 本机上传成功后仍可能收到自己的 SSE 事件，客户端必须按 `record_id + version` 幂等去重。
- 删除操作必须使用墓碑事件，不能只依赖服务端当前列表，否则离线客户端无法得知记录已删除。
- 本地待上传任务应持久化到 SQLite outbox，不能只放在内存队列中，避免程序退出或断网时丢失。

记录列表的合并和排序以记录产生时间 `created_at` 为主。同一记录发生重复通知或状态更新时，使用 `version` 判断新旧；产生时间相同的不同记录使用 `record_id` 作为稳定排序补充条件。SSE 断线续传使用服务端 `cursor`，不能使用客户端时间戳代替，因为不同设备的系统时间可能不一致。

建议拆成以下接口：

```rust
#[async_trait::async_trait]
pub trait CloudSyncClient: Send + Sync {
    async fn bootstrap(&self, cursor: Option<String>) -> AppResult<SyncSnapshot>;
    async fn push_record(&self, record: &ClipRecord, idempotency_key: &str) -> AppResult<PushResult>;
    async fn fetch_record(&self, id: &str, version: u64) -> AppResult<RemoteClipRecord>;
    async fn upload_file(&self, file: UploadFile) -> AppResult<RemoteFile>;
    async fn download_file(&self, file_id: &str) -> AppResult<Vec<u8>>;
}

pub trait SyncEventStream: Send + Sync {
    async fn connect(&self, cursor: Option<String>) -> AppResult<SyncEventReceiver>;
}

pub trait SyncConflictResolver: Send + Sync {
    fn resolve(&self, local: Option<ClipRecord>, remote: RemoteClipRecord) -> SyncDecision;
}
```

`CloudSyncService` 负责编排初始化快照、outbox 上传、SSE 消费、HTTP 补拉和本地事务；HTTP、SSE、数据库和资源下载的具体实现放在 `infra`。SSE 连接使用当前有效 access token，token 刷新成功后应使用新 token 自动重连。

使用的设计模式：Outbox、Strategy、Observer。

### 6.5 本地会话与 VIP 权益

本地身份认证和 VIP 信息更新都属于“服务端状态在本地的缓存与刷新”问题，应共享刷新机制，但不能把 access token、用户资料和 VIP 权益混成同一个领域对象。

建议拆分为：

```text
SessionService             登录、登出、token 刷新、当前用户快照
EntitlementService         VIP 权益刷新、缓存和业务规则
AuthenticatedHttpClient    注入 token、识别失效响应、触发刷新并重试一次
SessionStore               加密持久化 token 和用户资料
EntitlementStore           持久化 VIP 快照及最后校验时间
```

统一处理流程：

1. 普通请求使用内存中的会话快照，启动时从加密存储恢复。
2. 服务端明确返回 access token 过期时，只允许一个请求执行 refresh token，其余并发请求等待同一刷新结果。
3. token 刷新成功后原子更新内存和本地加密存储，原请求最多自动重试一次，防止无限重试。
4. refresh token 也失效时清空本地会话、停止需要登录的后台任务，并统一发送 `auth-state-changed` 事件。
5. 服务端返回 VIP 状态过期或权益版本落后时，调用权益查询接口刷新 `EntitlementSnapshot`，原子更新缓存后发送 `vip-status-changed` 事件。
6. 支付成功、SSE 收到账户权益变化事件或用户主动刷新时，也进入同一个权益刷新入口，不再各自实现缓存更新逻辑。

云同步开关属于用户偏好。短暂 token 过期或网络错误时应暂停同步并尝试恢复，不应直接永久关闭并覆盖用户设置；只有明确退出登录或产品规则要求时才修改该设置。

本地 VIP 缓存至少需要保存 `checked_at`、`expires_at` 和服务端权益版本。网络不可用时可以用于界面展示，但涉及付费能力的最终判断应依据明确的离线策略，不能把“缓存存在”等同于“权益永久有效”。

VIP 判断不建议散落在剪贴板处理、设置保存、同步处理里。可以抽成策略：

```rust
pub trait EntitlementPolicy: Send + Sync {
    fn can_cloud_sync(&self, user: &UserState, vip: &VipInfo) -> PolicyDecision;
    fn can_store_record(&self, current_count: u32, settings: &Settings, vip: &VipInfo) -> PolicyDecision;
    fn can_sync_file(&self, file_size: u64, vip: &VipInfo) -> PolicyDecision;
}
```

使用的设计模式：Policy Object、Strategy。

## 7. 后端业务服务拆分建议

### 7.1 剪贴记录服务

```text
services/clip_record_service.rs
```

职责：

- 查询剪贴记录列表
- DTO 转换
- 删除记录
- 置顶记录
- 获取完整文本
- 获取图片路径

不负责：

- 直接处理 Tauri command
- 直接写系统剪贴板
- 直接做 HTTP 请求

### 7.2 剪贴板捕获服务

```text
services/clipboard_capture_service.rs
```

`clipboard-listener` 模块以及 `clipboard-listener -> ClipboardEventTigger::handle_event` 的触发链路是当前稳定的数据来源边界，不纳入本轮重构。不得修改监听模块、事件类型、注册方式和触发语义。

后续如果整理 `handle_event` 收到事件之后的业务代码，只能在保持上述入口完全兼容的前提下向服务层委托，不能改变剪贴板事件的来源和触发时机。当前优先处理会话状态与云同步，不开展这部分迁移。

业务处理职责：

- 接收系统剪贴板事件
- 判断文本、图片、文件类型
- 生成 `ClipRecord`
- 去重
- 保存文件资源
- 写入数据库
- 更新搜索索引
- 发送前端事件
- 投递同步队列

这个服务可以继续被 `clip_board_listener` 调用，但具体业务不要散在监听器实现里。

### 7.3 剪贴板复制服务

```text
services/clipboard_copy_service.rs
```

职责：

- 根据 record id 查询记录
- 解密或读取实际内容
- 调用 `ClipboardWriter`
- 根据设置决定是否自动粘贴
- 返回统一业务结果

### 7.4 设置服务

```text
services/settings_service.rs
```

职责：

- 加载设置
- 保存设置
- 校验设置
- 应用系统设置，例如快捷键、自启、云同步开关

注意拆分：

- 配置文件读写属于 `infra/settings/settings_repository.rs`
- 快捷键注册属于 `system/shortcut`
- VIP 限制校验属于业务策略

### 7.5 用户与 VIP 服务

```text
services/auth_service.rs
services/vip_service.rs
```

职责：

- 登录、登出、token 验证
- token 过期后的单飞刷新和单次请求重试
- 用户信息快照和加密持久化
- VIP 权益快照、版本和过期时间管理
- 服务端失效响应、支付结果和实时事件触发的统一刷新
- VIP 权益判断

`AuthService` 和 `VipService` 可以保留面向 command 的接口，但底层应共享统一状态刷新协调器。所有 HTTP API 不应各自读取 token、刷新 token 或更新 VIP 缓存。

不建议让前端直接关心太多 VIP 规则。前端只展示后端发布的状态快照，核心限制规则和缓存有效性由 Rust 后端判断。

## 8. 前端目标架构

建议前端结构：

```text
frontend/src/
  api/
    client.ts
    clipApi.ts
    settingsApi.ts
    userApi.ts
    vipApi.ts
  types/
    api.ts
    clip.ts
    settings.ts
    user.ts
    vip.ts
  stores/
    userStore.ts
    vipStore.ts
    settingsStore.ts
  composables/
    useClipList.ts
    useClipSearch.ts
    useClipEvents.ts
    useCloudSync.ts
    useImageLoader.ts
    useMessageBar.ts
  components/
    clip/
      ClipPanel.vue
      ClipToolbar.vue
      ClipList.vue
      ClipCard.vue
      ClipCardHeader.vue
      ClipCardActions.vue
      ClipTextContent.vue
      ClipImageContent.vue
      ClipFileContent.vue
    dialogs/
      SettingsDialog.vue
      LoginDialog.vue
      UserInfoDialog.vue
      VipAccountDialog.vue
      UpdateDialog.vue
    common/
      MessageBar.vue
      ConfirmDialog.vue
```

### 8.1 ScrollContainer 拆分

`ScrollContainer.vue` 目标是变成页面容器，不直接承载全部业务。

建议拆出：

- `useClipList.ts`：分页、刷新、加载更多。
- `useClipSearch.ts`：搜索值、防抖搜索。
- `useClipEvents.ts`：监听 `clip_record_change`、同步状态更新、下载完成事件。
- `useCloudSync.ts`：云同步开关加载和保存。
- `ClipToolbar.vue`：标题、搜索框、同步按钮、用户按钮、设置按钮。
- `ClipList.vue`：列表、空状态、加载更多。

### 8.2 ClipCard 拆分

`ClipCard.vue` 目标是只负责卡片结构，不直接承担所有内容类型和操作细节。

建议拆出：

- `ClipCardHeader.vue`
- `ClipCardActions.vue`
- `ClipTextContent.vue`
- `ClipImageContent.vue`
- `ClipFileContent.vue`
- `ClipDeleteConfirm.vue`
- `AutoPasteWarningDialog.vue`

内容类型展示可以使用简单分发：

```vue
<ClipTextContent v-if="record.type === 'Text'" />
<ClipImageContent v-else-if="record.type === 'Image'" />
<ClipFileContent v-else-if="record.type === 'File'" />
```

暂时不需要为了“设计模式”强行引入复杂动态组件。先让文件变小、职责清晰。

### 8.3 前端状态边界

建议规则：

- 后端保存真实状态，例如 token、设置文件、VIP 缓存。
- 前端 store 保存展示状态和短期 UI 状态。
- 业务限制以 Rust 后端判断为准。
- 前端只做必要的交互预判，例如未登录时先弹登录框。

## 9. 分阶段实施路线

### 阶段 0：建立重构安全网

目标：确保后续每一步都能验证。

任务：

- 记录当前构建命令：
  - `cargo check --workspace`
  - `cd frontend && npm.cmd run build`
- 确认当前主流程可运行。
- 不改业务功能，只整理文档和目标结构。

验收：

- 两个构建命令能跑通，或者明确记录当前已知失败原因。

### 阶段 1：统一前后端响应格式

目标：稳定前后端边界。

任务：

- 新增 `response.rs`。
- 新增前端 `types/api.ts` 和 `api/client.ts`。
- 先迁移 3 到 5 个 command：
  - `get_clip_records`
  - `copy_clip_record`
  - `save_settings`
  - `login`
  - `get_vip_status`
- 前端 `apiInvoke` 同时兼容旧响应和新响应。

验收：

- 被迁移 command 的前端调用不再依赖 command 名称判断错误级别。
- 旧 command 仍能继续工作。

### 阶段 2：引入 AppContext

目标：减少新代码对 `CONTEXT` 的依赖。

任务：

- 新增 `app_context.rs`。
- 先把 `RBatis`、`Settings` 放进 `AppContext`。
- 新 command 使用 `tauri::State<AppContext>`。
- 保留 `CONTEXT` 兼容旧代码。

验收：

- 新迁移的 command 不直接从 `CONTEXT` 获取数据库和设置。

### 阶段 3：拆分前端大组件

目标：降低前端维护成本。

任务：

- 拆 `ScrollContainer.vue`。
- 拆 `ClipCard.vue`。
- 抽出 API 类型定义，减少 `any`。

验收：

- `ScrollContainer.vue` 主要负责页面组合。
- `ClipCard.vue` 主要负责卡片外壳。
- 文本、图片、文件展示逻辑分离。
- `npm.cmd run build` 通过。

### 阶段 4：拆分剪贴记录核心后端

目标：让剪贴记录成为清晰的业务模块。

任务：

- 把 `ClipRecord` 数据库访问迁移到 `ClipRecordRepository`。
- 新增 `ClipRecordService`。
- 把查询、删除、置顶、获取完整文本迁移到 service。
- command 只调用 service。

验收：

- `clip_record.rs` 逐渐收缩为模型和少量领域方法。
- command 中没有复杂数据库逻辑。

### 阶段 5：收敛本地会话和 VIP 状态

目标：统一 token、用户资料和 VIP 权益的本地存储、缓存失效和实时刷新流程。

任务：

- 建立 `SessionSnapshot` 和 `EntitlementSnapshot`。
- 将 token 与用户资料存储收口到 `SessionStore`。
- 将 VIP 本地缓存收口到 `EntitlementStore`。
- 在统一 HTTP 客户端中处理 token 过期、单飞刷新和单次重试。
- 将 VIP 过期、支付成功和实时权益事件收口到同一个刷新入口。
- 删除业务模块中重复的 token/VIP 刷新和缓存更新逻辑。

验收：

- 并发请求遇到 token 过期时只发起一次 refresh 请求。
- token 和 VIP 状态更新都先完成本地原子提交，再通知前端和后台任务。
- 任一 HTTP API 不再自行实现 token 或 VIP 刷新。
- 短暂认证失败不会永久覆盖用户的云同步偏好。

### 阶段 6：迁移 HTTP + SSE 云同步

目标：使用 HTTP 完成上传和补拉，使用 SSE 完成多客户端实时变更通知，并保证断线后可恢复。

任务：

- 新增 `CloudSyncClient` trait。
- 新增 `SyncEventStream` 和 SSE 默认实现。
- 新增 SQLite outbox、同步游标和删除墓碑存储。
- 登录后执行近期记录快照合并，再从快照游标建立 SSE。
- 小记录走 SSE 内联，大记录收到事件后通过 HTTP 拉取。
- 按 `record_id + version` 去重，按 `created_at` 合并和排序。
- 接入断线重连、事件重放、心跳超时和指数退避。

验收：

- 本地新增记录即使断网或退出程序也不会丢失待上传任务。
- 多个客户端可以实时收到新增、更新和删除事件。
- SSE 断开期间的事件可以通过游标重放或快照补拉恢复。
- 重复事件和本机回环事件不会产生重复记录。
- token 刷新后 SSE 使用新 token 自动重连。

## 10. 设计模式学习点

### 10.1 Repository

用于隔离数据库访问。

适合：

- `ClipRecordRepository`
- `SettingsRepository`
- `SyncTimeRepository`

价值：

- 业务服务不关心 SQL 和 RBatis。
- 后续改数据库或改 SQL 不影响 command。

### 10.2 Strategy

用于可替换算法。

适合：

- 搜索实现：Bloom Filter、SQLite FTS、Tantivy。
- 同步冲突策略：本地优先、云端优先、时间优先。
- VIP 权益策略：免费、月度、季度、年度。

价值：

- 新增方案时不改调用方。

### 10.3 Adapter

用于封装外部库或系统 API。

适合：

- Tauri 剪贴板插件。
- HTTP 客户端。
- 系统文件选择器。
- 开机自启插件。

价值：

- 外部 API 变化时，只改适配器。

### 10.4 Facade

用于给复杂子系统提供简单入口。

适合：

- `ClipboardCopyService`
- `CloudSyncService`
- `SettingsService`

价值：

- command 层只调用一个清晰方法。

### 10.5 Observer / Event Bus

用于事件通知。

适合：

- 剪贴板变更通知前端。
- 同步状态更新。
- 登录过期通知。
- VIP 状态变化通知。

价值：

- 业务流程和 UI 通知解耦。

### 10.6 Dependency Injection

用于显式传递依赖。

适合：

- `AppContext`
- service 构造函数
- trait object 注入

价值：

- 依赖关系清楚。
- 便于测试。

## 11. 代码规范建议

### 11.1 后端

- `commands` 中不要写复杂业务，只做参数、调用、响应。
- `services` 中写业务流程。
- `domain` 中写业务概念、枚举、规则。
- `infra` 中写 SQLite、HTTP、文件系统、加密等技术实现。
- 不再新增裸 `String` 错误，统一转为 `AppError`。
- 新增状态值优先使用 enum，不使用裸 `i32` 魔法值。
- 新增前后端 DTO 放到 `dto` 或业务模块的 `dto.rs` 中。

### 11.2 前端

- API 类型放到 `types`。
- API 调用放到 `api`。
- 页面状态逻辑放到 `composables`。
- 组件只负责展示和触发事件。
- 大组件超过约 500 行时优先考虑拆分。
- 尽量减少 `any`，尤其是 command 返回值。

## 12. 风险控制

重构时遵守以下规则：

- 每次只重构一个边界，不同时大改前后端多处核心逻辑。
- 每个阶段结束都运行：
  - `cargo check --workspace`
  - `cd frontend && npm.cmd run build`
- 不在重构阶段顺手改 UI 风格或新增业务功能。
- 保留旧接口兼容一段时间，新接口跑通后再删除旧接口。
- 对搜索、同步、VIP、设置保存这类高风险模块，先补最小测试再重构。

## 13. 推荐的第一步

建议从“统一前后端响应格式”开始。

原因：

- 它是前后端边界问题，影响面大但可以渐进迁移。
- 做完以后，后端 service 怎么拆，前端 API 层都能保持稳定。
- 能立即改善错误处理和代码可读性。

第一步具体任务：

1. 新增 `src-tauri/src/response.rs`。
2. 新增统一 `CommandResponse<T>`。
3. 新增 `command_result` 辅助函数。
4. 前端新增统一 `ApiResponse<T>` 类型。
5. 迁移 `get_clip_records` 作为第一个试点。

完成这一小步后，再迁移复制、设置保存、登录、VIP 状态等 command。

# ClipPal 项目提示词大纲

## 1. 项目定位

- ClipPal 是一个基于 Tauri 2 + Vue 3 的桌面剪贴板管理器。
- 后端使用 Rust，负责剪贴板监听、SQLite 存储、搜索索引、自动粘贴、托盘、窗口、全局快捷键、云同步、用户登录、VIP/支付和软件更新。
- 前端使用 Vue 3 + Vite，负责剪贴板列表、内容展示、设置、登录、VIP、支付和更新弹窗等界面。

## 2. 项目结构

- `Cargo.toml`：Rust workspace 根配置。
- `src-tauri`：Tauri 打包壳，仅保留 main/lib 启动入口、build.rs、配置、权限和图标。
- `crates/desktop-tauri`：独立桌面适配 crate，承载 IPC、系统能力、AppContext 和生命周期；可选同步/商业实现按 feature 隔离。
- `crates/clip-core`：本地域模型、仓储、SQLite、搜索与设置读写。
- `crates/clip-sync-protocol`、`crates/clip-sync-client`：共享同步契约与客户端基础实现。
- `crates/desktop-app-core`：单机记录采集、查询、内容转换、缓存清理、设置校验和应用资源；`crates/selfhost-app-core` 在此基础上扩展同步。
- 纯业务公共 crate 不得依赖 Tauri、`src-tauri` 或商业实现。`desktop-tauri` 是框架适配例外，但不得反向依赖或读取 `src-tauri` 路径；打包 context、配置和图标由启动壳注入。
- `tauri-plugin-clipboard-pal`：自定义剪贴板 Tauri 插件。
- `clipboard-listener`：剪贴板事件监听库。
- `frontend`：Vue 3 前端项目。
- `assets`：README 和展示用资源。

## 3. 常用命令

- 前端构建：在 `frontend` 目录运行 `npm.cmd run build`。
- Rust 检查：在项目根目录运行 `cargo check --workspace`。
- 本地启动：在项目根目录运行 `cargo tauri dev`。
- PowerShell 下如果 `npm run build` 被脚本策略拦截，使用 `npm.cmd run build`。

## 4. 关键入口

- 打包启动：`src-tauri/src/lib.rs`；实际桌面启动：`crates/desktop-tauri/src/lib.rs`；Tauri command 注册：`crates/desktop-tauri/src/commands/mod.rs`。
- SQLite 初始化和表结构迁移：`crates/clip-core/src/storage/schema.rs`。
- 剪贴板记录模型：`crates/clip-core/src/domain/clip/clip_record.rs`。
- 剪贴板记录查询与展示编排：`crates/desktop-app-core/src/records.rs`；Tauri 扩展副作用在 `crates/desktop-tauri/src/core/application/clip_record_service.rs`。
- 搜索索引：`crates/clip-core/src/search/in_memory.rs`，通过注入的解码策略解密内容。
- 剪贴板监听初始化：`crates/desktop-tauri/src/desktop/listener.rs`；事件适配在 `core/application/capture_listener.rs`，实际采集在 `crates/desktop-app-core/src/capture.rs`。
- 前端根组件：`frontend/src/App.vue`。
- 主列表组件：`frontend/src/components/ScrollContainer.vue`。
- 前端 Tauri API 封装：`frontend/src/utils/api.ts`。

## 5. 开发约定

- 优先小范围修改，避免无关改动。
- 不要覆盖用户已有改动。
- 不要删除现有中文注释和文档，除非用户明确要求。
- 修改后端或依赖后，运行 `cargo check --workspace`。
- 修改前端后，运行 `npm.cmd run build`。
- 长时间运行 `cargo tauri dev` 只用于必要验证，验证后清理测试进程。
- 明确一个点：我的这个项目的 Rust 后端（`crates/clip-core`、`crates/desktop-app-core`、`crates/desktop-tauri` 等）主要用来处理软件内部逻辑，就像java服务端处理的逻辑类似，而frontend目录下是前端所有的代码，主要是软件的页面显示和交互内容，在处理任务时要尽量区分前后端这个概念，该前端交互显示相关的写在frontend的前端代码中，关于业务逻辑、系统设置等等内容的时候，要写到对应 Rust 后端 crate 中，`src-tauri` 仅保留打包启动入口，这是一个大规范
- 前端代码在开发时，能使用流行切可靠的开源框架就尽量使用框架，要使用最合适的、性能最好、影响最小的框架，尽量不要手动去写各种框架已经实现的内容，要合理的使用自己的前端实现和已有开源框架，也不是说完全就使用开源框架，只是优先级高一点

## 6. 排查流程

- 先用最小命令复现问题。
- 区分问题来源：源码、依赖解析、本地环境、构建产物或运行时状态。
- 对启动问题，优先跑 `cargo check --workspace`，确认是否为 Rust 编译问题。
- 对前端问题，优先跑 `npm.cmd run build`，确认是否为 TypeScript/Vite 问题。
- 对 Tauri 运行问题，再执行 `cargo tauri dev` 查看完整启动链路。

## 7. 前端补充

- Vite 输出目录是 `../dist`，位于 `frontend` 目录外。
- 构建时出现 `outDir is not inside project root and will not be emptied` 是已知提示。
- 全局样式主要在 `frontend/src/assets/styles/global.css`。

## 8. 协作偏好

- 回答要直接说明结论、原因和下一步。
- 修复问题时尽量给出已验证的结果。
- 如果不能验证，要明确说明未验证的部分。
- 对依赖或环境问题，优先解释为什么“以前能跑，现在突然不能跑”。

## 9. 代码内部遵守的约定
- 应用内的上下文相关数据使用AppContext，保持简洁

## 10. 当前单机优先阶段

- 默认 Cargo feature 为 `local`。先完善单机版，云同步、商业扩展和 AI 暂不扩展。
- 本地业务在 `crates/desktop-app-core`、`crates/clip-core`，桌面系统适配在 `crates/desktop-tauri/src/desktop`，IPC 编排在 `core/application` 与 `commands`。这仍属于后端，前端只处理显示和交互。
- 旧 `biz/services/infra/domain/dto/utils/api/system` 目录和散落的托盘/窗口入口已经删除，不新增兼容转发层。
- 单机删除直接清理数据库、搜索索引和受管缓存，不等待同步确认；不得删除用户原始文件。
- 结构检查：`python3 scripts/check-v2-structure.py`。迁移测试使用临时数据库，禁止测试访问用户正式数据。

- `src-tauri/src` 只允许 `main.rs` / `lib.rs`，不得重新添加业务、IPC、系统适配或生命周期模块。
- 打包壳内的插件直接依赖用于 Tauri 构建权限元数据，不代表插件实现归属启动壳；不要删除这些声明导致 capability 失效。
- 桌面适配测试使用 `cargo test -p desktop-tauri --lib`；自建/商业测试追加 `--no-default-features --features self-hosted/commercial`。仅测试 ClipPal 启动壳不能替代适配测试。

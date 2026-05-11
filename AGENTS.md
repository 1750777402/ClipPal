# ClipPal 项目提示词大纲

## 1. 项目定位

- ClipPal 是一个基于 Tauri 2 + Vue 3 的桌面剪贴板管理器。
- 后端使用 Rust，负责剪贴板监听、SQLite 存储、搜索索引、自动粘贴、托盘、窗口、全局快捷键、云同步、用户登录、VIP/支付和软件更新。
- 前端使用 Vue 3 + Vite，负责剪贴板列表、内容展示、设置、登录、VIP、支付和更新弹窗等界面。

## 2. 项目结构

- `Cargo.toml`：Rust workspace 根配置。
- `src-tauri`：主 Tauri 应用。
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

- 后端启动和 Tauri command 注册：`src-tauri/src/lib.rs`。
- SQLite 初始化和表结构迁移：`src-tauri/src/sqlite_storage.rs`。
- 剪贴板记录模型：`src-tauri/src/biz/clip_record.rs`。
- 剪贴板记录查询：`src-tauri/src/biz/query_clip_record.rs`。
- 搜索索引：`src-tauri/src/biz/content_search.rs`。
- 剪贴板监听初始化：`src-tauri/src/clip_board_listener.rs`。
- 前端根组件：`frontend/src/App.vue`。
- 主列表组件：`frontend/src/components/ScrollContainer.vue`。
- 前端 Tauri API 封装：`frontend/src/utils/api.ts`。

## 5. 开发约定

- 优先小范围修改，避免无关重构。
- 不要覆盖用户已有改动。
- 不要删除现有中文注释和文档，除非用户明确要求。
- 遵循当前 Rust/Vue 写法，不轻易引入新框架或大抽象。
- 修改后端或依赖后，运行 `cargo check --workspace`。
- 修改前端后，运行 `npm.cmd run build`。
- 长时间运行 `cargo tauri dev` 只用于必要验证，验证后清理测试进程。
- 明确一个点：我的这个项目的src-tauri中的rust代码主要用来处理软件内部逻辑，就像java服务端处理的逻辑类似，而frontend目录下是前端所有的代码，主要是软件的页面显示和交互内容，在处理任务时要尽量区分前后端这个概念，该前端交互显示相关的写在frontend的前端代码中，关于业务逻辑、系统设置等等内容的时候，要写到src-tauri中的rust代码中，这是一个大规范
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

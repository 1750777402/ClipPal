# 单机数据库 v2

本文件描述当前实施结果。数据库仍位于原数据目录的 clip_record.db；文本加密格式、
密钥来源、主界面 CSS、自动粘贴和辅助功能授权流程沿用原实现。

## 表和身份

SQLite user_version 为 2，新建库只有 clip_record、clip_record_resource 两张业务表，
SQLite 自己的 sqlite_sequence 等系统表不计入业务表。

clip_record：

| 字段 | 说明 |
| --- | --- |
| id | INTEGER PRIMARY KEY AUTOINCREMENT，本地操作和外键关联标识；删除后不复用 |
| content_key | TEXT NOT NULL UNIQUE，版本化 SHA-256 内容身份，不作外键 |
| type | text/image/files/directory，小写核心类型 |
| content | 文本保存完整 AES-GCM 密文，其他类型为 NULL |
| created_at | 首次入库毫秒时间，重复复制保留 |
| last_copied_at | 最近一次复制毫秒时间 |
| sort_order | 64 位本地顺序，在写事务内分配，不依赖系统时间先后 |
| pinned | 0/1，部分唯一索引保证最多一个置顶 |

clip_record_resource：

| 字段 | 说明 |
| --- | --- |
| id | INTEGER PRIMARY KEY AUTOINCREMENT，精确选择同名资源的本地标识 |
| record_id | 关联 clip_record.id，ON DELETE CASCADE |
| position | 从 0 开始的资源顺序，与 record_id 联合唯一 |
| display_name | 原始显示名称，用于文件恢复和页面展示 |
| resource_kind | image/file/directory |
| storage_kind | managed 是应用缓存副本，reference 是用户原件引用 |
| path | managed 使用资源根目录相对路径，reference 使用绝对路径 |
| byte_size | 文件字节数，目录或缺失资源为 NULL |
| content_hash | 完整资源 SHA-256，目录或缺失旧资源为 NULL |

每个 SQLite 池连接均启用 foreign_keys；驱动采用 WAL 和同步写入策略。记录和资源更新、
置顶互斥、删除和超限清理都使用事务。失败回滚，不会留下半套资源关系。

Rust 实体分别为 `domain/clip/clip_record.rs` 中的 `ClipRecord` 与
`domain/clip/clip_record_resource.rs` 中的 `ClipRecordResource`，字段与表逐一对应。
`ClipRecord.resources` 是仓储加载的有序子实体集合，不是主表列。资源归记录聚合管理，
由 `SqliteClipRecordRepository` 在同一写事务维护，不开放绕过记录的资源独立增删。
读主表与资源表使用同一事务快照；资源分批加载，避免历史数量增大后超出参数限制。

数据库 ID 为 i64；IPC 中记录 ID 和资源 ID 以十进制字符串传输，避免 JavaScript
number 丢失大整数精度。前端仍使用 Text/Image/File 展示类型和既有样式；目录通过 File
卡片展示。单文件复制参数从 file_path 改为 resource_id，不按名称猜测对应文件。

## 内容唯一规则

- 文本：text:SHA256(完整原文 UTF-8)。计算在加密前，保留空格、缩进和换行；
  仅空字符串不采集，纯空白字符串可记录。
- 图片：image:SHA256(实际图片文件全部字节)，不进行视觉近似去重。
- 文件：files:SHA256(确定性 JSON 清单)。清单项包含名称、资源类型、完整内容摘要；
  单文件忽略名称，允许相同内容以新名称合并。多资源清单按值排序，保留重复项。
- 目录：directory:SHA256(规范化绝对路径)，仅表示本机引用，不代表跨设备目录身份。

大文件流式计算完整 SHA-256，不再使用采样摘要作为身份。文件准备在阻塞工作线程执行；
先复制后对副本计算摘要，保证保存的字节与内容身份一致。普通文件尽可能建立 managed
快照，复制失败退回 reference；目录始终为 reference，多资源中每项明确保存所有权。

重复复制按 content_key 更新原记录，保留 id、created_at、pinned，更新最近时间、顺序
和资源清单，提交成功后回收替换掉的旧受管文件。图片可复用仍存在的缓存。

## 本地业务闭环

采集 → 完整摘要 → 文本加密/文件快照 → 记录和资源事务 → 搜索索引 → IPC 轻量预览 →
文本解密/图片读取/文件名称恢复 → 系统剪贴板 → 可选自动粘贴。

查询、搜索、置顶、删除、另存为和超限清理均使用新表。单机删除直接删除记录及资源行，
提交后移除索引和受管缓存；reference 原件、目录和资源根外的文件永不随记录删除。
缓存路径解析拒绝 ..、绝对 managed 路径和符号链接越界。

## 删除策略

单机版暂不增加 `deleted` / `deleted_at` 或回收站。用户删除、超限淘汰均物理删除主表，
外键级联清理资源表，再移除索引并回收受管文件；外部原件保留。重复删除是幂等操作，
再次复制相同内容会创建新的本地 ID。索引清理失败记录日志，不能阻断已经提交的删除和
其余缓存回收；查询回查数据库，不会显示已删除记录。缓存删除失败记录日志，不撤销
已提交的数据库删除；进程崩溃后的孤立缓存扫描回收另行完善。

当前没有撤销删除、回收站或审计需求，逻辑删除会持续保存剪贴板敏感内容，并要求所有
查询、唯一键、容量计算和缓存清理额外处理删除状态，因此没有必要加入核心两表。
物理删除是业务层删除，不承诺 SQLite 页、WAL 或磁盘上的不可恢复擦除。

自建版将显式跨端删除和新增统一为待上传动作。跨端删除与动作入队在同一 SQLite 事务
提交，动作通过 `content_key` 定位，并保留到云端确认。本机超限淘汰仅清理本机。
若以后引入回收站，再单独设计恢复、保留时长和敏感缓存处置。动作的顺序、重试与跨端
回放见 [云同步设计](cloud-sync-design.md)。

## 开发库初始化

首版开发阶段直接使用当前 SQLite 表结构。客户端启动会创建缺失的表。结构变更时使用新开发库。

## 同步与商业边界

clip-core 和 desktop-app-core 不再定义同步字段、跳过原因、同步仓储和云同步设置。
`clip-sync-protocol` 定义统一的云同步请求与响应，独立于本地记录实体。

当前 self-hosted/commercial feature 继续可以编译，但新的两表结构尚未接入同步扩展，
单机版和未启用同步的安装包关闭 `capabilities.cloud_sync`。自建版显式启用 V2
同步后，由独立扩展表保存待传动作、来源和游标。旧同步任务仍保持停用。

前端默认 local，商业 UI 通过 --mode commercial 才进入产物。Vite 清理旧生成文件，
避免之前商业构建的脚本留在单机安装包。单机打包配置只嵌入本地加密配置，不嵌入云端配置。

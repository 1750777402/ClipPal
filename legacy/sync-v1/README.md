# v1 同步实现归档

本目录保留单机数据库拆分前的同步模型、SQLite 操作、同步运行时及兼容内容转换代码。
源码用于后续协议迁移参考，不属于 Cargo workspace，不编译、不打包、不运行。

旧运行时使用 UUID 主键和 clip_record 内的 sync_flag / sync_time / skip_type 等字段，
不能用于 v2 本地两表结构。当前自建/商业 feature 可编译，但同步能力返回 false，
不会启动旧任务；未来通过独立扩展表和 content_key 重新接入。

单机核心的历史表迁移仅在 desktop-app-core/src/migration.rs 中识别旧字段；
迁移前完整备份旧库，之后本地活动数据库只保留本地记录、资源及 SQLite 系统表。

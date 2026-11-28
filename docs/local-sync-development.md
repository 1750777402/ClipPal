# 本机桌面客户端与 Java 服务端联调

默认的 `cargo tauri dev` 使用 Cargo 的 `local` feature，只运行单机功能。需要明确启动自建版，开启同步配置，并保证两端的部署令牌和内容密钥匹配。

本机开发链路为：

```text
Tauri 自建版调试客户端 → http://127.0.0.1:8080 → Spring Boot 内嵌 Tomcat → MySQL
```

生产部署仍采用客户端 HTTPS → Nginx → HTTP Tomcat。只有调试构建额外允许 `127.0.0.1` 等回环 IP、`localhost` 的 HTTP，局域网和公网 HTTP 会被拒绝。正式 release 构建保持 HTTPS 要求。

客户端和服务端使用同一套 `/sync/*` 接口。两端应使用同一批源码构建。需要清理旧格式数据时，可重新初始化 SQLite 和本地 MySQL 的 `clip_pal_sync`。

## 1. 在 IDEA 启动服务端

打开同级 `clip-pal-sync` 工程，JDK 21，选择已配置的 `ClipPalSyncApplication` 或 `ClipPal Sync Local`。

本机启动参数为：

```text
--spring.profiles.active=local --spring.config.additional-location=file:$PROJECT_DIR$/config/
```

IDEA 将 `$PROJECT_DIR$` 展开为服务端工程目录。服务端通过被 Git 忽略的 `config/application-local.yml` 加载固定部署令牌和内容密钥。MySQL `mysql84` 容器需要处于运行状态。本机日常联调固定使用 `clip_pal_sync`，首次初始化或重置时在该库执行 `sql/init.sql`。

服务应在 `http://127.0.0.1:8080/actuator/health` 返回 `UP`。本机 Tomcat 不配置 TLS，也不需要 Nginx。

## 2. 配置客户端

`src-tauri/config.json` 是被 Git 忽略的本机打包配置，已有 `app_secret` 必须保留。本轮已将开发配置指向本机，并与服务端已有的令牌、内容密钥核对一致。

需要关注的同步字段：

```json
{
  "cloud_sync": {
    "enabled": true,
    "domain": "http://127.0.0.1:8080",
    "deployment_token": "填写与服务端相同的部署令牌"
  }
}
```

上面只展示同步部分，不能用它覆盖整个配置文件。`deployment_token` 对应服务端 `clip-pal.security.deployment-token`。`app_secret.secret_key` 经过字符混淆，服务端 `clip-pal.sync.content-key` 必须使用还原后的真实32字节密钥；本轮没有修改已有内容密钥。

配置在编译时嵌入。修改后要重新编译启动客户端，运行中的旧单机进程不会自动变成自建版。

## 3. 启动自建版桌面客户端

先在旧终端按 `Ctrl+C` 停止原有开发进程，并确认托盘中的旧 ClipPal 已退出。两个版本共用单实例标识。旧进程还在时，新进程可能只唤起旧单机界面。旧的 Vite 开发服务器也应随旧终端结束，释放 1420 端口。

在 macOS 终端或 Windows PowerShell 中，从客户端仓库根目录启动。

```bash
cargo dev-sync
```

命令会启动自建版的 Vite 和 Rust 客户端，并使用和安装版相同的 ClipPal 数据根目录。macOS 默认是 `~/Library/Application Support/ClipPal`，Windows 默认是 `%LOCALAPPDATA%\ClipPal\data`。数据库位于根目录下的 `data/clip_record.db`，Windows 完整路径为 `%LOCALAPPDATA%\ClipPal\data\data\clip_record.db`。首次绑定时，符合条件的已有文本会进入同步队列。

也可用 `npm --prefix frontend run dev:self-hosted` 启动相同的自建开发版本。单机开发命令和其余常用命令见 [README 的日常开发](../README.md#日常开发)。

## 4. 手动验证

1. 客户端设置页应出现“文本云同步”，状态从验证部署变为等待同步。
2. 复制一条容易辨认的测试文本，列表中应出现新记录。
3. 用数据库工具查询开发库：

```sql
SELECT id, content_key, type, created, updated, del_flag
FROM clip_record ORDER BY id DESC LIMIT 10;

SELECT id, operation_id, action, content_key, record_id, created
FROM clip_sync_change ORDER BY id DESC LIMIT 20;
```

新增应有一条 `upsert` 动作，日志中的 `id` 就是对外的 `sync_seq`。不要仅用Java控制台有没有SQL判断成功，项目默认关闭SQL参数日志以免泄露内容。

4. 在自建版确认删除一条文本记录，应新增 `delete` 动作，云端记录的 `del_flag` 变为 1。本地容量淘汰只清理当前设备的历史。
5. 停止Java后继续复制，再启动Java，等待重试。记录应补传，客户端上传队列最终清空。

要验证多设备，请使用第二台设备或下方的隔离自动测试；同一份应用的单实例机制不允许直接同时打开两个正常桌面进程。

## 5. 自动验证

用日常配置启动客户端和服务端，确认同步已连接，再从客户端仓库根目录执行。

```bash
python3 scripts/test-cloud-sync.py
```

脚本会向系统剪贴板写入一条带唯一编号的测试文本。它以只读方式查询日常 SQLite，再通过现有服务的变更接口核对 MySQL 提交、游标和队列确认。样例会留在剪贴板和两端记录中。断网补传和跨端删除按上一节验证。

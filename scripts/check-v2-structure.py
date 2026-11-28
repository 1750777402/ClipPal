"""验证 v2 公共包的依赖方向和单机 profile 的同步隔离；不修改源码。"""
import json
from pathlib import Path
import subprocess

ROOT = Path(__file__).resolve().parents[1]


def cargo(*args):
    """在仓库根目录执行只读 Cargo 查询，返回文本结果供后续边界检查。"""
    return subprocess.check_output(["cargo", *args], cwd=ROOT, text=True)


# 先从 Cargo 的解析结果建立依赖图，不靠手工猜测配置文件路径。
metadata = json.loads(cargo("metadata", "--no-deps", "--format-version", "1"))
packages = {package["name"]: package for package in metadata["packages"]}
# 核心只向下依赖，自建版组合单机核心，商业能力不反向进入公共包。
allowed = {
    "clip-core": set(),
    "clip-sync-protocol": set(),
    "clip-sync-client": {"clip-sync-protocol"},
    "desktop-app-core": {"clip-core"},
    "selfhost-app-core": {"desktop-app-core", "clip-sync-client"},
}
for name, dependencies in allowed.items():
    actual = {
        dependency["name"] for dependency in packages[name]["dependencies"]
        if dependency["name"] in packages and dependency["kind"] != "dev"
    }
    if actual != dependencies:
        raise SystemExit(f"{name}: 公共包依赖应为 {dependencies}，实际为 {actual}")
    if any(dependency["name"].startswith("tauri") for dependency in packages[name]["dependencies"]):
        raise SystemExit(f"{name}: 公共核心不可依赖 Tauri")

# 检查 feature 解析后的实际依赖树，避免可选声明意外启用了扩展。
local_tree = cargo("tree", "-p", "ClipPal", "--no-default-features", "--features", "local", "--edges", "normal")
for name in ("selfhost-app-core", "clip-sync-client", "clip-sync-protocol", "tauri-plugin-http", "bincode", "backon"):
    if name in local_tree:
        raise SystemExit(f"local profile 意外引入 {name}")

protocol_tree = cargo("tree", "-p", "clip-sync-protocol", "--edges", "normal")
for name in ("tauri", "rbatis", "rbdc-sqlite", "clipboard-listener"):
    if name in protocol_tree:
        raise SystemExit(f"独立协议包意外引入 {name}")
# 桌面本地/同步实现只访问策略端口；商业逻辑只能留在商业适配器和组装层。
for folder in ("crates/desktop-tauri/src/sync", "crates/desktop-tauri/src/core/application", "crates/desktop-app-core/src"):
    for source in (ROOT / folder).rglob("*.rs"):
        code = source.read_text()
        for forbidden in ("VipService", "token_manager", "crate::commercial", "services::vip_service"):
            if forbidden in code:
                raise SystemExit(f"{source.relative_to(ROOT)}: 业务实现意外依赖商业能力 {forbidden}")
for old in ("biz", "services", "infra", "domain", "dto", "utils", "api", "system", "sqlite_storage.rs", "application", "infrastructure", "interfaces", "platform", "shared", "ai"):
    if (ROOT / "src-tauri/src" / old).exists():
        raise SystemExit(f"旧入口尚未删除: src-tauri/src/{old}")
# 桌面框架包允许依赖 Tauri，但不能反向读取或依赖应用壳。
adapter = packages["desktop-tauri"]
if any(dependency["name"] == "ClipPal" for dependency in adapter["dependencies"]):
    raise SystemExit("desktop-tauri 不可反向依赖打包壳")
for source in (ROOT / "crates/desktop-tauri/src").rglob("*.rs"):
    code = source.read_text()
    for forbidden in ("src-tauri", "CARGO_MANIFEST_DIR", "include_str!", "include_bytes!", "generate_context!"):
        if forbidden in code:
            raise SystemExit(f"{source.relative_to(ROOT)}: 桌面适配包不可读取宿主资产 {forbidden}")
shell = ROOT / "src-tauri/src"
if {path.name for path in shell.iterdir()} != {"main.rs", "lib.rs"}:
    raise SystemExit("src-tauri/src 只应保留 main.rs / lib.rs")
for source in shell.glob("*.rs"):
    code = source.read_text()
    for forbidden in ("#[tauri::command]", "mod ", "clip_core::", "desktop_app_core::", "tauri_plugin_", "bootstrap::", "AppContext"):
        if forbidden in code:
            raise SystemExit(f"{source.relative_to(ROOT)}: 打包壳混入业务或框架适配实现 {forbidden}")
# 除适配包外，打包壳只允许框架启动、宏展开和权限元数据依赖。
shell_allowed = {"desktop-tauri", "tauri", "tauri-build", "serde_json", "tokio", "log",
    "tauri-plugin-opener", "tauri-plugin-dialog", "tauri-plugin-autostart",
    "tauri-plugin-global-shortcut", "tauri-plugin-window-state"}
for dependency in packages["ClipPal"]["dependencies"]:
    if dependency["name"] not in shell_allowed:
        raise SystemExit(f"打包壳包含非启动依赖: {dependency['name']}")
for name in ("ClipPal", "desktop-tauri"):
    if packages[name]["features"]["default"] != ["local"]:
        raise SystemExit(f"{name}: 默认构建必须为单机版 local")
for profile in ("local", "self-hosted", "commercial", "sync", "commercial-extension"):
    if packages["ClipPal"]["features"][profile] != [f"desktop-tauri/{profile}"]:
        raise SystemExit(f"打包壳 feature 未正确转发: {profile}")
for source in (ROOT / "crates/desktop-app-core/src").rglob("*.rs"):
    code = source.read_text()
    for forbidden in ("tauri::", "crate::app_context", "crate::commercial", "clip_sync_client::"):
        if forbidden in code:
            raise SystemExit(f"{source.relative_to(ROOT)}: 单机核心意外依赖宿主或扩展 {forbidden}")
# 单机领域和应用核心与同步状态隔离。
for folder in ("crates/clip-core/src", "crates/desktop-app-core/src"):
    for source in (ROOT / folder).rglob("*.rs"):
        code = source.read_text()
        for forbidden in ("SkipSyncReason", "SyncStatus", "VipLimit", "SKIP_SYNC", "NOT_SYNCHRONIZED",
                          "sync_flag", "cloud_source", "skip_type", "cloud_sync", "retain_tombstones",
                          "list_synced_tombstones", "update_sync_status", "insert_batch_merge"):
            if forbidden in code:
                raise SystemExit(f"{source.relative_to(ROOT)}: 单机核心仍包含同步领域 {forbidden}")
print("v2 依赖方向、单机领域/仓储边界、协议隔离、桌面适配和薄启动壳检查通过")

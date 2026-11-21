"""在前端默认单机构建后检查生成资产，防止商业和同步逻辑混入安装包。"""
from pathlib import Path
import re

# 从脚本自身定位仓库，避免执行时所在目录影响检查范围。
root = Path(__file__).resolve().parents[1]
dist = root / "dist"
index = (dist / "index.html").read_text()
# 读取实际生成的入口文件，不猜测 Vite 带摘要的文件名。
script = re.search(r'src="([^"]+\.js)"', index)
if not script:
    raise SystemExit("缺少前端入口脚本")
code = (dist / script.group(1).lstrip("/")).read_text()
# 用有代表性的命令和事件检测单机入口是否意外引入扩展运行时代码。
for forbidden in ("get_vip_status", "get_pay_url", "user_register", "auth-expired", "sync_status_update", "cloud-sync-disabled"):
    if forbidden in code:
        raise SystemExit(f"单机前端仍包含扩展命令/事件: {forbidden}")
# 入口没引用也不应残留商业分包，否则安装包仍会携带这些资源。
if list((dist / "assets").glob("CommercialAccount*")):
    raise SystemExit("单机生成目录残留商业前端脚本")
print("单机前端入口与生成目录未包含商业/同步命令和旧商业资源")

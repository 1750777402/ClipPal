#!/usr/bin/env python3
"""通过系统剪贴板验证正在运行的自建版客户端和服务端。"""

import base64
import hashlib
import json
import os
from pathlib import Path
import sqlite3
import subprocess
import sys
import time
from urllib import error, parse, request
from uuid import uuid4


ROOT = Path(__file__).resolve().parents[1]


def desktop_database():
    """按桌面客户端的系统路径查找日常使用的 SQLite 数据库。"""
    if sys.platform == "darwin":
        return Path.home() / "Library/Application Support/ClipPal/data/clip_record.db"
    if sys.platform == "win32":
        return Path(os.environ["LOCALAPPDATA"]) / "ClipPal/data/data/clip_record.db"
    raise RuntimeError("剪贴板联调脚本支持 macOS 和 Windows。")


def copy_text(text):
    """从系统剪贴板触发桌面客户端的正式采集入口。"""
    if sys.platform == "darwin":
        subprocess.run(["pbcopy"], input=text.encode("utf-8"), check=True, timeout=5)
    else:
        encoded = base64.b64encode(text.encode("utf-8")).decode("ascii")
        command = f"Set-Clipboard -Value ([Text.Encoding]::UTF8.GetString([Convert]::FromBase64String('{encoded}')))"
        subprocess.run(["powershell.exe", "-NoProfile", "-Command", command], check=True, timeout=5)


class NoRedirect(request.HTTPRedirectHandler):
    """当前部署的令牌只发给配置中的服务地址。"""

    def redirect_request(self, *args, **kwargs):
        return None


def main():
    database = desktop_database()
    if not database.is_file():
        raise RuntimeError("找不到日常 SQLite 数据库，请先运行 cargo dev-sync。")

    sync = json.loads((ROOT / "src-tauri/config.json").read_text(encoding="utf-8"))["cloud_sync"]
    if not sync.get("enabled") or not sync.get("deployment_token"):
        raise RuntimeError("请先启用同步并配置部署令牌。")
    endpoint = sync["domain"].rstrip("/")
    opener = request.build_opener(NoRedirect())

    def get(resource, parameters=None):
        suffix = "?" + parse.urlencode(parameters) if parameters else ""
        req = request.Request(
            f"{endpoint}/sync/{resource}{suffix}",
            headers={"Authorization": f"Bearer {sync['deployment_token']}"},
        )
        with opener.open(req, timeout=5) as response:
            return json.load(response)

    with sqlite3.connect(database.as_uri() + "?mode=ro", uri=True) as db:
        state = db.execute("SELECT pull_cursor FROM clip_sync_state WHERE id = 1").fetchone()
        if state is None:
            raise RuntimeError("客户端还未初始化云同步，请先在设置中确认同步状态。")
        (after,) = state

        text = f"ClipPal 云同步联调 {uuid4().hex[:12]}"
        content_key = "text:" + hashlib.sha256(text.encode("utf-8")).hexdigest()
        copy_text(text)
        print(f"已写入系统剪贴板：{text}", flush=True)

        # 服务端变更接口读真实 MySQL。客户端落库和队列确认则直接查本机 SQLite。
        deadline = time.monotonic() + 45
        change = None
        local = None
        while time.monotonic() < deadline:
            if change is None:
                page = get("changes", {"after": after, "limit": 100})
                for item in page["changes"]:
                    after = item["sync_seq"]
                    if item["content_key"] == content_key and item["action"] == "upsert":
                        change = item
                        break
            local = db.execute(
                "SELECT id, content FROM clip_record WHERE content_key = ? AND type = 'text'",
                (content_key,),
            ).fetchone()
            if change and local and local[1] == change["ciphertext"]:
                cursor = db.execute("SELECT pull_cursor FROM clip_sync_state WHERE id = 1").fetchone()
                pending = db.execute(
                    "SELECT 1 FROM clip_sync_record WHERE content_key = ? AND upload_status = 0 LIMIT 1",
                    (content_key,),
                ).fetchone()
                confirmed = db.execute(
                    "SELECT 1 FROM clip_sync_record WHERE record_id = ? AND upload_status = 1",
                    (local[0],),
                ).fetchone()
                if cursor and cursor[0] >= change["sync_seq"] and pending is None and confirmed:
                    print(f"通过：SQLite 记录 {local[0]}，MySQL 同步序号 {change['sync_seq']}，上传队列已确认。")
                    return
            if change is not None or not page["has_more"]:
                time.sleep(0.5)
        raise RuntimeError(f"等待同步超时。SQLite 落库：{bool(local)}，MySQL 已提交：{bool(change)}。")


if __name__ == "__main__":
    try:
        main()
    except error.HTTPError as exc:
        sys.exit(f"同步接口返回 HTTP {exc.code}，请检查本机服务和部署令牌。")
    except sqlite3.Error as exc:
        sys.exit(f"读取 SQLite 失败，请确认自建版已启动并开启同步：{exc}")
    except (OSError, ValueError, KeyError, RuntimeError, subprocess.SubprocessError) as exc:
        sys.exit(f"联调失败：{exc}")

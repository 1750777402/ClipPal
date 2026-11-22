# 自动更新配置

支持自动更新的安装包会在启动后检查一次版本，也可在「设置 → 检查更新」手动检查。发现新版后，用户确认才会下载，签名通过后才会安装。个人包默认使用 [`src-tauri/tauri.personal.json`](../src-tauri/tauri.personal.json)，其中 `endpoints` 为空，`createUpdaterArtifacts` 为 `false`，应用会提示手动安装新版。

搭建自己的更新渠道只需要能提供 HTTPS 静态文件的站点或发布页。客户端内置公钥和更新清单地址，发布者使用对应私钥签名。软件更新和剪贴板云同步各自配置，单机版也能使用自己的更新源。

## 1. 准备 Tauri CLI 和更新密钥

先按 [README 的打包说明](../README.md#从零打包桌面端) 准备构建环境并安装依赖。以下命令从仓库根目录运行。已安装 Tauri CLI 2.9.4 时可直接使用。

```sh
cargo install tauri-cli --version 2.9.4 --locked
```

**新渠道只生成一次密钥。** 已发布的渠道继续使用与已安装客户端公钥配对的私钥。官方渠道的公钥和清单地址已写在 [`src-tauri/tauri.conf.json`](../src-tauri/tauri.conf.json)。

在 macOS 终端生成密钥。

```sh
mkdir -p "$HOME/.tauri"
cargo tauri signer generate -w "$HOME/.tauri/clippal-update.key"
```

在 Windows PowerShell 生成密钥。

```powershell
New-Item -ItemType Directory -Force "$HOME/.tauri" | Out-Null
cargo tauri signer generate -w "$HOME/.tauri/clippal-update.key"
```

- `clippal-update.key.pub` 是公钥文件，复制其完整文本到客户端配置的 `pubkey`。
- `clippal-update.key` 是私钥文件，和生成时设置的密码一起保存在安全位置。两者只用于构建或 CI，避免提交到仓库。
- 私钥遗失或更换后，已安装客户端仍只信任原公钥。若无法继续使用原私钥，用户需手动安装内置新公钥的版本，才能接收新渠道的更新。

Tauri 更新签名用于验证下载的更新包。公开分发时，按平台另行配置 macOS Developer ID 签名与公证、Windows Authenticode 签名。 [Tauri 更新文档](https://v2.tauri.app/plugin/updater/) · [Tauri 分发文档](https://v2.tauri.app/distribute/)

## 2. 配置更新清单地址和公钥

自建渠道在 `src-tauri` 下创建 `tauri.release.json`，用 `.pub` 文件内容和自己的 HTTPS 地址替换示例占位值。地址要指向客户端下载时能直接读取的 `latest.json`。从第一版开始就用同一个渠道地址构建安装包，后续发布时更新该地址上的文件内容。

```json
{
  "bundle": {
    "createUpdaterArtifacts": true
  },
  "plugins": {
    "updater": {
      "pubkey": "替换为 clippal-update.key.pub 的完整文本",
      "endpoints": ["https://updates.example.com/clippal/local/latest.json"]
    }
  }
}
```

`createUpdaterArtifacts` 决定打包时是否生成更新产物，`endpoints` 决定客户端去哪里查版本。单机、自建和商业构建使用各自的更新清单地址，只在该地址发布对应类型的包，避免升级时切换版本类型。自建预览版仍只提供本地剪贴板功能。

## 3. 提高版本号并构建签名产物

将 `src-tauri/tauri.conf.json` 中的 `version` 提高到比已安装版本更新的语义版本，例如从 `1.0.7` 提高到 `2.0.0`。同步 `Cargo.toml` 的 workspace 应用版本和 `frontend/package.json` 的版本，并更新锁文件。`latest.json` 要填写最终安装包中的版本号，Git 分支名或标签不影响检查结果。

修改版本号后，从仓库根目录执行 `cargo update --workspace` 和 `npm --prefix frontend install --package-lock-only`。Windows PowerShell 使用 `npm.cmd`。

以下命令构建启用自有更新源的单机发布包。官方发布沿用仓库中的更新源和公钥时，去掉 `--config src-tauri/tauri.release.json`，并使用官方渠道对应的私钥。个人打包命令默认使用手动更新配置。

macOS 默认终端使用 zsh，可以直接运行以下命令。

```zsh
export TAURI_SIGNING_PRIVATE_KEY="$HOME/.tauri/clippal-update.key"
read -rs 'TAURI_SIGNING_PRIVATE_KEY_PASSWORD?请输入更新私钥密码：'
echo
export TAURI_SIGNING_PRIVATE_KEY_PASSWORD
VITE_EDITION=local npm --prefix frontend run build && \
  cargo tauri build --config src-tauri/tauri.release.json --features local -- --no-default-features --locked
unset TAURI_SIGNING_PRIVATE_KEY TAURI_SIGNING_PRIVATE_KEY_PASSWORD
```

在 Windows PowerShell 运行以下命令。

```powershell
$env:TAURI_SIGNING_PRIVATE_KEY = "$HOME/.tauri/clippal-update.key"
$clippalSigningPassword = Read-Host "请输入更新私钥密码" -AsSecureString
$env:TAURI_SIGNING_PRIVATE_KEY_PASSWORD = [System.Net.NetworkCredential]::new("", $clippalSigningPassword).Password
$env:VITE_EDITION = "local"
try {
    npm.cmd --prefix frontend run build
    if ($LASTEXITCODE -ne 0) { throw "前端构建失败" }
    cargo tauri build --config src-tauri/tauri.release.json --features local -- --no-default-features --locked
    if ($LASTEXITCODE -ne 0) { throw "安装包构建失败" }
} finally {
    Remove-Item Env:TAURI_SIGNING_PRIVATE_KEY, Env:TAURI_SIGNING_PRIVATE_KEY_PASSWORD, Env:VITE_EDITION
    $clippalSigningPassword.Dispose()
}
```

自建桌面预览若要连自己的更新源，先将配置中的地址改成自建渠道，例如 `https://updates.example.com/clippal/self-hosted/latest.json`。将构建命令中的 `VITE_EDITION=local`（PowerShell 为 `"local"`）和 `--features local` 改成对应的 `self-hosted`，并保留 `--no-default-features`。每个渠道分别归档产物。

## 4. 上传产物，再发布 `latest.json`

按目标系统和架构构建，使用打包命令实际输出的文件名。安装包供首次安装使用，签名更新包供应用内更新使用。

| 平台 | `platforms` 键 | `url` 指向的更新文件 |
| --- | --- | --- |
| Apple Silicon Mac | `darwin-aarch64` | `bundle/macos/` 下的 `.app.tar.gz`，其 `.sig` 在同一目录 |
| Intel Mac | `darwin-x86_64` | Intel 构建生成的 `.app.tar.gz` 和对应 `.sig` |
| Windows x64 | `windows-x86_64` | `bundle/nsis/` 的 `.exe` 或 `bundle/msi/` 的 `.msi`，使用所选文件对应的 `.sig` |

以下示例只发布 Apple Silicon Mac。将 `url` 换成上传后的更新包直链，将 `signature` 换成对应 `.sig` 文件的完整文本。其他平台按同样方式添加，填入该平台实际发布的文件和签名。

```json
{
  "version": "2.0.0",
  "notes": "修复开机自启时弹出主窗口的问题。",
  "pub_date": "2026-11-22T10:00:00+08:00",
  "platforms": {
    "darwin-aarch64": {
      "url": "https://updates.example.com/clippal/local/2.0.0/ClipPal.app.tar.gz",
      "signature": "替换为该更新包的 .sig 文件内容"
    }
  }
}
```

`pub_date` 使用 RFC 3339 时间格式。macOS 手动安装继续提供 `.dmg`，更新清单引用 `.app.tar.gz`。每个 `signature` 与 `url` 对应到同一份构建产物，签名后保持文件内容原样。仓库根目录的 [`latest.json`](../latest.json) 可作为现有渠道格式参考，客户端读取的是 `endpoints` 中的线上地址。

先上传带版本号的安装和更新产物，验证 HTTPS 直链能下载，再把清单发布到 `endpoints` 指向的固定位置。正式发布前，在测试渠道用前一版安装包走一遍「检查更新 → 确认 → 下载和验签 → 安装 → 重启」，核对版本号及原有剪贴板历史。官方发布继续保留上一版产物和清单，便于定位发布问题。

| 更新界面现象 | 检查位置 |
| --- | --- |
| 提示手动更新 | 确认已安装的是启用更新源的发布包。macOS 应从安装后的 `.app` 运行。 |
| 已是最新版本 | 检查线上清单 `version` 是否比已安装版本更新，确认地址上的文件已经刷新。 |
| 暂无适用安装包 | 检查 `platforms` 是否包含当前系统和运行架构。 |
| 签名校验失败 | 检查客户端公钥是否与构建私钥配对，确认 `.sig` 和实际下载的文件相对应。 |

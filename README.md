# ClipPal

ClipPal 是基于 Tauri 2 和 Vue 3 的桌面剪贴板管理器。它在本机记录复制过的文本、图片、文件和目录，支持搜索、去重、置顶、删除、快捷键唤起、图片另存为和自动粘贴。文本以加密形式保存在本地 SQLite，图片和文件资源由本机管理。

## 选择版本

| 版本 | 适合谁 | `v2.0.0` 分支当前状态 |
| --- | --- | --- |
| 单机版 | 只在一台电脑使用 | 默认版本，安装后即可使用本地剪贴板历史，无需账号或服务端。 |
| 自建版桌面预览 | 想提前验证自建版本的桌面构建 | 可以打包、安装并使用单机功能。新版同步协议和服务端仍待接入，预览包当前仅提供本地使用。 |

**只想使用 ClipPal**，可在发布页下载与你系统和架构匹配的现成安装包。发布页里的旧版功能以对应版本说明为准。**想自行生成安装包**，请下载 `v2.0.0` 分支源码，从下文第 1 步开始。 [Gitee 发布页](https://gitee.com/ygz123/clip-pal/releases)

## 从零打包桌面端

### 1. 安装构建工具

在目标系统上构建该系统的安装包。首次安装工具后，关闭并重新打开终端，让新命令加入 `PATH`。

1. 安装 Node.js LTS（推荐 22.12+ 或 24 LTS，附带 npm）、Rust（通过 rustup 安装或更新稳定版）和 Git。用 ZIP 下载源码时可跳过 Git。 [Node.js](https://nodejs.org/) · [Rust](https://rustup.rs/) · [Git](https://git-scm.com/downloads)
2. 按平台补装系统工具。

| 系统 | 准备工作 |
| --- | --- |
| macOS | 打开「终端」执行 `xcode-select --install`，按提示安装 Command Line Tools。已安装 Xcode 的机器可用 `xcode-select -p` 检查工具路径。Apple Silicon 推荐使用原生 ARM 终端和 Rust 工具链，`uname -m` 应显示 `arm64`。 |
| Windows 10/11 | 在 Visual Studio Installer 安装 Build Tools 的「使用 C++ 的桌面开发」，包括 MSVC 和 Windows SDK。确认已安装 Microsoft Edge WebView2 Runtime，并使用 MSVC 版 Rust。可运行 `rustc -vV` 检查 `host` 是否以 `windows-msvc` 结尾。建议先从 x64 构建开始。 |

系统依赖安装页面见 [Tauri 环境要求](https://v2.tauri.app/start/prerequisites/)。当前 README 的安装包流程覆盖 macOS 和 Windows，Linux 暂无验收过的发布流程。

macOS 终端可用以下命令检查工具。

```bash
node --version
npm --version
rustc --version
cargo --version
```

Windows PowerShell 把 `npm --version` 换成 `npm.cmd --version`，其余命令相同。项目通过 npm 安装锁定版本的 Tauri 2 CLI，SQLite 由 Rust 依赖提供。

### 2. 下载源码并安装依赖

用 Git 获取源码。以下命令在 macOS 终端和 Windows PowerShell 都可以执行。

```bash
git clone --branch v2.0.0 https://gitee.com/ygz123/clip-pal.git
cd clip-pal
```

也可以在仓库网页切到 `v2.0.0` 后下载 ZIP，解压后进入包含 `Cargo.toml`、`frontend`、`src-tauri` 的目录。接下来的命令都从这个目录执行。

在 macOS 终端运行以下命令。

```bash
npm --prefix frontend ci
```

在 Windows PowerShell 运行以下命令。

```powershell
npm.cmd --prefix frontend ci
```

首次构建还会下载 Rust 依赖和平台打包工具，请保持网络连接。`frontend/package-lock.json` 和 `Cargo.lock` 固定依赖版本，建议保留并使用仓库提供的版本。

### 3. 按需打包

每次选择下面的一条命令。脚本会先编译对应版本的前端，再调用 Tauri 生成安装包。个人构建关闭更新产物与远程更新检查，直接使用仓库提供的配置即可。

| 版本 | macOS 终端 | Windows PowerShell |
| --- | --- | --- |
| 单机版 | `npm --prefix frontend run package:local` | `npm.cmd --prefix frontend run package:local` |
| 自建版桌面预览 | `npm --prefix frontend run package:self-hosted` | `npm.cmd --prefix frontend run package:self-hosted` |

自建预览版当前无需部署地址、令牌或服务器。联网同步开放后，自建配置将包括 HTTPS 服务地址、部署认证和附件限制，届时本节会加入对应的配置与联调步骤。

**需要两个版本时请依次构建。** 两个版本目前沿用 ClipPal 的包标识、产物名称和本地数据目录。每次成功后先把安装包复制到不同目录，例如 `packages/local` 和 `packages/self-hosted`，再构建下一个版本。安装时把它们视为同一个应用的两个构建，切换前先从托盘退出 ClipPal。用来试装的电脑若已有历史数据，建议先退出应用后备份完整 ClipPal 数据目录。

### 4. 找到安装包并试用

终端打印 `Finished ... bundle` 后，会列出本次生成的路径。默认产物位置如下。

| 平台 | 安装文件 | 位置 |
| --- | --- | --- |
| macOS | `.dmg` | `target/release/bundle/dmg/` |
| macOS | `.app` | `target/release/bundle/macos/` |
| Windows | NSIS 安装程序 `.exe` | `target/release/bundle/nsis/` |
| Windows | MSI 安装包 `.msi` | `target/release/bundle/msi/` |

- **macOS** 打开 `.dmg`，把 `ClipPal.app` 拖到「应用程序」。如果 macOS 拦截你亲自构建且确认来源的应用，可在「系统设置 → 隐私与安全性」使用系统提供的「仍要打开」。自动粘贴需要辅助功能授权，在 ClipPal 的「设置 → 自动粘贴」点击「请求授权」或「打开系统设置」。授权后先切回目标应用，再唤起 ClipPal 选择记录。
- **Windows** 双击 `.exe` 或 `.msi`，任选一种即可。安装程序可能需要联网获取 WebView2 Runtime。对于系统安全提示，应先核对安装包来源。
- **验收** 启动后复制一段测试文本和一张图片，打开 ClipPal，检查历史列表、搜索、复制回原应用和图片另存为。退出重开后确认历史仍在。自建预览版按同一组本地功能验收。完整桌面验收见 [单机版验证记录](docs/standalone-validation.md)。

### 可选：用 Cargo 调用 Tauri 打包

`cargo build` 编译 Rust 可执行文件。要生成 `.dmg`、`.msi` 或 NSIS 安装程序，使用 `cargo tauri build`。第 3 步的 npm 脚本把前端构建和 Tauri 打包合在一起，也可以分开执行。首次从源码构建仍需按第 2 步安装 Vue 前端依赖。

首次使用 Cargo 入口时，安装项目已验证的 Tauri CLI 版本。

```sh
cargo install tauri-cli --version 2.9.4 --locked
```

从仓库根目录执行对应版本的两条命令。Windows PowerShell 将 `npm` 改为 `npm.cmd`。如果之前手动设置过 `VITE_EDITION`，先清除该环境变量，让下面的模式选择对应前端。

```sh
# 单机版
npm --prefix frontend run build
cargo tauri build --config src-tauri/tauri.personal.json --features local -- --no-default-features --locked
```

```sh
# 自建桌面预览版
npm --prefix frontend run build -- --mode self-hosted
cargo tauri build --config src-tauri/tauri.personal.json --features self-hosted -- --no-default-features --locked
```

前端构建成功后再执行对应的 Cargo 命令。如果 `dist` 已经由对应版本的前端构建生成，可以直接运行该 Cargo 命令。安装包位置和个人更新配置与第 3 步相同。

### 常见问题

| 现象 | 处理方式 |
| --- | --- |
| 找不到 `node`、`npm` 或 `cargo` | 安装相应工具后重开终端，用第 1 步的版本命令检查。PowerShell 中使用 `npm.cmd`。 |
| `npm ci` 或 Cargo 下载失败 | 先排查网络、代理和对应软件源的连通性。按原命令重试，保留仓库锁文件。 |
| Windows 报 `link.exe` 找不到或缺 Windows SDK | 回到 Visual Studio Installer 补装「使用 C++ 的桌面开发」，确认 `rustc -vV` 的 `host` 是 MSVC，再重开终端。 |
| 只想打某种安装包 | Windows 用 `npm.cmd --prefix frontend run package:local -- --bundles nsis` 单独生成 `.exe`。macOS 用 `npm --prefix frontend run package:local -- --bundles app` 单独生成 `.app`。自建预览使用对应的 `package:self-hosted` 命令。 |
| macOS 报 `failed to run bundle_dmg.sh` | 在真实的 macOS 桌面会话运行打包命令。DMG 制作需要挂载临时磁盘映像。如果只供本机试用，可先用 `-- --bundles app` 生成 `.app` 并复制到「应用程序」。 |
| 生成的文件名仍显示 `1.0.7` | `v2.0.0` 是开发分支名。目前应用打包版本仍为 `1.0.7`，正式发布前会统一版本号。 |
| 找不到安装包或只看到编译产物 | 找终端里第一处 `error`，修复后重新打包。以本次终端列出的 `Finished ... bundle` 路径为准。 |
| 另一个版本好像把原版覆盖了 | 当前两版共用 ClipPal 安装标识，先按第 3 步分目录保存产物。Mac 数据位于 `~/Library/Application Support/ClipPal`，Windows 数据位于 `%APPDATA%\ClipPal`。 |

## 维护者发布流程

个人包通过手动安装新版本更新。维护者要向多人分发或启用自动更新时，按以下流程发布。

1. **确定发布范围。** 先发布单机版。自建预览单独归档，待服务端和多设备同步验收完成后再作为自建正式版发布。每个版本保留各平台、架构对应的安装包。
2. **统一版本号。** 核对 `Cargo.toml` 的 workspace 应用版本、`src-tauri/tauri.conf.json` 的安装包版本及 `frontend/package.json` 的版本。更新依赖锁文件后再按 `--locked` 构建。版本以安装包实际显示的值为准。
3. **配置签名。** Tauri 更新签名与系统代码签名分别配置。已发布的更新渠道应继续使用与客户端公钥配对的更新私钥。新渠道可用 `node frontend/node_modules/@tauri-apps/cli/tauri.js signer generate -w <新私钥文件路径>` 生成密钥。把公钥和自己的 HTTPS 更新地址填入发布配置的 `plugins.updater`。将私钥路径交给构建环境变量 `TAURI_SIGNING_PRIVATE_KEY`，用 `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` 提供密码，私钥和密码存储在本机安全位置或 CI 的秘密变量中。面向公众分发时，按发布平台准备 macOS Developer ID 签名与公证、Windows Authenticode 签名。 [Tauri 更新签名](https://v2.tauri.app/plugin/updater/) · [Tauri 分发指南](https://v2.tauri.app/distribute/)
4. **构建正式产物。** 从仓库根目录先运行 `npm --prefix frontend run build`（PowerShell 用 `npm.cmd`），以默认单机前端构建。然后执行 `node frontend/node_modules/@tauri-apps/cli/tauri.js build --features local -- --no-default-features --locked`。维护自己的更新渠道时，在 `--features` 前增加 `--config <发布覆盖配置路径>`，该配置的 `bundle.createUpdaterArtifacts` 应为 `true`。
5. **先验收再上传。** Apple Silicon、Intel Mac 和 Windows x64 按实际支持范围在对应环境试装，检查首次启动、旧版升级、快捷键、自动粘贴、托盘、另存为及自启。有官方更新渠道时，再从上一个已发布版本实际升级一次，并确认升级后仍可读取本地历史。
6. **最后更新 `latest.json`。** 先上传安装和更新产物，再发布更新清单。macOS 用户下载 `.dmg`，更新器使用 `bundle/macos/` 下的 `.app.tar.gz` 及对应 `.sig`。Windows 更新器使用与 `.sig` 对应的 `.exe` 或 `.msi`。`platforms` 中用 `darwin-aarch64`、`darwin-x86_64`、`windows-x86_64` 等平台键，`url` 指向更新文件，`signature` 填对应 `.sig` 的内容。只列出本次确实上传并验证过的产物。保留上一版安装包和清单，便于排查发布问题。

后续正式版可以把上述流程放到 macOS / Windows CI 构建任务中，按版本、平台、架构分别归档。先让个人包构建和实机验证稳定，再把发布私钥接入受保护的发布任务。

单机正式包可以通过「设置 → 检查更新」下载并安装已签名的新版。用户确认后会使用本次检查得到的版本、下载地址和签名。个人包和未配置更新源的构建会在同一入口提示手动更新。自建或商业包若要启用自动更新，应配置对应版本自己的更新源和签名，确保安装后仍是原版本类型。

## 界面与项目结构

![主界面](assets/2.png)

![设置界面](assets/1.png)

商业扩展示例见 [用户信息](assets/3.png) 和 [VIP](assets/4.png)。代码依赖方向为 **商业版 → 自建版 → 单机版核心**。本仓库的 `src-tauri` 提供打包入口，`desktop-tauri` 负责桌面适配，`desktop-app-core` 和 `clip-core` 提供本地业务，`frontend` 提供 Vue 页面。自建同步、独立商业仓库和可选 AI 能力的设计见 [v2.0.0 架构规划](docs/2.0.0-architecture.md)，本地库和删除策略见 [单机数据库 v2](docs/local-database-v2.md)。主要依赖包括 [Tauri](https://tauri.app/)、[RBatis](https://github.com/rbatis/rbatis)、[Serde](https://serde.rs/) 和 [clipboard-rs](https://github.com/ChurchTao/clipboard-rs)。

## 版本记录

| 版本号   | 日期         | 主要更新内容                                                                                             |
|----------|--------------|----------------------------------------------------------------------------------------------------|
| v1.0.0   | 2025-06-18   | 项目初始化，支持基础剪贴板监听与历史管理                                                                               |
| v1.0.1   | 2025-06-19   | 增加图片另存为功能，禁用页面鼠标右键，优化部分样式                                                                          |
| v1.0.2   | 2025-06-26   | 优化sqlite初始化逻辑，支持字段新增；添加数据加密存储功能(数据加密使用了分词功能，导致安装包体积增大了很多TvT)；增强日志打印；优化错误处理等等<br/>⚠️ 本次更新后需卸载并清除旧数据 |
| v1.0.3   | 2025-06-28   | 新增了自动粘贴功能；新增了新手引导；优化前端对内容卡片的展示样式；优化了部分代码                                                           |
| v1.0.4   | 2025-07-03   | 优化了前端代码，增加自适应UI，图片加载方式优化；去除了对内容的分词处理(安装包体积大大降低、内存占用率也大大降低)和.bin文件存储，采用内存搜索和bloomfilter配合的方式处理模糊搜索  |
| v1.0.5   | 2025-09-04   | 处理了很多bug；添加了云同步功能；添加了用户登录相关功能；添加了vip和支付相关功能  |
| v1.0.6   | 2025-10-31   | 添加软件更新功能  |
| v1.0.7   | 2025-11-27   | 适配macOS,处理部分bug  |

## 开源协议

本项目基于 [Apache License 2.0](LICENSE) 开源。你可以自由使用、修改和分发本项目，但请保留原作者版权声明。

## ⚖️ 免责声明

1. 本项目是作为一款开源项目提供的，开发者在法律允许的范围内不对软件的功能性、安全性或适用性提供任何形式的明示或暗示的保证。
2. 用户明确理解并同意，使用本软件的风险完全由用户自己承担，软件以"现状"和"现有"基础提供。开发者不提供任何形式的担保，无论是明示还是暗示的，包括但不限于适销性、特定用途的适用性和非侵权的担保。
3. 在任何情况下，开发者或其供应商都不对任何直接的、间接的、偶然的、特殊的、惩罚性的或后果性的损害承担责任，包括但不限于使用本软件产生的利润损失、业务中断、个人信息泄露或其他商业损害或损失。
4. 所有在本项目上进行二次开发的用户，都需承诺将本软件用于合法目的，并自行负责遵守当地的法律和法规。
5. 开发者有权在任何时间修改软件的功能或特性，以及本免责声明的任何部分，并且这些修改可能会以软件更新的形式体现。

**本免责声明的最终解释权归开发者所有**

## 贡献方式

欢迎任何形式的贡献！你可以通过以下方式参与：
- 提交 Issue 反馈 bug 或建议
- Fork 并提交 Pull Request

## 联系方式

- 作者：jingchuan
- 邮箱：1750777402@qq.com
- QQ: 1750777402
- GitHub: https://github.com/1750777402/ClipPal
- Gitee: https://gitee.com/ygz123/clip-pal

---

> 本项目致力于为用户提供高效、安全的剪贴板管理体验。如果你喜欢本项目，欢迎 star、推荐和分享！

# Atha

Atha 是一个本地优先、以消息形式保存阅读反应的个人阅读系统。

当前遵循“后端先于前端”。Tauri 2 与 Svelte 5 应用已包含本地书架、应用内 EPUB2 / EPUB3、CBZ、FB2 / FBZ、MOBI / AZW / AZW3、Markdown 与 TXT 导入、阅读器和本地消息式阅读；Windows 使用 WebView2，Linux 使用 WebKitGTK，Android 使用系统 WebView。各格式的最高证据等级不同，PCT-AL10 上已安装的 arm64 调试包也不等同于发布验收。日常 GUI 与 APK 构建优先使用 Linux GNOME 会话，Windows 只在用户明确要求时运行。现有 `p0/` 只用于 FFI 与 SQLite 技术验证，不属于正式后端。

## 工程入口

- 根 `Cargo.toml`：正式 workspace；
- `backend/atha-backend/`：后端库、书根边界、EPUB2 / EPUB3、CBZ、FB2 / FBZ、MOBI / AZW / AZW3、Markdown 与 TXT 导入、正式消息数据库；
- `reader/app/`：Tauri 2、Svelte 5 应用壳和 production 前端构建；
- `reader/web/`：不依赖前端框架的阅读内核；
- 全局 `project-workflow`：管理任务 claim、工站证据与关闭；
- `scripts/check-docs.sh`、`scripts/check-reader-linux.sh`：当前 Bash 文档门与 Linux GUI 阅读门；
- `p0/`：独立实验，不进入根 workspace。

```bash
bash scripts/check-docs.sh
bash scripts/check-workflow.sh
bash scripts/check-reader-linux.sh
```

当前 Linux 与 PCT 开发入口只使用 Bash；旧 `.ps1` 仅保留为 Windows 历史兼容入口，不由当前门禁调用。

## 打开本地书籍

```bash
mise exec -- pnpm --dir reader/app build
mise exec -- cargo build --package atha-reader-app --locked
./target/debug/atha-reader-app
```

应用默认打开书架，可从系统文件对话框选择一个或多个 EPUB、CBZ、FB2 / FBZ、MOBI / AZW / AZW3、Markdown 或 TXT。也可用保留的 EPUB CLI 直接打开一本书：

```bash
./target/debug/atha-reader-app --epub "$HOME/Documents/Books/book.epub"
```

当前入口支持符合既定安全与资源边界的 EPUB2 / EPUB3 子集、只含 JPEG / PNG 页面的 CBZ、有界 FB2 / FBZ、PalmDOC / MOBI6 与纯 KF8 / AZW3 子集、UTF-8 Markdown，以及经 BOM、严格 UTF-8 或锁定 detector 识别的 TXT。DRM、KFX、AZW4、活动内容、网络资源和未知压缩稳定拒绝；Markdown 原始 HTML、活动链接和图片能力不会进入书根。Windows 书架记录位于 `%LOCALAPPDATA%\Atha\Library`，导入缓存位于 `%LOCALAPPDATA%\Atha\ImportedBooks`；Android 使用应用私有的 `app_local_data_dir`。系统 picker 的 SAF bridge 只接受允许列表中的显示文件名后缀，不从 URI 或正文猜格式，也不请求宽泛存储权限。同一格式的相同内容从不同路径导入只产生一个书架项，并复用同一阅读状态。

标注、笔记、回复、引用和历史快照统一保存在平台数据根下的 `Messages`；Windows 对应 `%LOCALAPPDATA%\Atha\Messages`，Android 对应应用私有数据目录。书架移除不会删除这些记录；阅读页的笔记面板可导出本书消息。

## 本地开发环境

Linux / macOS 的 Bash 开发机使用项目根 `.mise.toml` 管理 Node、pnpm 和 JDK，先运行：

```bash
mise install
mise exec -- pnpm --dir reader/app install --frozen-lockfile
mise exec -- pnpm --dir reader/app check
mise exec -- cargo build --workspace --locked
```

Rust 版本继续由 `rust-toolchain.toml` 固定。Windows 开发机复制 `env/example.ps1` 为 `env/local.ps1`，并填写本机的 `cargo`、`cmake`、`ctest`、`node`、`pnpm` 和 `sqlite3` 路径；Android 开发还需填写 JDK、Android SDK 与 NDK 路径。`env/local.ps1` 已被 Git 忽略；检查脚本统一加载它，不依赖当前 Shell 的 `PATH`。Linux 测试主机的实时路径、SSH 别名与 GNOME Wayland 启动约定以用户级 `$CODEX_HOME/HOSTS.md` 为准，不写死在仓库。

Tauri 阅读器的当前本地 GUI 检查入口是 `bash scripts/check-reader-linux.sh`。旧 `atha-reader-host` 暂时保留为 Wry/Tao 性能与安全基线，不是新的产品界面入口。

Android 本地门槛固定 Node 24.1.0、JDK 21、NDK 28.2.13676358、compile / target SDK 36 与 min SDK 26。启动 API 36、x86_64、16 KiB 页面的 `Atha_API_36_16K` AVD 后运行：

```bash
JAVA_HOME="$(mise where java)" \
ANDROID_HOME="$HOME/Android/Sdk" \
ANDROID_SDK_ROOT="$HOME/Android/Sdk" \
ANDROID_NDK_HOME="$HOME/Android/Sdk/ndk/28.2.13676358" \
mise exec -- pnpm --dir reader/app tauri android build --debug --target x86_64 --apk --ci
```

PCT-AL10 候选使用 `bash scripts/check-pct-reader.sh build` 和 `verify`；获准真机更新时再以显式 serial 运行 `install --device <serial>`。该入口检查 arm64 APK、签名、宽泛存储权限、16 KiB ZIP / ELF 对齐、设备身份与防降级；功能与触摸体验仍由 Linux GUI 门和用户真机验收覆盖。

## 项目入口

- 当前状态：`docs/ACTIVE.md`
- 文档索引：`docs/INDEX.md`
- 架构总览：`docs/architecture/OVERVIEW.md`
- 路线图：`docs/roadmap/ROADMAP.md`
- 协作规则：`AGENTS.md`

生产代码必须经过规格、计划、交叉审阅和用户批准门禁。

## 许可证

除另有标注的第三方代码与资产外，Atha 第一方代码依据 GNU Affero General Public License v3.0 or later（SPDX：`AGPL-3.0-or-later`）授权，详见 [`LICENSE`](LICENSE)。第三方字体、书籍、词典、fixture 与用户内容保留各自许可或权利状态；仓库内已复制资产的版权与许可见 [`THIRD_PARTY_NOTICES.md`](THIRD_PARTY_NOTICES.md)。

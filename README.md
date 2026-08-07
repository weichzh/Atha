# Atha

Atha 是一个本地优先、以消息形式保存阅读反应的个人阅读系统。

当前以 Windows 为稳定基线，并遵循“后端先于前端”。Tauri 2 与 Svelte 5 应用已包含本地书架、应用内 EPUB2 / EPUB3 与 CBZ 导入、阅读器和本地消息式阅读；Windows 使用 WebView2，Android 已用系统 WebView 建立同一产品壳和 reader runtime 的 EPUB / CBZ 功能纵切。当前 Android 证据限于 x86_64 16 KiB 模拟器，不代表 ARM 真机性能或发布包。现有 `p0/` 只用于 FFI 与 SQLite 技术验证，不属于正式后端。

## 工程入口

- 根 `Cargo.toml`：正式 workspace；
- `backend/atha-backend/`：后端库、书根边界、EPUB2 / EPUB3 与 CBZ 导入、正式消息数据库；
- `reader/app/`：Tauri 2、Svelte 5 应用壳和 production 前端构建；
- `reader/web/`：不依赖前端框架的阅读内核；
- `scripts/Invoke-Atha.ps1`：统一运行已登记检查并记录本机流程；
- `scripts/check-backend.ps1`：fmt、clippy、test 和 doc 统一检查；
- `p0/`：独立实验，不进入根 workspace。

```powershell
pwsh -NoProfile -File .\scripts\Invoke-Atha.ps1 check docs -Activity documentation
pwsh -NoProfile -File .\scripts\Invoke-Atha.ps1 station -Activity implementation -Scope backend
pwsh -NoProfile -File .\scripts\Invoke-Atha.ps1 report
```

当前统一入口只试点 `docs` target；其他检查仍直接运行既有脚本，待真实样本证明有价值后再接入。

## 打开 EPUB / CBZ

```powershell
. .\scripts\Import-AthaEnvironment.ps1 -RepoRoot (Get-Location).Path
& $env:ATHA_PNPM --dir .\reader\app build
& $env:ATHA_CARGO build --package atha-reader-app --locked
.\target\debug\atha-reader-app.exe
```

应用默认打开书架，可从系统文件对话框选择一个或多个 EPUB / CBZ。也可用保留的 EPUB CLI 直接打开一本书：

```powershell
.\target\debug\atha-reader-app.exe --epub 'E:\Books\book.epub'
```

当前入口支持符合既定安全与资源边界的 EPUB2 / EPUB3 子集，以及只含 JPEG / PNG 页面的 CBZ。CBZ 每图生成一个 section；可选 `ComicInfo.xml` 只投影 `Title`、`Writer` 与唯一有效的 `FrontCover`。Windows 书架记录位于 `%LOCALAPPDATA%\Atha\Library`，导入缓存位于 `%LOCALAPPDATA%\Atha\ImportedBooks`；Android 使用应用私有的 `app_local_data_dir`，不会改变 Windows 既有数据根。Android 系统 picker 通过 SAF content URI 与应用 cache 之间的流式桥接复用同一 backend；不透明的 content URI 副本先按严格 EPUB marker / container 识别，其余进入严格 CBZ 校验，不做通用失败回退。应用不请求宽泛存储权限。同一内容从不同路径导入只产生一个书架项，并复用同一阅读状态。

标注、笔记、回复、引用和历史快照统一保存在平台数据根下的 `Messages`；Windows 对应 `%LOCALAPPDATA%\Atha\Messages`，Android 对应应用私有数据目录。书架移除不会删除这些记录；阅读页的笔记面板可导出本书消息。

## 本地开发环境

每台电脑在开始开发前复制 `env/example.ps1` 为 `env/local.ps1`，并填写本机的 `cargo`、`cmake`、`ctest`、`node`、`pnpm` 和 `sqlite3` 路径。Android 开发还需填写 JDK、Android SDK 与 NDK 路径。`env/local.ps1` 已被 Git 忽略；检查脚本统一加载它，不依赖当前 Shell 的 `PATH`。

Tauri 阅读器的完整本地检查入口是 `pwsh -NoProfile -File .\scripts\check-tauri-reader.ps1`。旧 `atha-reader-host` 暂时保留为 Wry/Tao 性能与安全基线，不是新的产品界面入口。

Android 本地门槛固定 Node 24.1.0、JDK 21、NDK 28.2.13676358、compile / target SDK 36 与 min SDK 26。启动 API 35、x86_64、16 KiB 页面的 `Atha_API_35_16K` AVD 后运行：

```powershell
pwsh -NoProfile -File .\scripts\check-android-reader.ps1 -BookPath 'C:\path\to\book.epub' -CleanAppData
```

该入口检查 APK 构建、安装、冷启动、固定字段日志、16 KiB ZIP / ELF 对齐和宽泛存储权限，并在专用 AVD 上从干净应用数据自动完成系统 picker 导入、打开、reader ready、强停重启与书架持久性。对动态生成的 CBZ 使用 `-VerifyCbzFixture`，额外检查逐页导航、坏页继续、恢复和 PSS 证据。省略 `-BookPath` 时只运行构建和启动门槛；消息导出、全库备份 / 恢复仍是独立 opt-in 验收，避免默认替换开发中的 MessageStore。

## 项目入口

- 当前状态：`docs/ACTIVE.md`
- 文档索引：`docs/INDEX.md`
- 架构总览：`docs/architecture/OVERVIEW.md`
- 路线图：`docs/roadmap/ROADMAP.md`
- 协作规则：`AGENTS.md`

生产代码必须经过规格、计划、交叉审阅和用户批准门禁。

## 许可证

除另有标注的第三方代码与资产外，Atha 第一方代码依据 GNU Affero General Public License v3.0 or later（SPDX：`AGPL-3.0-or-later`）授权，详见 [`LICENSE`](LICENSE)。第三方字体、书籍、词典、fixture 与用户内容保留各自许可或权利状态；仓库内已复制资产的版权与许可见 [`THIRD_PARTY_NOTICES.md`](THIRD_PARTY_NOTICES.md)。

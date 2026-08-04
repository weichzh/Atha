# Atha

Atha 是一个本地优先、以消息形式保存阅读反应的个人阅读系统。

当前只推进 Windows，并遵循“后端先于前端”。Tauri 2、Svelte 5 与 WebView2 应用已包含本地书架、应用内 EPUB3 导入、阅读器和本地消息式阅读；移动平台尚未开始。现有 `p0/` 只用于 FFI 与 SQLite 技术验证，不属于正式后端。

## 工程入口

- 根 `Cargo.toml`：正式 workspace；
- `backend/atha-backend/`：后端库、书根边界、EPUB3 导入与正式消息数据库；
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

## 打开 EPUB

```powershell
. .\scripts\Import-AthaEnvironment.ps1 -RepoRoot (Get-Location).Path
& $env:ATHA_PNPM --dir .\reader\app build
& $env:ATHA_CARGO build --package atha-reader-app --locked
.\target\debug\atha-reader-app.exe
```

应用默认打开书架，可从系统文件对话框选择一个或多个 EPUB。也可用 CLI 直接打开一本书：

```powershell
.\target\debug\atha-reader-app.exe --epub 'E:\Books\book.epub'
```

当前入口只支持符合既定安全与资源边界的 EPUB3。书架记录位于 `%LOCALAPPDATA%\Atha\Library`，导入缓存位于 `%LOCALAPPDATA%\Atha\ImportedBooks`；同一内容从不同路径导入只产生一个书架项，并复用同一阅读状态。

标注、笔记、回复、引用和历史快照统一保存在 `%LOCALAPPDATA%\Atha\Messages`。书架移除不会删除这些记录；阅读页的笔记面板可导出本书消息，对话浮层可导出当前对话。

## 本地开发环境

每台电脑在开始开发前复制 `env/example.ps1` 为 `env/local.ps1`，并填写本机的 `cargo`、`cmake`、`ctest`、`node`、`pnpm` 和 `sqlite3` 路径。`env/local.ps1` 已被 Git 忽略；检查脚本统一加载它，不依赖当前 Shell 的 `PATH`。

Tauri 阅读器的完整本地检查入口是 `pwsh -NoProfile -File .\scripts\check-tauri-reader.ps1`。旧 `atha-reader-host` 暂时保留为 Wry/Tao 性能与安全基线，不是新的产品界面入口。

## 项目入口

- 当前状态：`docs/ACTIVE.md`
- 文档索引：`docs/INDEX.md`
- 架构总览：`docs/architecture/OVERVIEW.md`
- 路线图：`docs/roadmap/ROADMAP.md`
- 协作规则：`AGENTS.md`

生产代码必须经过规格、计划、交叉审阅和用户批准门禁。

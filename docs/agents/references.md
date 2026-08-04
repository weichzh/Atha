# 外部文档入口

本文件只保存 Atha 直接依赖的外部边界、版本事实来源和最短用法，不复制官方文档。进入任务时完整读取；当任务涉及下列边界的行为、API 语义、兼容性、错误、安全或性能时，先打开对应官方入口再诊断或修改。

## Tauri 2

- 版本事实：`reader/app/package.json`、`reader/app/src-tauri/Cargo.toml` 与 `Cargo.lock`。
- 官方入口：[从前端调用 Rust](https://v2.tauri.app/develop/calling-rust/)、[Rust API](https://docs.rs/tauri/latest/tauri/)。
- 项目快速用法：前端统一通过 `reader/app/src/messages.ts` 调用；Rust command 位于 `reader/app/src-tauri/src/lib.rs` 并在 `generate_handler!` 注册。可能阻塞的工作使用 async command；读取器 command 保留 `WebviewWindow` 来源校验并返回 `Result`。
- 最短检查：`pnpm --dir reader/app check`、`pnpm --dir reader/app build`、`cargo test -p atha-reader-app`。
- 必须重查：command 参数映射、async 或主线程行为、state、window/webview、IPC 权限和插件 API。

## Svelte 5

- 版本事实：`reader/app/package.json` 与 `reader/app/pnpm-lock.yaml`。
- 官方入口：[Svelte 文档](https://svelte.dev/docs/svelte/overview)。
- 项目快速用法：应用入口位于 `reader/app/src/main.ts`，阅读器外壳位于 `reader/app/src/components/ReaderChrome.svelte`；优先复用现有组件和原生 Svelte 状态，不增加第二套 UI 状态容器。
- 最短检查：`pnpm --dir reader/app check` 与 `pnpm --dir reader/app build`。
- 必须重查：runes、生命周期、事件、组件绑定、编译器警告和升级兼容性。

## WebView2

- 版本事实：Windows 目标机的 Evergreen WebView2 Runtime；产品入口由 Tauri 2 承载，`reader/atha-reader-host` 只保留为性能和安全基线。
- 官方入口：[WebView2 文档](https://learn.microsoft.com/en-us/microsoft-edge/webview2/)。
- 项目快速用法：书籍内容保持脚本、网络、路径和未知资源隔离；真实交互回归使用 `scripts/check-reader-samples.ps1`，Tauri 产品入口使用 `scripts/check-tauri-reader.ps1`。
- 必须重查：runtime 分发、浏览器能力差异、窗口缩放、输入事件、进程模型、安全边界和性能诊断。

## CSS 渲染与资源边界

- 版本事实：`reader/atha-reader.css`、`reader/web/content.mjs` 与 `backend/atha-backend/src/messages/model.rs`。
- 官方入口：[CSS Values 4](https://www.w3.org/TR/css-values-4/)、[CSS Images 4](https://www.w3.org/TR/css-images-4/)、[CSS Syntax 3](https://www.w3.org/TR/css-syntax-3/)。
- 项目快速用法：书籍和快照 CSS 不允许发起资源请求；`url()`、`src()`、`image()`、`image-set()`、`@import`、反斜杠转义和 Shadow DOM 穿透选择器在写入与显示两端均拒绝。快照捕获只保存不含子资源的 reader CSS 规则。
- 必须重查：新增 CSS 资源函数、转义语法、CSSOM 序列化、Shadow DOM selector 和浏览器实现变化。

## EPUB 3.3

- 版本事实：`backend/atha-backend/src/reader/epub/` 的导入实现与测试样本。
- 官方入口：[EPUB 3.3](https://www.w3.org/TR/epub-33/)。
- 项目快速用法：导入与不可信内容处理留在 backend；正式行为由 `backend/atha-backend/tests/epub_import.rs` 和阅读器样本检查覆盖。
- 必须重查：容器、OPF、spine、资源、导航、媒体类型、URL 解析和安全要求。

## SQLite 与 FTS5

- 版本事实：`backend/atha-backend/Cargo.toml`、`Cargo.lock` 与 `docs/codebase/DATABASE.md`。
- 官方入口：[SQLite](https://www.sqlite.org/docs.html)、[FTS5](https://www.sqlite.org/fts5.html)。
- 项目快速用法：持久化与迁移统一位于 `backend/atha-backend/src/`；消息检索不得在前端复制数据库事实。
- 最短检查：`cargo test -p atha-backend`。
- 必须重查：事务、迁移、约束、FTS 查询、排序、备份和并发语义。

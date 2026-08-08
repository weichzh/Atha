# 外部文档入口

本文件只保存 Atha 直接依赖的外部边界、版本事实来源和最短用法，不复制官方文档。进入任务时完整读取；当任务涉及下列边界的行为、API 语义、兼容性、错误、安全或性能时，先打开对应官方入口再诊断或修改。

## Tauri 2

- 版本事实：`reader/app/package.json`、`reader/app/src-tauri/Cargo.toml` 与 `Cargo.lock`。
- 官方入口：[从前端调用 Rust](https://v2.tauri.app/develop/calling-rust/)、[Rust API](https://docs.rs/tauri/latest/tauri/)。
- 项目快速用法：前端统一通过 `reader/app/src/messages.ts` 调用；Rust command 位于 `reader/app/src-tauri/src/lib.rs` 并在 `generate_handler!` 注册。可能阻塞的工作使用 async command；读取器 command 保留 `WebviewWindow` 来源校验并返回 `Result`。
- 最短检查：`pnpm --dir reader/app check`、`pnpm --dir reader/app build`、`cargo test -p atha-reader-app`。
- 必须重查：command 参数映射、async 或主线程行为、state、window/webview、IPC 权限和插件 API。

## Tauri Logging

- 版本事实：`reader/app/src-tauri/Cargo.toml` 与 `Cargo.lock` 中的 `tauri-plugin-log` / `log`。
- 官方入口：[Tauri Logging](https://v2.tauri.app/plugin/logging/)、[`tauri-plugin-log` Rust API](https://docs.rs/tauri-plugin-log/latest/tauri_plugin_log/)。
- 项目快速用法：产品 Rust 日志只使用 `atha::` target 和固定 operation / event、stage、code、耗时与计数；Info 以上写 stdout 与平台 AppLog，单文件 1 MiB，保留当前文件和最近两个归档。不得记录书名、路径、正文、笔记、查询、提示词或内容哈希。
- 最短检查：运行 `scripts/check-tauri-reader.ps1` 后检查平台 AppLog 同时包含启动、打开和 reader 固定阶段事件，并确认不含 fixture 路径或内容。
- 必须重查：插件 release line、Android 日志目录 / logcat 行为、target filter、轮转语义和敏感字段。

## Tauri Android、SAF 与平台文件

- 版本事实：`reader/app/package.json`、`reader/app/src-tauri/Cargo.toml`、`Cargo.lock`、`reader/app/src-tauri/tauri.android.conf.json` 与 `reader/app/src-tauri/gen/android/`；本项目 Android 门槛固定 Node 24.1.0、JDK 21、NDK 28.2.13676358、compile / target SDK 36、min SDK 26，当前默认目标为 API 36 x86_64 16 KiB AVD。
- 官方入口：[Tauri Android 前置条件](https://v2.tauri.app/start/prerequisites/#android)、[Tauri Dialog](https://v2.tauri.app/plugin/dialog/)、[`FilePath` Rust API](https://docs.rs/tauri-plugin-dialog/latest/tauri_plugin_dialog/enum.FilePath.html)、[Tauri File System](https://v2.tauri.app/plugin/file-system/)、[`FsExt` Rust API](https://docs.rs/tauri-plugin-fs/latest/tauri_plugin_fs/trait.FsExt.html)、[Android 16 KiB page size](https://developer.android.com/guide/practices/page-sizes)、[Android Auto Backup](https://developer.android.com/identity/data/autobackup)。
- 项目快速用法：`tauri-plugin-dialog` 只选择文件；`platform_file::PickerInput` / `PickerOutput` 对普通路径直接转发，对 Android content URI 使用官方 fs plugin 流式复制到应用 `cache/Picker`。书籍 picker 先用 Tauri 内置 `app.path().file_name(content://...)` 经 PathPlugin / `ContentResolver` 取得显示文件名，只保留 `.epub` / `.cbz` / `.fb2` / `.fbz` / `.md` / `.markdown` / `.txt` 后缀；不得从 URI 字符串或正文猜格式。输入复用领域上限：单次至多 32 本书、archive / TXT 源文件至多 512 MiB、直接 FB2 至多 64 MiB、Markdown 至多 16 MiB、恢复制品至多 8 GiB。临时目录独占创建并由 RAII 与启动清理回收，路径、URI、标题和内容不得进入日志。
- 最短检查：`pwsh -NoProfile -File scripts/check-android-reader.ps1 -BookPath <local-book> -CleanAppData`；Markdown / TXT 增加 `-VerifyMarkdownText`。入口默认验证 `Atha_API_36_16K`，需要复核历史 API 35 证据时再显式传 `-ExpectedAvd` / `-ExpectedApi`；它同时验证 APK badging、无宽泛存储权限、16 KiB ZIP / ELF 对齐、安装、冷启动、系统 picker 导入、打开、reader ready、强停重启、书架持久与固定日志。消息导出、全库备份 / 恢复仍按活动 change 独立 opt-in，不把模拟器结果称为 ARM 真机性能证据。
- 必须重查：Tauri mobile / plugin release line、Android Gradle Plugin 与 SDK / NDK 兼容、SAF provider 的 open / truncate / failure 语义、release 签名、实体 ARM 设备的 I/O / 内存 / WebView 性能，以及 API 31+ `dataExtractionRules` 与设备厂商的备份 / 迁移行为。

## Linux GNOME 目标测试

- 运行事实：主机别名、用户目录和实时工具路径只保存在用户级 `$CODEX_HOME/HOSTS.md`；仓库不硬编码地址。GNOME user manager 当前应包含 `WAYLAND_DISPLAY=wayland-0`、`DISPLAY=:0` 与 `XDG_RUNTIME_DIR=/run/user/1000`。
- 官方入口：[Tauri WebDriver](https://v2.tauri.app/develop/tests/webdriver/)、[Tauri WebDriver 手动配置](https://v2.tauri.app/develop/tests/webdriver/manual-setup/)、[`WebviewWindowBuilder::use_https_scheme`](https://docs.rs/tauri/latest/tauri/webview/struct.WebviewWindowBuilder.html#method.use_https_scheme)。
- 项目快速用法：SSH 中的 CLI 构建直接运行；需要在远程 GNOME / RDP 桌面显示并在 SSH 断开后保留的 GUI 使用 `systemd-run --user --collect --unit=<唯一名> <program>`，例如 `systemd-run --user --collect --unit=gui-code code ~/Code/Atha`。不得用 `sudo` 启动 GUI，也不得依赖 SSH shell 临时导出的 Wayland 变量。
- 测试策略：日常格式与 GUI 开发直接使用 Linux Tauri / WebKitGTK；Android 模拟器只在发布前或移动端专项验收时启动，Windows 只在用户明确要求时使用。用户已批准通过安全通道把私有 `fixtures/local` 复制到目标仓库的忽略目录；样本仍只用于本地 opt-in，不提交、不写日志、不进入公开证据或分发包。
- 最短检查：`systemctl --user show-environment | rg '^(WAYLAND_DISPLAY|DISPLAY|XDG_RUNTIME_DIR)='`；安装一次 `cargo install tauri-driver --version 2.0.6 --locked` 后运行 `pwsh -NoProfile -File scripts/check-fb2-source.ps1 -VerifyLinuxGui`。入口用唯一 transient unit 启动真实 Tauri 壳，隔离 XDG 应用数据，并把构建、WebKitGTK 交互、截图和日志隐私作为 Linux 目标证据。
- 必须重查：SSH 别名、GNOME 会话是否仍存活、systemd user 环境、KVM / emulator、Rust Android targets、JDK / SDK / NDK、Linux WebKitGTK / Tauri prerequisites 与私有样本是否已显式放置。

## Markdown / TXT

- 版本事实：`backend/atha-backend/Cargo.toml` 与 `Cargo.lock` 固定 `pulldown-cmark 0.13.4`、`chardetng 1.0.0`、`encoding_rs 0.8.35` 和 `regex 1.13.1`。
- 官方入口：[`pulldown-cmark` 0.13.4](https://docs.rs/pulldown-cmark/0.13.4/)、[`chardetng` 1.0.0](https://docs.rs/chardetng/1.0.0/)、[`encoding_rs` 0.8.35](https://docs.rs/encoding_rs/0.8.35/)、[`regex` 1.13.1](https://docs.rs/regex/1.13.1/)；格式取舍见 `docs/research/markdown-txt-format-assessment.md`。
- 项目快速用法：`reader::text` 直接生成现有 ReaderManifest / BookRoot。Markdown raw HTML 转义，链接 / 图片只保留惰性文本；TXT 只在至少两个高置信整行标题时生成章节 TOC，并按约 1 MiB 合并物理 sections。相同字节的 Markdown / TXT 使用不同固定身份域；EPUB / CBZ 身份不变。
- 最短检查：`cargo test --locked -p atha-backend --test text_import`、`pwsh -NoProfile -File scripts/check-text-source.ps1`；私有 TXT 只通过 `ATHA_LOCAL_TXT_SAMPLE` 显式 opt-in，不输出路径、标题、正文或哈希。
- 必须重查：parser / decoder release line、`encoding_rs` 的复合 SPDX、章节规则与真实语料、ARM64 Android 性能、Markdown 新增活动资源能力及 ReaderManifest section / TOC 上限。

## FB2 / FBZ、`quick-xml` 与 `base64`

- 版本事实：`backend/atha-backend/Cargo.toml` 与 `Cargo.lock` 固定 `quick-xml 0.41`（`encoding` feature）、`base64 0.22.1`、`zip 8.6` 和 `imagesize 0.15.0`。
- 官方入口：[`quick-xml` 0.41](https://docs.rs/quick-xml/0.41.0/quick_xml/)、[`base64` 0.22.1](https://docs.rs/base64/0.22.1/base64/)、[`zip` 8.6](https://docs.rs/zip/8.6.0/zip/)。
- 项目快速用法：`reader::fb2` 对直接 FB2 或单根成员 FBZ 做两遍有界流式解析，只投影受支持正文、目录、内部链接和 JPEG / PNG binary，不解释源 stylesheet，不加载外部资源。同一 XML 的 FB2 / FBZ 共享固定格式域身份；picker 只按后缀分派，不嗅探正文。
- 最短检查：`cargo test --locked -p atha-backend --test fb2_import`；Linux 真实 GUI 使用 `pwsh -NoProfile -File scripts/check-fb2-source.ps1 -VerifyLinuxGui`。
- 必须重查：FB2 schema / encoding 语料、未知元素策略、binary 图片类型、FBZ 单成员大小、依赖许可证和 Android ARM 真机内存。

## Rust 文件锁与 `fs2`

- 版本事实：`rust-toolchain.toml` 固定 Rust 1.97.1，`backend/atha-backend/Cargo.toml` 与 `Cargo.lock` 固定 `fs2 = 0.4.3`。
- 官方入口：[Rust 1.97.1 Unix `std::fs` 源码](https://github.com/rust-lang/rust/blob/1.97.1/library/std/src/sys/fs/unix.rs#L1353-L1550)、[`fs2::FileExt`](https://docs.rs/fs2/0.4.3/fs2/trait.FileExt.html)、[`fs2` 0.4.3 package metadata](https://docs.rs/crate/fs2/0.4.3/source/Cargo.toml.orig)。
- 项目快速用法：Rust 1.97.1 的 Unix 文件锁实现没有把 Android 列入 `flock` 支持集合，Android 会得到 `Unsupported`；MessageStore 维护锁只在 `store.rs` 与 `backup.rs` 通过 `fs2::FileExt` 使用 shared / exclusive try-lock。`fs2` 0.4.3 的许可为 `MIT/Apache-2.0`，不扩展为通用锁 abstraction。
- 最短检查：`cargo test -p atha-backend`，再在 Android 冷启动日志中确认 MessageStore setup 不再返回 `message-database`。
- 必须重查：Rust 升级后标准库是否原生支持 Android、网络文件系统 / 厂商内核的 advisory lock 语义、维护锁协议变化与依赖许可变化；标准库实测覆盖 Android 前不移除 `fs2`。

## 项目与依赖许可证

- 版本事实：根 `LICENSE`、根 / member Cargo manifest、独立 P0 manifest 与 `reader/app/package.json` 中的精确 SPDX。
- 官方入口：[GNU AGPL v3](https://www.gnu.org/licenses/agpl-3.0.html)、[SPDX `AGPL-3.0-or-later`](https://spdx.org/licenses/AGPL-3.0-or-later.html)、[Cargo license 字段](https://doc.rust-lang.org/cargo/reference/manifest.html#the-license-and-license-file-fields)、[Cargo workspace package 继承](https://doc.rust-lang.org/cargo/reference/workspaces.html#the-package-table)、[npm license 字段](https://docs.npmjs.com/cli/configuring-npm/package-json/#license)。
- 项目快速用法：Atha 第一方代码使用 `AGPL-3.0-or-later`；第三方代码与资产保留各自许可。依赖接入必须核对精确 `-only` / `-or-later`、链接与分发方式以及适用的源码、NOTICE、修改说明或重新链接义务。
- 最短检查：Cargo / npm manifest 解析、根许可证与 GNU 官方纯文本哈希一致、三个锁文件无 diff、required `docs` gate。
- 必须重查：首次正式分发、双许可 / CLA、CSS 社区独立仓库、`AGPL-3.0-only` 依赖、LGPL Android 链接以及任何可再分发性不明的字体、书籍、词典或 fixture。

## Svelte 5

- 版本事实：`reader/app/package.json` 与 `reader/app/pnpm-lock.yaml`。
- 官方入口：[Svelte 文档](https://svelte.dev/docs/svelte/overview)。
- 项目快速用法：应用入口位于 `reader/app/src/main.ts`，阅读器外壳位于 `reader/app/src/components/ReaderChrome.svelte`；优先复用现有组件和原生 Svelte 状态，不增加第二套 UI 状态容器。
- 最短检查：`pnpm --dir reader/app check` 与 `pnpm --dir reader/app build`。
- 必须重查：runes、生命周期、事件、组件绑定、编译器警告和升级兼容性。

## Tiptap 3

- 版本事实：`reader/app/package.json` 与 `reader/app/pnpm-lock.yaml`。
- 官方入口：[Svelte 集成](https://tiptap.dev/docs/editor/getting-started/install/svelte)、[StarterKit](https://tiptap.dev/docs/editor/extensions/functionality/starterkit)、[JSON 持久化](https://tiptap.dev/docs/guides/output-json-html)、[ProseMirror Markdown 示例](https://prosemirror.net/examples/markdown/)、[ProseMirror API](https://prosemirror.net/docs/ref/#markdown)。
- 项目快速用法：消息输入器只启用 StarterKit 中已进入 Atha 消息正文契约的文字节点与标记；持久化始终使用 Tiptap JSON，`plainText` 只是搜索与无格式回退投影。原始 Markdown 是同一 JSON 文档的临时输入视图，使用稳定的 `prosemirror-markdown` 双向转换；官方 `@tiptap/markdown` 当前仍是 Beta，不进入产品路径。编辑器与 Markdown 转换分别按需加载。
- 最短检查：`pnpm --dir reader/app test:markdown`、`pnpm --dir reader/app check` 与 `pnpm --dir reader/app build`。
- 必须重查：扩展 schema、粘贴内容、链接协议、JSON 兼容性、编辑器生命周期和只读呈现。

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

## CBZ、ComicInfo 与 `imagesize`

- 版本事实：`backend/atha-backend/Cargo.toml` 与 `Cargo.lock` 中的 `zip 8.6`、`quick-xml 0.41`、`imagesize 0.15.0`。
- 官方入口：[`imagesize 0.15.0`](https://docs.rs/crate/imagesize/0.15.0)、[`ImageType`](https://docs.rs/imagesize/0.15.0/imagesize/enum.ImageType.html)、[`blob_size`](https://docs.rs/imagesize/0.15.0/imagesize/fn.blob_size.html)、[ComicInfo schema](https://github.com/anansi-project/comicinfo)、[PKWARE APPNOTE](https://pkware.cachefly.net/webdocs/casestudies/APPNOTE.TXT)、[`HTMLImageElement.decode()`](https://html.spec.whatwg.org/multipage/embedded-content.html#dom-img-decode-dev)。
- 项目快速用法：`reader::archive` 为 EPUB / CBZ 共享 crate-private ZIP 信任边界；`reader::cbz` 只接受 JPEG / PNG，以 `imagesize` 校验魔数、非零尺寸、8192 单边和 20000000 像素预算，再生成 schema 1 ReaderManifest 与一图一 XHTML section。`imagesize` 不验证完整压缩流，最终损坏由 WebView `img.decode()` 显示受控坏页。ComicInfo 只消费有界的 Title、Writer 与唯一有效 FrontCover。
- 最短检查：`cargo test --locked -p atha-backend --test cbz_import`、`pwsh -NoProfile -File scripts/check-cbz-source.ps1`，再用 `pwsh -NoProfile -File scripts/check-android-reader.ps1 -BookPath .\.tmp\cbz-gate.cbz -CleanAppData -VerifyCbzFixture` 验证系统 picker、逐页阅读、强停恢复和 PSS 证据。
- 必须重查：新增图片格式、RTL / spread、ComicInfo 字段、ZIP feature、`imagesize` 类型 / 尺寸行为、完整 decoder 需求、Android GPU / renderer 内存与 ARM 真机门槛。

## SQLite 与 FTS5

- 版本事实：`backend/atha-backend/Cargo.toml`、`Cargo.lock` 与 `docs/codebase/DATABASE.md`。
- 官方入口：[SQLite](https://www.sqlite.org/docs.html)、[FTS5](https://www.sqlite.org/fts5.html)。
- 项目快速用法：持久化与迁移统一位于 `backend/atha-backend/src/`；消息检索不得在前端复制数据库事实。
- 最短检查：`cargo test -p atha-backend`。
- 必须重查：事务、迁移、约束、FTS 查询、排序、备份和并发语义。

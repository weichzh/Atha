# 代码库地图

## 仓库状态

当前生产代码包含根 Cargo workspace、正式后端 crate、EPUB3、EPUB2 / NCX 子集与 CBZ JPEG / PNG 导入、本地书架、正式消息数据库、Tauri 2 产品 host、Svelte 5 应用壳和无框架阅读内核；Windows WebView2 是稳定基线，Android 已有同一产品壳与 reader runtime 的 EPUB / CBZ 功能纵切，EPUB2 的系统 picker、目录跳转与位置恢复及 CBZ 的逐页、坏页继续和强停恢复均已通过正式模拟器入口；直接 Wry / Tao host 暂留为 Windows 回归基线。精确演进历史由 Git 保存，本文件只描述当前结构。

## 顶层结构

| 路径 | 责任 | 状态 |
|---|---|---|
| `.cargo/config.toml` | RsProxy sparse index 与 Cargo 网络配置 | 已配置 |
| `Cargo.toml`、`Cargo.lock` | 正式 virtual workspace 与锁文件 | M3 已验证 |
| `backend/atha-backend/` | 正式后端库、书根资源边界、EPUB3、EPUB2 / NCX 子集与 CBZ JPEG / PNG 导入、本地书架、消息数据库与阅读遥测校验 | 本地已验证 |
| `reader/app/` | Tauri 2、Vite、Svelte 5 产品入口；离线搜索 / 进度 / 排序 / 批量选择书架、应用壳、能力清单、受控协议和打包配置 | Windows / Android 已验证 |
| `reader/app/src-tauri/src/lib.rs` | Tauri composition root，以及当前仍同文件的 library、telemetry、固定字段平台日志与 protocol adapter | 已验证 |
| `reader/app/src-tauri/src/platform_file.rs` | 普通路径与 Android SAF content URI 共用的流式 Picker cache bridge；RAII / 启动清理和输入大小边界 | Android 模拟器已验证 |
| `reader/app/src-tauri/src/runtime_diagnostics.rs` | Windows Recorder 与移动端交互诊断的目标平台选择；固定事件 token 由 backend 统一校验 | Windows / Android 已验证 |
| `reader/app/src-tauri/gen/android/`、`tauri.android.conf.json` | Tauri 官方 Android 工程、min SDK 26、compile / target SDK 36、manifest、应用图标与暗色系统栏样式 | x86_64 debug 已验证 |
| `reader/app/src-tauri/src/message_commands.rs` | 消息 IPC adapter；统一阅读路由校验、DTO 转发、稳定错误和原生导出 dialog | 已验证 |
| `reader/app/src-tauri/src/message_maintenance.rs` | 全库消息维护 IPC adapter；统一资料库根路由、备份 / 恢复 dialog 与 blocking worker | 已验证 |
| `reader/atha-reader-host/src/` | 共享 CLI、窗口尺寸和诊断逻辑；Wry/Tao 基线 host | 已验证 |
| `reader/atha-reader.html`、`reader/atha-reader.css` | 唯一阅读页结构、默认样式、原生阅读偏好、书签、消息投影、搜索面板、对话浮层与内容 dialog | 已实现 |
| `reader/web/` | Locator、导航、偏好、输入与内容动作、阅读会话、状态、书签、搜索、消息适配/对话、标注投影、内容安全、分页、诊断、benchmark 和页面组合入口 | 已实现 |
| `reader/samples.json` | 四个本地验收样本的入口、manifest、内容、搜索和边界断言清单 | M2 已验证 |
| `p0/ffi/` | Rust/C++ 共享 C ABI 调用与所有权对照 | 本地 P0 实验 |
| `p0/sqlite/` | SQLite、FTS5、Outbox schema 与故障检查 | 本地 P0 实验 |
| `scripts/check-backend.ps1` | 正式后端 fmt、clippy、test 和 doc | M1 已通过 |
| `scripts/check-p0-ffi.ps1` | 构建两个 FFI 实现并运行统一 runner | 已通过 |
| `scripts/check-p0-sqlite.ps1` | 重建数据库并验证事务、FTS 与 10k 冒烟 | 已通过 |
| `scripts/check-reader-slice.ps1` | 构建实际 host，运行安全、布局和性能验收 | M2 已通过 |
| `scripts/check-reader-formula-performance.ps1` | 锁定真实公式重负载 EPUB 与章节，构建 Tauri 前端并运行十样本 WebView2 median/P95 benchmark | 已通过 |
| `scripts/export_reader_sample.py` | 安全、可重复地从 EPUB 导出单章节、带 manifest 的多章节或 fixture-only 全 XHTML 验收样本 | M2 已通过 |
| `scripts/Serve-ReaderValidation.ps1` | 只读环回提供同一阅读页、manifest 和书根资源 | M2 R1 已通过 |
| `scripts/check-reader-samples.ps1` | 四样本实际 host、内容交互、状态、搜索、标注与明暗主题截图总验收 | M2 已通过 |
| `scripts/check-reader-wheel.ps1` | 真实浏览器媒体滚轮、连续离散输入接受率与输入到稳定页 P95 快速检查 | 已通过 |
| `scripts/check-reader-gate.ps1` | 组合四样本、大书搜索、进程树内存、强杀恢复和固定 P95 性能门槛 | M2 R8 已通过 |
| `scripts/check-tauri-reader.ps1` | Svelte production build、workspace Rust 检查、Tauri build、普通 EPUB 消息模式启动、导入探针与性能门槛 | 已通过 |
| `scripts/check-android-reader.ps1` | 固定 16 KiB x86_64 AVD 的 APK 构建、对齐、安装、冷启动，以及 opt-in picker / 阅读恢复、书架搜索 / 视图 / 选择 / 移出、截图与双日志隐私检查 | 模拟器 EPUB / CBZ / 书架纵切已通过 |
| `scripts/check-message-reading.ps1` | 正式消息集成测试、前端检查/build、Tauri/host 测试及 command / permission 映射 | 已通过 |
| `scripts/check-epub-source.ps1` | 固定 EPUB3 的 Rust 检查、真实导入形状与 WebView2 import probe | M3 已通过 |
| `scripts/check-cbz-source.ps1` | 动态原创 CBZ、workspace Rust 检查、导入形状与 Windows WebView2 import probe | Windows 已通过 |
| `scripts/check-library-shelf.ps1` | 本地书架后端、production build、真实 Tauri 无参数启动，以及四视口搜索 / 视图 / 菜单 / 触控 / 安全区 UI 检查 | Windows 已通过 |
| `scripts/Invoke-Atha.ps1` | 统一工程 CLI；自动记录 `check docs`、`station` 与 `report` | 本地已验证 |
| `scripts/Measure-Workflow.ps1` | schema v1/v2 本机流程日志、兼容汇总与自检 | 本地已验证 |
| `docs/agents/workflow.md`、`docs/agents/references.md` | 全局工作流的项目契约，以及外部技术的官方入口和快速用法 | 已配置 |
| `docs/` | 当前事实、路线、长期决策、活动 change 与未决研究 | 已建立 |

`p0/` 只保存技术验证，不是生产后端。后续正式代码不得直接在 P0 目录上堆叠。

### 正式后端基线

- workspace 包含 `atha-backend`、`atha-reader-host` 与 `atha-reader-app`，并显式排除 P0 Rust crate；产品 app 依赖 backend 与 host 的共享 Windows 启动 / 诊断代码，host 只依赖 backend；
- 版本 `0.1.0`、edition 2024、Rust `1.97.1`、第一方许可证 `AGPL-3.0-or-later` 和禁止 unsafe 的 lint 由 workspace 统一；独立 P0 crate 与前端 package 显式投影同一许可证；
- 后端 crate 使用共享 `zip 8.6`、`quick-xml`、`sha2`、`serde` 与 `serde_json` 处理 EPUB / CBZ，CBZ 只新增运行时 `imagesize 0.15.0` 的 `jpeg` / `png` feature；动态原创 fixture writer 使用 dev-only `png 0.18.1`。`dom_query`、锁定的 `rusqlite 0.40.1` bundled SQLite 与 `fs2 0.4.3` 实现严格快照校验、消息事实和跨 Windows / Android 维护锁；`fs2` 许可为 `MIT/Apache-2.0`，没有 repository trait、锁服务或多格式工厂；
- 根锁文件包含正式后端导入、消息数据库、`fs2 0.4.3`、Tauri 官方日志 / 文件系统插件与固定版本的 Wry/Tao 承载依赖，P0 继续保留独立锁文件；
- `backend::messages::MessageStore` 拥有 schema v2 只向前迁移、WAL、外键、FTS5、事务 Outbox、内容寻址资产、旧标注迁移、自包含交换导出及 schema 1 完整备份 / 恢复；维护锁通过 `fs2::FileExt` 实现，因为 Rust 1.97.1 的标准库 Unix 文件锁在 Android 返回 `Unsupported`。

### HTML 阅读切片

- `BookRoot` 规范化书根并拒绝编码、路径、符号链接、文件类型、MIME 与大小越界；reader manifest 已声明的 section 以 XHTML 返回，因此不依赖源文件扩展名，也不把未声明的无扩展名文件当作 XHTML；
- schema 1 manifest 声明内容版本、有序 section、资源和可选 TOC；Windows host 的 `--epub` 与 `--book-root` 输入互斥，后者再从 `--manifest` 与兼容 `--entry` 二选一；
- `reader::archive` 为 EPUB / CBZ 共享 crate-private `zip 8.6` 打开、路径、重复 / 重叠、加密、symlink、成员和声明解压总量边界；写入成员与 CBZ 页面另按实际读取量累计，少量 EPUB metadata 读取只受单成员上限约束。`zip 8.6` 没有 pre-allocation `max_entries` API，因此打开前只以标准 terminal EOCD hint 拒绝超过 10000 项、trailing garbage 与歧义 terminal EOCD，打开后再校验条目数；fallback / ZIP64 在 post-open 检查前的最坏预分配是受 512 MiB 源文件上限约束的残余风险；
- `reader::epub` 的公开 interface 是 `import_epub`：`mod` 编排内容哈希与原子缓存，`package` 拥有 container、OPF2 / OPF3 spine、navigation 和 schema 1 计划；OPF2 只从 `spine@toc` 找到有界 NCX，把嵌套 `navPoint` 按前序拍平成现有 TOC，OPF3 继续使用唯一 XHTML nav；XHTML 由 OPF media type 判定，同一源字节跨路径得到相同缓存根和状态键；
- `reader::cbz::import_cbz` 只接受 JPEG / PNG，按路径分段自然序生成一图一 XHTML section 与声明资源；`ComicInfo.xml` 只投影 `Title`、`Writer` 与唯一有效 `FrontCover`，`imagesize` 校验类型和像素预算，WebView decode 失败时显示可导航坏页；
- `reader::library::LocalLibrary` 以已知扩展名严格分派 EPUB / CBZ，不透明 URI 副本则以严格 EPUB marker / container 与严格 CBZ 做内容分派；它以内容哈希为身份，用每书一份 JSON 提供 `list`、`import`、`open`、`cover` 和 `remove`；移除记录不删除导入缓存或阅读状态；
- `atha` 与 `atha-book` 自定义协议只提供应用资源和当前书根资源；导航、新窗口、下载与外部请求默认拒绝；
- 原生 host 的 `main.rs` 只选择 Windows 入口；`windows.rs` 组合事件循环，`launch`、`protocol` 与 `diagnostics` module 分别拥有参数和窗口、受控资源、稳定状态键、日志与 benchmark；WebView2 使用持久 profile；
- 阅读页源码保持原生 ES module：`locator`、`navigation`、`preferences`、`session` 与 `pagination` 拥有既有阅读热路径；`content` 与 `search` 在解析前只白名单并剥离 HTML5、XHTML 1.1 和兼容扩展 XHTML 1.0 Strict 固定声明，主动内容仍拒绝；`content` 额外从已验证 Range 捕获 Snapshot 候选，并对具有显式宽高的 SVG 公式执行当前页优先校验、解码和章节内短期复用；`message-store` 把 Tauri Message client 适配为标注投影并迁移旧记录；`annotations` 负责选择、重选、重锚、高亮、根消息列表与筛选；`conversations` 负责回复、引用、修订、关系、快照、跳回、本条/本章/本书查询投影和本书导出；`diagnostics` 继续拥有验证与 benchmark；`app` 只组合流程并禁用默认右键菜单；
- 十八份页面源码由 Vite 或应用资源协议按固定顺序交付为单个 `atha-reader` runtime，避免为源码分层增加多次请求；浏览器验证服务器使用同一顺序，并对各 module 与拼接后的整体 bundle 运行语法检查；
- Locator 以内容版本、section id 和 DOM 文本 UTF-16 偏移表示 point/range；R2 range 限于单 section 并检查实际文本边界，无效输入安全回落并留下诊断，页码不作为内容坐标；窗口重排暂时无法测量文字矩形时保留已校验偏移和当前页，错误界面显示稳定代码与阶段而不暴露书籍内容；
- 上一页和下一页可跨 section；manifest TOC 与已有书签继续共用隐藏的原生 `select` 数据源，壳层把它投影为全屏目录按钮，书签紧随对应章节并通过 Locator 跳转；用户点击章节或书签后等待导航稳定并返回沉浸阅读；字号重排按变化前 Locator 恢复到包含同一偏移的页面；
- 应用默认拥有系统/浅色/纸张/深色主题、亮度、字号、字体、紧凑/标准/舒展密度和点击/滑动翻页；亮度只过滤阅读页，四边距不属于用户偏好，旧记录中的边距字段在恢复时忽略；本书覆盖只拥有书源样式和安全用户 CSS，两层分别校验和持久化；书签与进度按 host 提供的书籍状态键分区，位置高频写与低频状态分离；
- 公式按源尺寸随字号缩放，行间公式使用独立 `1.5` 倍率并在逻辑内容列中居中；
- 阅读页内部设备像素尺寸跟随 WebView 视口与 DPR，使用 CSS 多栏并以 `1 / devicePixelRatio` 隔离系统 DPI；移动阅读壳层默认沉浸，48 CSS px 工具栏只覆盖固定 144 设备像素的页眉页脚安全区且不参与分页；文字、公式和原子内容均有布局后裁切检查；
- Windows 窗口与壳层控件使用系统逻辑像素，默认内部尺寸为 430 × 820，最小为 360 × 640，可自由调整和最大化；窗口变化经 Navigation 队列恢复 Locator；
- 书内文档的宿主 IPC 只接收固定、限长、非内容性的性能与状态事件；
- Tauri 产品入口保持单 WebView；Svelte 组件拥有书架、应用壳和对话 DOM，书架只对受限 DTO 做本地标题 / 作者搜索、严格进度二态、稳定排序与显式批量选择；Vite 直接拼接十八份 reader module，书籍 DOM、消息事实和分页热路径不进入组件状态；无阅读路由时不加载 reader bundle；
- Tauri `lib.rs` 组合状态、窗口、protocol、lifecycle、固定字段平台日志与 command 注册，并暂时保留 library、telemetry 与 protocol adapter；书架 command 只向可信壳暴露受限书目，不返回源路径或内容；`message_commands` adapter 统一校验当前阅读窗口并转发受限 DTO，`message_maintenance` adapter 只接受资料库根路由并在 blocking worker 执行全库备份 / 恢复；动态 `atha-book` 提供当前正文，独立 `atha-cover` 只读提供已登记封面；阅读器遥测复用后端白名单解析和共享 diagnostics，reader failure 额外携带固定阶段；官方日志插件只持久化 `atha::` target 的启动、导入 / 打开、reader 首稳 / ready / failure 和 protocol 5xx 数值事件，1 MiB 轮转并保留三份，不记录书籍或消息内容；消息专项检查精确核对 handler 注册与 permission；

### Android EPUB 纵切

- Tauri mobile crate output、mobile entry point 与 target-gated Windows host 已接通；Windows 仍使用 `%LOCALAPPDATA%\Atha`，Android 使用 Tauri `app_local_data_dir`，两端共用 LocalLibrary、MessageStore 与 reader kernel；
- Android 工程固定 min SDK 26、compile / target SDK 36；本机 gate 固定 Node 24.1.0、JDK 21、NDK 28.2.13676358，并运行 `Atha_API_35_16K`（API 35、x86_64、16,384-byte page）AVD；APK 通过 16 KiB ZIP alignment 与全部 x86_64 ELF `LOAD 0x4000` 检查；
- `platform_file::PickerInput` / `PickerOutput` 对普通路径零复制，对 SAF content URI 使用锁文件已有的官方 `tauri-plugin-fs` 流式复制到应用 `cache/Picker`；导入仍限制单次 32 本和单本 512 MiB，恢复仍限制 8 GiB，消息导出和备份仍由 backend 限制为 512 MiB 与 8 GiB；每个 cache 目录独占创建，Drop 与启动均清理；
- 系统 picker 链路已在模拟器手工验证 EPUB 导入 / 打开 / 重启恢复、消息导出及全库备份 / 恢复，完成后 Picker cache 为空；manifest 不请求宽泛存储权限，设置 `allowBackup=false` 并以 API 31+ `dataExtractionRules` 排除 cloud backup 与 device transfer；
- Android app storage 实测 hard link 返回 `PermissionDenied`。非 Android 备份继续用 hard link 提供 no-replace 发布；Android 只在 Tauri adapter 新建的独占 Picker cache 目录内使用相邻 rename。`rename` 本身不保证 no-replace，当前正确性依赖该独占目录前置条件；只有后续出现其他 Android backend 调用方或实测竞态，才研究 `renameat2` 等替代；
- Android `ACTION_CREATE_DOCUMENT` 会先创建 provider 文档；完整 cache 制品向 content URI 复制时若失败，provider 可能留下不完整目标。Atha 会报告失败并清空自身 cache，但不能对所有 provider 承诺删除外部残留；
- 当前最高证据是 x86_64 模拟器功能链路，不覆盖 ARM 真机的内存、I/O、WebView 或词典性能，也不是签名发布证据。
- CBZ 共用同一 Android picker、私有数据根、reader runtime 和 Locator 恢复链路；`-VerifyCbzFixture` 已验证逐页、坏页继续、日志隐私与 app PSS，并在 renderer 不能唯一归因时明确不生成数值。
- 离线书架的 opt-in AVD 门使用真实本地 EPUB，在干净数据上覆盖导入后的本地搜索、默认 / 进度 / 书名 / 作者、选择 / 全选 / 取消 / 批量移出、返回空态、44 CSS px 触控、应用健康和 logcat / `Atha.log*` 隐私扫描；受限原生 bridge 按黑色书架与阅读主题同步系统栏图标明暗。证据仍是单书模拟器链路，不冒充多书排序、ARM 真机或大书架性能。

## 已实现能力

### FFI 对照

- 共享 C 头文件；
- C++ 与 Rust 动态库；
- ABI 版本、空调用、1 MiB 字节校验、字符串跨边界分配与释放；
- 统一动态加载 runner；
- Rust 单元测试与 CTest。

### SQLite 对照

- `Work`、`Edition`、`Conversation`、`Message`、`MessageRevision`、`SourceAnchor` 与 `OutboxEvent` 骨架；
- WAL、外键、FTS5 外部内容表和同步触发器；
- 当前修订归属外键；
- 强制 Outbox 失败后的整事务回滚验证；
- 10,000 消息、修订和 Outbox 的本地冒烟。

### 正式消息式阅读

- schema v2 `MessageStore`、不可变修订、墓碑、回复、正反向引用、SourceAnchor/SourceSnapshot 版本、当前修订 FTS5 与事务 Outbox；
- HTML/CSS/呈现参数与图片资源双层信任边界校验，资源按 SHA-256 内容寻址；
- localStorage 标注原子幂等迁移、Edition/单对话自包含 ZIP 导出与公开完整性检查；
- Tauri TypeScript client、根 Message 标注/笔记投影、全屏筛选、半屏/全屏对话浮层、本条/本章/本书记录及时间/书序投影、受限富文本与 Markdown 输入、历史/关系/快照/跳回和本书导出入口。

## 验证证据

本文件只登记验证入口和当前最高证据等级，不保存逐次运行流水。精确提交、命令、计数、基准 run id 与关闭收据由 Git 和 `project-workflow` 保存。

| 范围 | 正式入口 | 当前最高证据 |
| --- | --- | --- |
| 文档与流程 | `scripts/Invoke-Atha.ps1 check docs` | Windows 本地静态检查 |
| 后端 | `scripts/check-backend.ps1` | Windows 本地构建、lint、测试与文档 |
| 阅读内核 | `scripts/check-reader-samples.ps1`、`scripts/check-reader-gate.ps1` | 真实 Windows WebView2 困难样本、恢复、内存与性能门槛 |
| 产品入口 | `scripts/check-tauri-reader.ps1`、`scripts/check-library-shelf.ps1` | 真实 Windows Tauri / WebView2 本地链路 |
| 离线书架 | `node --test reader/app/tests/library.test.ts`、`scripts/check-library-shelf.ps1`、`scripts/check-android-reader.ps1 -VerifyLibraryShelfUi` | 搜索 / 排序 / 严格进度 / 部分失败逻辑，Windows 真壳与四视口渲染，以及 API 35 x86_64 16 KiB AVD 单书交互、截图和双日志隐私检查 |
| 消息与数据 | `scripts/check-message-reading.ps1`、`scripts/check-tauri-reader.ps1` | 真实 Windows Tauri / WebView2 消息闭环，以及本地集成测试、前端构建、Tauri seam 与权限检查 |
| Android EPUB 纵切 | `scripts/check-android-reader.ps1`；活动 change 记录的消息 SAF opt-in 链路 | API 35 x86_64 16 KiB 模拟器上的构建、安装、冷启动、系统 picker 导入 / 打开 / 重启持久，以及消息 export / backup / restore；不是 ARM 真机性能证据 |
| EPUB2 / NCX 子集 | `cargo test -p atha-backend --test epub_import`、EPUBCheck 5.3.0、`scripts/check-android-reader.ps1 -VerifyEpub2NcxFixture` | 动态原创 fixture 通过规范 oracle；Windows WebView2 与 API 35 x86_64 16 KiB Android 模拟器已验证目录跳转和强停后同一 section / page 恢复 |
| CBZ JPEG / PNG | `cargo test --locked -p atha-backend --test cbz_import`、`scripts/check-cbz-source.ps1`、`scripts/check-android-reader.ps1 -VerifyCbzFixture` | 动态原创 fixture 的 importer、安全矩阵和 reader 坏页自检已通过；Windows WebView2 与 API 35 x86_64 16 KiB Android 模拟器已验证逐页、坏页继续和强停恢复 |
| 公式性能 | `scripts/check-reader-formula-performance.ps1` | 固定真实 EPUB 章节的十样本本地 benchmark |

这些结果不是 CI、安装包、生产环境或跨设备证据。源码、依赖、配置或样本变化后，应重新运行受影响的最小入口；只有最终候选才扩展到 required gate。
## 已知缺口

缺口描述 as-built 事实，不代表已经排期；优先级只由 `docs/roadmap/ROADMAP.md` 和活动 change 决定。

| 类别 | 当前缺口 |
| --- | --- |
| 产品回流 | 书架没有文件关联、拖放、分组或最近阅读；尚无多书 Android 排序、超大书架滚动与虚拟化性能证据 |
| 格式与引用 | EPUB2 首版仍是 UTF-8 XHTML / NCX 子集；CBZ 首版只有 JPEG / PNG，不含 RTL / spread / 区域标注；未完成 UTF-16、DTBook、完整 fallback、其他格式来源、跨内容版本 Locator 重锚定或富文本迁移 |
| 数据与设备 | 没有加密、checkpoint、全应用备份或跨设备同步 |
| 交付 | 没有 CI、Windows 安装包或签名 Android 发布包；Android 当前只验证 x86_64 debug APK 与模拟器 |
| 工程结构 | 旧 Wry / Tao host 尚未删除；reader runtime 仍由固定顺序组成单 bundle |
| 性能证据 | 基准没有设备指纹，也没有跨日期重复运行统计；Android 尚无 ARM 真机内存、I/O、WebView 或词典 benchmark |

## 正式代码约定

正式后端使用 `backend/`，测试靠近所属 crate；P0 实验继续保留在 `p0/`。新增 module 或依赖必须由已获批准的 `accepted` change 与真实用例驱动，不能用空骨架预留。

## 相关文档

- 架构：`docs/architecture/OVERVIEW.md`
- 数据库：`docs/codebase/DATABASE.md`
- 路线图：`docs/roadmap/ROADMAP.md`

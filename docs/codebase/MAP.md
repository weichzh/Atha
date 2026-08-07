# 代码库地图

## 仓库状态

当前生产代码包含根 Cargo workspace、正式后端 crate、EPUB3 导入、本地书架、正式消息数据库、Tauri 2 / WebView2 host、Svelte 5 应用壳和无框架阅读内核；直接 Wry / Tao host 暂留为回归基线。精确演进历史由 Git 保存，本文件只描述当前结构。

## 顶层结构

| 路径 | 责任 | 状态 |
|---|---|---|
| `.cargo/config.toml` | RsProxy sparse index 与 Cargo 网络配置 | 已配置 |
| `Cargo.toml`、`Cargo.lock` | 正式 virtual workspace 与锁文件 | M3 已验证 |
| `backend/atha-backend/` | 正式后端库、书根资源边界、EPUB3 导入、本地书架、消息数据库与阅读遥测校验 | 已实现 |
| `reader/app/` | Tauri 2、Vite、Svelte 5 产品入口；书架、应用壳、能力清单、受控协议和打包配置 | 已验证 |
| `reader/app/src-tauri/src/lib.rs` | Tauri composition root，以及当前仍同文件的 library、telemetry、固定字段平台日志与 protocol adapter | 已验证 |
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
| `scripts/check-message-reading.ps1` | 正式消息集成测试、前端检查/build、Tauri/host 测试及 command / permission 映射 | 已通过 |
| `scripts/check-epub-source.ps1` | 固定 EPUB3 的 Rust 检查、真实导入形状与 WebView2 import probe | M3 已通过 |
| `scripts/check-library-shelf.ps1` | 本地书架后端、production build、真实 Tauri 无参数启动与移动书架 UI | M4 已通过 |
| `scripts/Invoke-Atha.ps1` | 统一工程 CLI；自动记录 `check docs`、`station` 与 `report` | 本地已验证 |
| `scripts/Measure-Workflow.ps1` | schema v1/v2 本机流程日志、兼容汇总与自检 | 本地已验证 |
| `docs/agents/workflow.md`、`docs/agents/references.md` | 全局工作流的项目契约，以及外部技术的官方入口和快速用法 | 已配置 |
| `docs/` | 当前事实、路线、长期决策、活动 change 与未决研究 | 已建立 |

`p0/` 只保存技术验证，不是生产后端。后续正式代码不得直接在 P0 目录上堆叠。

### 正式后端基线

- workspace 包含 `atha-backend`、`atha-reader-host` 与 `atha-reader-app`，并显式排除 P0 Rust crate；产品 app 依赖 backend 与 host 的共享 Windows 启动 / 诊断代码，host 只依赖 backend；
- 版本 `0.1.0`、edition 2024、Rust `1.97.1` 和禁止 unsafe 的 lint 由 workspace 统一；
- 后端 crate 使用 `zip`、`quick-xml`、`sha2`、`serde` 与 `serde_json` 处理 EPUB，并用 `dom_query` 与锁定的 `rusqlite 0.40.1` bundled SQLite 实现严格快照校验和消息事实；没有 repository trait 或多格式工厂；
- 根锁文件包含正式后端导入、消息数据库、Tauri 官方日志插件与固定版本的 Wry/Tao 承载依赖，P0 继续保留独立锁文件；
- `backend::messages::MessageStore` 拥有 schema v2 只向前迁移、WAL、外键、FTS5、事务 Outbox、内容寻址资产、旧标注迁移、自包含交换导出及 schema 1 完整备份 / 恢复。

### HTML 阅读切片

- `BookRoot` 规范化书根并拒绝编码、路径、符号链接、文件类型、MIME 与大小越界；reader manifest 已声明的 section 以 XHTML 返回，因此不依赖源文件扩展名，也不把未声明的无扩展名文件当作 XHTML；
- schema 1 manifest 声明内容版本、有序 section、资源和可选 TOC；Windows host 的 `--epub` 与 `--book-root` 输入互斥，后者再从 `--manifest` 与兼容 `--entry` 二选一；
- `reader::epub` 的公开 interface 只有 `import_epub`：`mod` 编排内容哈希与原子缓存，`archive` 拥有 ZIP/路径/大小边界，`package` 拥有 container、OPF spine、navigation 和 schema 1 计划；XHTML 由 OPF media type 判定，navigation 仅额外允许标准 HTML5 DOCTYPE；同一源字节跨路径得到相同缓存根和状态键；
- `reader::library::LocalLibrary` 复用 EPUB importer，以内容哈希为身份，用每书一份 JSON 提供 `list`、`import`、`open`、`cover` 和 `remove`；移除记录不删除导入缓存或阅读状态；
- `atha` 与 `atha-book` 自定义协议只提供应用资源和当前书根资源；导航、新窗口、下载与外部请求默认拒绝；
- 原生 host 的 `main.rs` 只选择 Windows 入口；`windows.rs` 组合事件循环，`launch`、`protocol` 与 `diagnostics` module 分别拥有参数和窗口、受控资源、稳定状态键、日志与 benchmark；WebView2 使用持久 profile；
- 阅读页源码保持原生 ES module：`locator`、`navigation`、`preferences`、`session` 与 `pagination` 拥有既有阅读热路径；`content` 额外从已验证 Range 捕获 Snapshot 候选，并对具有显式宽高的 SVG 公式执行当前页优先校验、解码和章节内短期复用；`message-store` 把 Tauri Message client 适配为标注投影并迁移旧记录；`annotations` 负责选择、重选、重锚、高亮、根消息列表与筛选；`conversations` 负责回复、引用、修订、关系、快照、跳回、本条/本章/本书查询投影和本书导出；`diagnostics` 继续拥有验证与 benchmark；`app` 只组合流程并禁用默认右键菜单；
- 十八份页面源码由 Vite 或应用资源协议按固定顺序交付为单个 `atha-reader` runtime，避免为源码分层增加多次请求；浏览器验证服务器使用同一顺序，并对各 module 与拼接后的整体 bundle 运行语法检查；
- Locator 以内容版本、section id 和 DOM 文本 UTF-16 偏移表示 point/range；R2 range 限于单 section 并检查实际文本边界，无效输入安全回落并留下诊断，页码不作为内容坐标；窗口重排暂时无法测量文字矩形时保留已校验偏移和当前页，错误界面显示稳定代码与阶段而不暴露书籍内容；
- 上一页和下一页可跨 section；manifest TOC 与已有书签继续共用隐藏的原生 `select` 数据源，壳层把它投影为全屏目录按钮，书签紧随对应章节并通过 Locator 跳转；用户点击章节或书签后等待导航稳定并返回沉浸阅读；字号重排按变化前 Locator 恢复到包含同一偏移的页面；
- 应用默认拥有系统/浅色/纸张/深色主题、亮度、字号、字体、紧凑/标准/舒展密度和点击/滑动翻页；亮度只过滤阅读页，四边距不属于用户偏好，旧记录中的边距字段在恢复时忽略；本书覆盖只拥有书源样式和安全用户 CSS，两层分别校验和持久化；书签与进度按 host 提供的书籍状态键分区，位置高频写与低频状态分离；
- 公式按源尺寸随字号缩放，行间公式使用独立 `1.5` 倍率并在逻辑内容列中居中；
- 阅读页内部设备像素尺寸跟随 WebView 视口与 DPR，使用 CSS 多栏并以 `1 / devicePixelRatio` 隔离系统 DPI；移动阅读壳层默认沉浸，48 CSS px 工具栏只覆盖固定 144 设备像素的页眉页脚安全区且不参与分页；文字、公式和原子内容均有布局后裁切检查；
- Windows 窗口与壳层控件使用系统逻辑像素，默认内部尺寸为 430 × 820，最小为 360 × 640，可自由调整和最大化；窗口变化经 Navigation 队列恢复 Locator；
- 书内文档的宿主 IPC 只接收固定、限长、非内容性的性能与状态事件；
- Tauri 产品入口保持单 WebView；Svelte 组件拥有书架、应用壳和对话 DOM，Vite 直接拼接十八份 reader module，书籍 DOM、消息事实和分页热路径不进入组件状态；无阅读路由时不加载 reader bundle；
- Tauri `lib.rs` 组合状态、窗口、protocol、lifecycle、固定字段平台日志与 command 注册，并暂时保留 library、telemetry 与 protocol adapter；书架 command 只向可信壳暴露受限书目，不返回源路径或内容；`message_commands` adapter 统一校验当前阅读窗口并转发受限 DTO，`message_maintenance` adapter 只接受资料库根路由并在 blocking worker 执行全库备份 / 恢复；动态 `atha-book` 提供当前正文，独立 `atha-cover` 只读提供已登记封面；阅读器遥测复用后端白名单解析和共享 diagnostics，reader failure 额外携带固定阶段；官方日志插件只持久化 `atha::` target 的启动、导入 / 打开、reader 首稳 / ready / failure 和 protocol 5xx 数值事件，1 MiB 轮转并保留三份，不记录书籍或消息内容；消息专项检查精确核对 handler 注册与 permission；

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
| 消息与数据 | `scripts/check-message-reading.ps1`、`scripts/check-tauri-reader.ps1` | 真实 Windows Tauri / WebView2 消息闭环，以及本地集成测试、前端构建、Tauri seam 与权限检查 |
| 公式性能 | `scripts/check-reader-formula-performance.ps1` | 固定真实 EPUB 章节的十样本本地 benchmark |

这些结果不是 CI、安装包、生产环境或跨设备证据。源码、依赖、配置或样本变化后，应重新运行受影响的最小入口；只有最终候选才扩展到 required gate。
## 已知缺口

缺口描述 as-built 事实，不代表已经排期；优先级只由 `docs/roadmap/ROADMAP.md` 和活动 change 决定。

| 类别 | 当前缺口 |
| --- | --- |
| 产品回流 | 书架没有文件关联、拖放、分组、排序设置或最近阅读；尚未用日常使用验收确认哪些真的阻塞主循环 |
| 格式与引用 | 只支持本地 EPUB3；没有 EPUB2 / NCX fallback、多格式来源、跨内容版本 Locator 重锚定或富文本迁移 |
| 数据与设备 | 没有加密、checkpoint、全应用备份或跨设备同步 |
| 交付 | 没有 CI 或 Windows 安装包；Tauri 当前只验证 debug build |
| 工程结构 | 旧 Wry / Tao host 尚未删除；reader runtime 仍由固定顺序组成单 bundle |
| 性能证据 | 基准没有设备指纹，也没有跨日期重复运行统计 |

## 正式代码约定

正式后端使用 `backend/`，测试靠近所属 crate；P0 实验继续保留在 `p0/`。新增 module 或依赖必须由已获批准的 `accepted` change 与真实用例驱动，不能用空骨架预留。

## 相关文档

- 架构：`docs/architecture/OVERVIEW.md`
- 数据库：`docs/codebase/DATABASE.md`
- 路线图：`docs/roadmap/ROADMAP.md`

# 代码库地图

## 仓库状态

当前生产代码包含根 Cargo workspace、正式后端 crate、EPUB3、EPUB2 / NCX 子集、CBZ JPEG / PNG、FB2 / FBZ、Markdown 与 TXT 导入、本地书架、正式消息数据库、Tauri 2 产品 host、Svelte 5 应用壳和无框架阅读内核。Linux Tauri / WebKitGTK 已成为日常 GUI 目标，FB2 书架、目录、搜索与恢复通过正式 Linux WebDriver 入口；Android 保留已有 EPUB2、CBZ、Markdown 与 TXT 的模拟器纵切，仅在发布前或移动端专项验收时启动。直接 Wry / Tao host 暂留为 Windows 回归基线。精确演进历史由 Git 保存，本文件只描述当前结构。

## 顶层结构

| 路径 | 责任 | 状态 |
|---|---|---|
| `.cargo/config.toml` | RsProxy sparse index 与 Cargo 网络配置 | 已配置 |
| `Cargo.toml`、`Cargo.lock` | 正式 virtual workspace 与锁文件 | M3 已验证 |
| `backend/atha-backend/` | 正式后端库、书根资源边界、全部已支持书籍格式、本地离线词典、书架、消息数据库与阅读遥测校验 | Linux 本地已验证 |
| `backend/atha-backend/src/reader/dictionary.rs` | MDict v2 与经典 Kindle MOBI6 词典的事务导入、HUFF 累计偏移 sidecar、固定格式分派、精确查词、MDD 范围读取和安全富文本 / 纯文本双投影 | 私有英文输出与公共安全矩阵已验证 |
| `reader/app/` | Tauri 2、Vite、Svelte 5 产品入口；离线搜索 / 进度 / 排序 / 批量选择书架、应用壳、能力清单、受控协议和打包配置 | Linux / Windows / Android 已验证 |
| `reader/app/src-tauri/src/lib.rs` | Tauri composition root，以及当前仍同文件的 library、telemetry、固定字段平台日志与 protocol adapter | 已验证 |
| `reader/app/src-tauri/src/dictionary_commands.rs` | 离线词典 picker、Tauri command、blocking adapter 与 internal-only 固定字段日志策略 | Linux GUI 已验证 |
| `reader/app/src-tauri/src/platform_file.rs` | 普通路径与 Android SAF content URI 共用的流式 Picker cache bridge；RAII / 启动清理和输入大小边界 | Android 模拟器已验证 |
| `reader/app/src-tauri/src/runtime_diagnostics.rs` | Windows Recorder 与移动端交互诊断的目标平台选择；固定事件 token 由 backend 统一校验 | Windows / Android 已验证 |
| `reader/app/src-tauri/gen/android/`、`tauri.android.conf.json` | Tauri 官方 Android 工程、min SDK 26、compile / target SDK 36、manifest、应用图标与暗色系统栏样式 | x86_64 debug 已验证 |
| `reader/app/src-tauri/src/message_commands.rs` | 消息 IPC adapter；统一阅读路由校验、DTO 转发、稳定错误和原生导出 dialog | 已验证 |
| `reader/app/src-tauri/src/message_maintenance.rs` | 全库消息维护 IPC adapter；统一资料库根路由、备份 / 恢复 dialog 与 blocking worker | 已验证 |
| `reader/atha-reader-host/src/` | 共享 CLI、窗口尺寸和诊断逻辑；Wry/Tao 基线 host | 已验证 |
| `reader/atha-reader.html`、`reader/atha-reader.css` | 唯一阅读页结构、分页 / 原生滚动、Readest 风格设置抽屉、默认样式、可视阅读偏好、CSS 模块 fallback、书签、消息投影、搜索面板、对话浮层与内容 dialog | Linux GUI 与 PCT-AL10 自动验收已通过 |
| `reader/web/` | Locator、导航、偏好、输入与内容动作、阅读会话、状态、书签、搜索、消息适配/对话、标注投影、内容安全、分页、诊断、benchmark 和页面组合入口 | 已实现 |
| `reader/web/style-module-package.mjs` | schema 1 CSS 模块包的解析、序列化、大小 / 字段 / 重复 ID 与注入式 CSS 校验边界 | 已实现 |
| `reader/app/src/components/panels/DictionaryPanel.svelte`、`reader/app/src/dictionary.ts` | 本地词典管理、当前词典选择、选区查词与安全富文本词条；移动端使用 75% 高底部抽屉 | 本地构建与三视口合成界面已验证 |
| `reader/samples.json` | 四个本地验收样本的入口、manifest、内容、搜索和边界断言清单 | M2 已验证 |
| `p0/ffi/` | Rust/C++ 共享 C ABI 调用与所有权对照 | 本地 P0 实验 |
| `p0/sqlite/` | SQLite、FTS5、Outbox schema 与故障检查 | 本地 P0 实验 |
| `scripts/check-backend.ps1` | 正式后端 fmt、clippy、test 和 doc | M1 已通过 |
| `scripts/check-p0-ffi.ps1` | 构建两个 FFI 实现并运行统一 runner | 已通过 |
| `scripts/check-p0-sqlite.ps1` | 重建数据库并验证事务、FTS 与 10k 冒烟 | 已通过 |
| `scripts/check-reader-slice.ps1` | 构建实际 host，运行安全、布局和性能验收 | M2 已通过 |
| `scripts/check-reader-formula-performance.sh` | 通过忽略 sidecar 锁定私密公式压力样本，复用 Bash Linux Tauri 手势矩阵执行 5 次预热与 20 次逐场景 P95 benchmark | Linux GUI 正式门已通过 |
| `scripts/export_reader_sample.py` | 安全、可重复地从 EPUB 导出单章节、带 manifest 的多章节或 fixture-only 全 XHTML 验收样本 | M2 已通过 |
| `scripts/Serve-ReaderValidation.ps1` | 只读环回提供同一阅读页、manifest 和书根资源 | M2 R1 已通过 |
| `scripts/check-reader-samples.ps1` | 四样本实际 host、内容交互、状态、搜索、标注与明暗主题截图总验收 | M2 已通过 |
| `scripts/check-reader-wheel.ps1` | 真实浏览器媒体滚轮、连续离散输入接受率与输入到稳定页 P95 快速检查 | 已通过 |
| `scripts/check-reader-gate.ps1` | 组合四样本、大书搜索、进程树内存、强杀恢复和固定 P95 性能门槛 | M2 R8 已通过 |
| `scripts/check-tauri-reader.ps1` | Svelte production build、workspace Rust 检查、Tauri build、普通 EPUB 消息模式启动、导入探针与性能门槛 | 已通过 |
| `scripts/check-android-reader.ps1` | 固定 16 KiB x86_64 AVD 的 APK 构建、对齐、安装、冷启动，以及 opt-in picker / 阅读恢复、书架、Markdown / TXT 目录与搜索、PSS、截图和双日志隐私检查 | 模拟器 EPUB / CBZ / Markdown / TXT / 书架纵切已通过 |
| `scripts/android-webview-eval.mjs` | Android gate 通过 adb 转发的 WebView DevTools endpoint 执行单次受控表达式并返回 JSON；不依赖持久浏览器 daemon | 模拟器正式入口已验证 |
| `scripts/check-text-source.ps1` | Markdown / TXT backend、前端与保留桌面 host 的本地格式回归；私有 TXT 仅由显式环境变量 opt-in | 本地已验证 |
| `scripts/check-fb2-source.ps1` | 动态原创 FB2 / FBZ、安全矩阵、workspace / Svelte / Tauri build 与 opt-in Linux Tauri WebDriver 纵切 | Linux GUI 已通过 |
| `scripts/check-dictionary-source.sh` | 公共行为、私有 MDict / Kindle 兼容与 release benchmark 的 Bash 入口 | Linux 私有英文输出与性能已验证 |
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
- 后端 crate 使用共享 `zip 8.6`、`quick-xml 0.41`、`sha2`、`serde` 与 `serde_json` 处理 EPUB / CBZ / FBZ；EPUB 图片尺寸复用 `imagesize 0.15.0` 并以 `kamadak-exif 0.6.1` 校正方向，FB2 只新增 `base64 0.22.1`。Markdown / TXT 具体 adapter 使用 `pulldown-cmark 0.13.4`、`chardetng 1.0.0`、`encoding_rs 0.8.35` 与已锁定 `regex 1.13.1`。动态原创 CBZ writer 使用 dev-only `png 0.18.1`。`dom_query`、锁定的 `rusqlite 0.40.1` bundled SQLite 与 `fs2 0.4.3` 实现严格快照校验、消息事实和跨 Windows / Android 维护锁；没有 repository trait、锁服务或多格式工厂；
- 离线词典的 MDict v2 固定使用带 `lzo` 的 `mdict-rs 0.1.4`，经典 Kindle 只保留一个有界具体 parser；`dom_query` 复用为释义语义白名单和兼容纯文本投影，没有新增 sanitizer、provider registry、词典 factory、网络或句柄缓存；
- 根锁文件包含正式后端导入、消息数据库、`fs2 0.4.3`、Tauri 官方日志 / 文件系统插件与固定版本的 Wry/Tao 承载依赖，P0 继续保留独立锁文件；
- `backend::messages::MessageStore` 拥有 schema v2 只向前迁移、WAL、外键、FTS5、事务 Outbox、内容寻址资产、旧标注迁移、自包含交换导出及 schema 1 完整备份 / 恢复；维护锁通过 `fs2::FileExt` 实现，因为 Rust 1.97.1 的标准库 Unix 文件锁在 Android 返回 `Unsupported`。

### HTML 阅读切片

- `BookRoot` 规范化书根并拒绝编码、路径、符号链接、文件类型、MIME 与大小越界；reader manifest 已声明的 section 以 XHTML 返回，因此不依赖源文件扩展名，也不把未声明的无扩展名文件当作 XHTML；共享缓存完整性检查逐项确认 manifest 声明的 section / resource 位于书根、是受限普通文件且 section 非空；
- schema 1 manifest 声明内容版本、有序 section、资源和可选 TOC；Windows host 的 `--epub` 与 `--book-root` 输入互斥，后者再从 `--manifest` 与兼容 `--entry` 二选一；
- `reader::archive` 为 EPUB / CBZ / FBZ 共享 crate-private `zip 8.6` 打开、路径、重复 / 重叠、加密、symlink、成员和声明解压总量边界；写入成员与 CBZ 页面另按实际读取量累计，少量 EPUB metadata 与单个 FBZ XML 读取只受单成员上限约束。`zip 8.6` 没有 pre-allocation `max_entries` API，因此打开前只以标准 terminal EOCD hint 拒绝超过 10000 项、trailing garbage 与歧义 terminal EOCD，打开后再校验条目数；fallback / ZIP64 在 post-open 检查前的最坏预分配是受 512 MiB 源文件上限约束的残余风险；
- `reader::epub` 的公开 interface 是 `import_epub`：`mod` 编排内容哈希、原子缓存和本地图片的有界原生宽高提示，`package` 拥有 container、OPF2 / OPF3 spine、navigation 和 schema 1 计划；OPF2 只从 `spine@toc` 找到有界 NCX，把嵌套 `navPoint` 按前序拍平成现有 TOC，OPF3 继续使用唯一 XHTML nav；v2 / v3 / v4 / v5 完整缓存都可读，有耐久源时把 v2 至 v4 按需升级到 v5；XHTML 由 OPF media type 判定，同一源字节跨路径得到相同缓存根和状态键；
- `reader::cbz::import_cbz` 只接受 JPEG / PNG，按路径分段自然序生成一图一 XHTML section 与声明资源；`ComicInfo.xml` 只投影 `Title`、`Writer` 与唯一有效 `FrontCover`，`imagesize` 校验类型和像素预算，WebView decode 失败时显示可导航坏页；
- `reader::fb2::import_fb2` 以 `quick-xml` 两遍有界流式解析直接 FB2 或单根成员 FBZ，投影 metadata、正文 / notes sections、目录、内部链接和 JPEG / PNG 图片；DTD、处理指令、外链、源 stylesheet、主动内容、未知正文元素、损坏引用与资源越界稳定拒绝；
- `reader::library::LocalLibrary` 以允许列表扩展名严格分派 EPUB / CBZ / FB2 / FBZ / MOBI / AZW / AZW3 / Markdown / TXT；Android content URI 由 Tauri PathPlugin 读取显示文件名后保留同一允许列表后缀，不从 URI 或正文猜格式。EPUB / CBZ 保持裸 SHA-256 身份，FB2 / FBZ 共享解包 XML 的固定格式域身份，Kindle 三后缀共享一个格式域，Markdown / TXT 用不同固定格式域隔离相同字节；加入书架只把源写入 `SourceBooks` 并登记，首次打开在 blocking worker 中调用既有 importer，同进程只发布一次准备结果；后续按精确 marker、元数据和 manifest 声明验证并复用 `ImportedBooks`，不完整缓存从任一同身份耐久后缀重建；再次登记复用同身份健康源，以验证后的 staging 原子覆盖身份异常源，并重建损坏记录；每书一份兼容 JSON 提供 `list`、`stage`、`import`、`open`、`cover` 和 `remove`，移除记录不删除耐久源、导入缓存或阅读状态；
- `reader::dictionary::LocalDictionaries` 在独立 `Dictionaries` 目录按固定格式域事务导入 MDX / MDD 或经典 MOBI6，MDict 复用成熟 reader，Kindle 以稀疏索引只读目标 records；精确查询结果同时生成兼容纯文本和固定元素白名单富文本，来源脚本、样式、属性与资源均不进入结果，Tauri 日志不记录路径、查询、词头、释义或资源；
- `atha`、`atha-book` 与 `atha-cover` 自定义协议只提供应用资源、当前书根与已登记封面；Windows / Android 使用 `https://*.localhost`，Linux 使用 `<scheme>://localhost`。同书校验比较协议与 host，不能依赖 custom scheme 恒为 `null` 的 `URL.origin`；导航、新窗口、下载与外部请求默认拒绝；
- 原生 host 的 `main.rs` 只选择 Windows 入口；`windows.rs` 组合事件循环，`launch`、`protocol` 与 `diagnostics` module 分别拥有参数和窗口、受控资源、稳定状态键、日志与 benchmark；WebView2 使用持久 profile；
- 阅读页源码保持原生 ES module：`locator`、`navigation`、`preferences`、`session` 与 `pagination` 拥有既有阅读热路径；`interaction` 以一次序列一个 owner 仲裁翻页、横向溢出和内容激活，`pagination` 用单个 rAF 写入拖动预览，长章节改写原生横向滚动，并缓存稳定页与当前 section 的 fragment 偏移；`session` 保留 live DOM 直到目标准备和排版完成，失败时恢复上一稳定内容与位置；`content` 用共享三槽和 8 Mi 字符预算复用已校验 detached section 或相邻 XHTML 原文，并以 generation 与 Promise identity 拒绝关闭后的在途回写；`reader-state` 拥有偏好、书签、进度和应用级阅读统计记录；`content` 与 `search` 在解析前只白名单并剥离 HTML5、XHTML 1.1 和兼容扩展 XHTML 1.0 Strict 固定声明，主动内容仍拒绝；`content` 额外从已验证 Range 捕获 Snapshot 候选，并对具有显式宽高的 SVG 公式执行当前页优先校验、解码、章节内短期复用和稳定书内几何缓存，成功显现不触发逐页重排，初次可见资源加载后由 `pagination` 统一再计一次，失败替换才在首次布局变化前捕获 Locator；`message-store` 把 Tauri Message client 适配为标注投影并迁移旧记录；`annotations` 负责选择、重选、重锚、高亮、根消息列表与筛选；`conversations` 负责回复、引用、修订、关系、快照、跳回、本条/本章/本书查询投影和本书导出；`diagnostics` 继续拥有验证与 benchmark；`app` 只组合流程并禁用默认右键菜单；
- 十九份页面源码由 Vite 或应用资源协议按固定顺序交付为单个 `atha-reader` runtime，避免为源码分层增加多次请求；浏览器验证服务器使用同一顺序，并对各 module 与拼接后的整体 bundle 运行语法检查；
- Locator 以内容版本、section id 和 DOM 文本 UTF-16 偏移表示 point/range；R2 range 限于单 section 并检查实际文本边界，无效输入安全回落并留下诊断，页码不作为内容坐标；窗口重排暂时无法测量文字矩形时保留已校验偏移和当前页，错误界面显示稳定代码与阶段而不暴露书籍内容；
- 上一页和下一页可跨 section；manifest TOC 与已有书签继续共用隐藏的原生 `select` 数据源，壳层把它投影为全屏目录按钮，书签紧随对应章节并通过 Locator 跳转；用户点击章节或书签后等待导航稳定并返回沉浸阅读；字号重排按变化前 Locator 恢复到包含同一偏移的页面；
- 应用默认拥有系统/浅色/纸张/深色主题、亮度、16–40 逻辑 CSS px 字号、字体和紧凑/标准/舒展密度；本书覆盖拥有左右翻页 / 上下滚动、书源样式、24 / 32 / 48 左右边距、顶格 / 2em 段首缩进、段距和最多 32 个有序 CSS 模块。旧点击 / 滑动开关和自由边距字段在恢复时忽略。旧单段 CSS 无损迁入本地模块，超过新组合上限时停用但不丢弃。模块包解析、序列化与结构限制由独立 codec 复用 `content.validateStylesheet()`，Preferences 保留 UI、按模块预览 timer、组合和每书持久化；CodeMirror 只按需增强同一 textarea，失败统一回滚；书签与进度按 host 提供的书籍状态键分区，位置高频写与低频状态分离；阅读统计使用独立的有界应用记录，只累计稳定、沉浸、可见、聚焦且未闲置的短区间，并投影今日、近 7 天、本书和连续阅读；
- 公式按源尺寸随字号缩放，行间公式使用独立 `1.5` 倍率并在逻辑内容列中居中；
- 阅读页内部设备像素尺寸跟随 WebView 视口与 DPR，字号以逻辑 CSS px 保存并按 `字号 × DPR` 写入正文，再以 `1 / devicePixelRatio` 隔离系统 DPI；分页模式使用 CSS 多栏，显示宽度不超过 20,000px 的章节横向 transform，超过阈值的长章节改用原生 `scrollLeft`，滚动模式则启用单栏原生纵向滚动。分页总数使用从书根到最后有意义内容的 Range，并忽略尾部空盒；待加载与已加载公式统一使用零字体和零行高，使旧 Chromium 不会把无 `src` 图片的 `alt` 回退文本分进多栏。移动阅读壳层默认沉浸，48 CSS px 工具栏只覆盖固定 144 设备像素的页眉页脚安全区且不参与分页；普通图片按页内可用面积等比限幅，符合 v5 增强条件的图片以原生宽高与书源 CSS 之前最多 512 个零特异性 `contain-intrinsic-size` 规则稳定解码前几何，常见未分层作者和用户 CSS 可继续覆盖；几何盒随正文出现，像素异步绘制，不增加普通图片或正文揭示闸门。失败占位优先保持连接状态下的非零实际盒，再退回合法尺寸属性。表格在正文固定页宽并裁掉超高内容，全屏投影保留 DOM 结构，待加载公式以三个并发的可取消 worker 等待各自真实终态并渐进填充，图片和表格均可缩放及双向滚动；
- Windows 窗口与壳层控件使用系统逻辑像素，默认内部尺寸为 430 × 820，最小为 360 × 640，可自由调整和最大化；窗口变化经 Navigation 队列恢复 Locator；
- 书内文档的宿主 IPC 只接收固定、限长、非内容性的性能与状态事件；
- Tauri 产品入口保持单 WebView；Svelte 组件拥有书架、应用壳和对话 DOM，书架只对受限 DTO 做本地标题 / 作者搜索、严格进度二态、稳定排序与显式批量选择；Vite 直接拼接十九份 reader module，书籍 DOM、消息事实和分页热路径不进入组件状态；无阅读路由时不加载 reader bundle，CodeMirror chunk 只在进入 CSS 模块页后加载；
- Tauri `lib.rs` 组合状态、窗口、protocol、lifecycle、固定字段平台日志与 command 注册，并暂时保留 library、telemetry 与 protocol adapter；书架 command 只向可信壳暴露受限书目和 `prepared`，不返回源路径或内容，首次打开在 blocking worker 准备；`message_commands` adapter 统一校验当前阅读窗口并转发受限 DTO，`message_maintenance` adapter 只接受资料库根路由并在 blocking worker 执行全库备份 / 恢复；动态 `atha-book` 在共享读锁内读取当前书根，不再逐资源深拷贝，独立 `atha-cover` 只读提供已登记封面；阅读器遥测复用后端白名单解析和共享 diagnostics，reader failure 额外携带固定阶段；官方日志插件只持久化 `atha::` target 的启动、书架、消息内部存储故障、reader 首稳 / ready / failure 和 protocol 5xx 固定字段事件，1 MiB 轮转并保留三份，不记录书籍或消息内容；预期输入、并发和安全拒绝保持静默；消息专项检查精确核对 handler 注册与 permission；

### Linux Tauri 目标

- 日常 GUI 使用当前 GNOME Wayland 会话中的 Tauri / WebKitGTK，不启动 Android 模拟器；发布前与移动端专项验收才恢复 Android 门禁；
- Linux 应用根是 `tauri://localhost`，书根与封面分别是 `atha-book://localhost`、`atha-cover://localhost`；平台常量统一供路由、维护 command、前端资源和 CSP 使用；
- `scripts/check-reader-linux.sh` 使用官方 `tauri-driver` 与系统 WebKitWebDriver 驱动真实 Tauri 壳，隔离 XDG 数据并在结束后清理；创建窗口后先拒绝 hidden 或零尺寸的无活动显示环境，避免 rAF 永久等待。当前 Bash 门覆盖完整导入诊断、跨 section 末页回退、普通 / 公式 / 表格 / 内部滚动与双向边界场景及 AppLog 隐私。拖动帧指标使用 pointer down 至 pointer up 间全部连续 rAF 的相邻间隔，并独立检查视觉更新数量；它是 WebKit 主线程 cadence，不是 compositor presentation 或真实 FPS。门禁请求 W3C touch Actions 并核对可信事件，但当前 WebKitGTK 实际报告 `mouse`，因此它不是实体触摸证据。

### Android EPUB 纵切

- Tauri mobile crate output、mobile entry point 与 target-gated Windows host 已接通；Windows 仍使用 `%LOCALAPPDATA%\Atha`，Android 使用 Tauri `app_local_data_dir`，两端共用 LocalLibrary、MessageStore 与 reader kernel；
- Android 工程固定 min SDK 26、compile / target SDK 36；本机 gate 固定 Node 24.1.0、JDK 21、NDK 28.2.13676358，默认运行 `Atha_API_36_16K`（API 36、x86_64、16,384-byte page）AVD，并保留参数复核历史 API 35 证据；APK 通过 16 KiB ZIP alignment 与全部 x86_64 ELF `LOAD 0x4000` 检查；
- `platform_file::PickerInput` / `PickerOutput` 对普通路径零复制，对 SAF content URI 使用锁文件已有的官方 `tauri-plugin-fs` 流式复制到应用 `cache/Picker`；导入仍限制单次 32 本和单本 512 MiB，恢复仍限制 8 GiB，消息导出和备份仍由 backend 限制为 512 MiB 与 8 GiB；每个 cache 目录独占创建，Drop 与启动均清理；
- 系统 picker 链路已在模拟器手工验证 EPUB 导入 / 打开 / 重启恢复、消息导出及全库备份 / 恢复，完成后 Picker cache 为空；manifest 不请求宽泛存储权限，设置 `allowBackup=false` 并以 API 31+ `dataExtractionRules` 排除 cloud backup 与 device transfer；
- Android app storage 实测 hard link 返回 `PermissionDenied`。非 Android 备份继续用 hard link 提供 no-replace 发布；Android 只在 Tauri adapter 新建的独占 Picker cache 目录内使用相邻 rename。`rename` 本身不保证 no-replace，当前正确性依赖该独占目录前置条件；只有后续出现其他 Android backend 调用方或实测竞态，才研究 `renameat2` 等替代；
- Android `ACTION_CREATE_DOCUMENT` 会先创建 provider 文档；完整 cache 制品向 content URI 复制时若失败，provider 可能留下不完整目标。Atha 会报告失败并清空自身 cache，但不能对所有 provider 承诺删除外部残留；
- 当前通用 Android 应用最高证据仍是 x86_64 模拟器完整 picker 链路；PCT-AL10 另有 arm64 debug APK 的阅读设置、字号 / DPR、水平翻页、原生纵向滚动和跨章专项证据，以及离线词典 release 原生查词 / RSS、Tauri 应用 PSS、华为 WebView 114 原生选区和直接点击查词证据。这些都不等同于签名发布验收。
- CBZ 共用同一 Android picker、私有数据根、reader runtime 和 Locator 恢复链路；`-VerifyCbzFixture` 已验证逐页、坏页继续、日志隐私与 app PSS，并在 renderer 不能唯一归因时明确不生成数值。
- Markdown / TXT 共用同一 picker、书根、ReaderManifest、Locator、搜索与恢复链路。Markdown 固定把 raw HTML / 活动链接 / 图片能力投影为惰性文本，代码块使用 Readest 同型 `pre-wrap` 且允许跨页；TXT 以高置信整行标题生成 TOC、按约 1 MiB 合并物理 sections。API 36 x86_64 16 KiB AVD 已用仓库 README 与私有真实 TXT 覆盖目录首 / 中 / 末、全文搜索、翻页、强停恢复、Picker cache、PSS、健康和双日志隐私；十样本只作为同环境基线，ARM64 真机仍未覆盖。
- Android edge-to-edge 由 native `WindowInsetsCompat` 提供 `systemBars | displayCutout` 四边事实，Web 端只消费一套自有 CSS 变量；阅读态隐藏状态栏但保留导航栏，章节标题仍在 cutout 下方，工具打开时章节标题隐藏且顶部工具栏完整避让状态栏。
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
| Markdown / TXT | `cargo test --locked -p atha-backend --test text_import`、`scripts/check-text-source.ps1`、`scripts/check-android-reader.ps1 -VerifyMarkdownText` | 仓库 Markdown 与私有 opt-in TXT 的 importer、安全矩阵、API 36 x86_64 16 KiB picker / 目录 / 搜索 / 翻页 / 强停恢复和十样本 TXT 相对基线已通过；未完成 ARM64 真机性能门 |
| FB2 / FBZ、阅读统计与手势 | `node --test reader/web/reader-state.test.mjs`、`cargo test --locked -p atha-backend --test fb2_import`、`bash scripts/check-reader-linux.sh` | 动态原创 fixture 的 importer、安全矩阵、Windows-1251 与内容身份已通过；当前 Bash Linux Tauri 门验证完整导入、跨章末页、AppLog 隐私及可信自动化指针，实际指针类型为 `mouse`，实体触摸待用户实测 |
| MOBI / AZW / AZW3 | `cargo test --locked -p atha-backend --test kindle_import`、`scripts/check-kindle-source.ps1 -VerifyLinuxGui` | `boko 0.5.0` 前置有界预检、两个私有普通 KF8、词典早拒绝和相同字节跨后缀身份已通过；真实 Linux Tauri / WebKitGTK 已验证 204 条唯一目录、搜索、重排、恢复、非空截图和 AppLog 隐私；源 flow stylesheet 与 Android ARM64 性能尚未完成 |
| MDict / Kindle 离线词典 | `cargo test --locked -p atha-backend --test dictionary_lookup`、`bash scripts/check-dictionary-source.sh --private-fixtures fixtures/local` | 私有 MDict v2 / MDD 与经典 Kindle MOBI6 的固定英文查询 / 兼容纯文本哈希、范围读取、安全富文本矩阵和 release benchmark 已通过；Kindle HUFF 使用真实累计 record 偏移。当前 Linux 冷 / 热 P95 为 Kindle 6.296 / 5.319ms、MDict 0.877 / 0.891ms、MDD 0.459 / 0.465ms，RSS 27,644 KiB。桌面浅色、移动浅色与移动深色合成词条已验证边界、滚动和无横向溢出；既有 PCT-AL10 arm64 查词、选区和抽屉证据早于富文本变更，当前富文本尚未在真机重跑 |
| 公式与手势性能 | `bash scripts/check-reader-formula-performance.sh --epub <path>`、`bash scripts/check-pct-reader-fps.sh --device <serial> --duration 10` | 当前源码连续两次通过 Bash Linux Tauri 5 + 20 正式门，每轮记录 440 次动作；两轮最差聚合页面状态更新 / 点按 / 连续 rAF P95 / 最大 rAF / 稳定分别为 32 / 7 / 17 / 19 / 352ms 与 32 / 7 / 19 / 20 / 352ms。公式在测量前已全部稳定，因此不证明加载揭示体验。最终 PCT-AL10 包在公共书同节、跨节和用户公式书的六个自动前后滑窗口分别提交 11、12、13、14、15、7 个 SurfaceFlinger presentation，页面均落到预期页并在动作后进入 `no-new-buffer`；release monitor 为 raw-only，不能据此报告总体 P95 或自然手指手感 |

这些结果不是 CI、安装包、生产环境或跨设备证据。源码、依赖、配置或样本变化后，应重新运行受影响的最小入口；只有最终候选才扩展到 required gate。
## 已知缺口

缺口描述 as-built 事实，不代表已经排期；优先级只由 `docs/roadmap/ROADMAP.md` 和活动 change 决定。

| 类别 | 当前缺口 |
| --- | --- |
| 产品回流 | 书架没有文件关联、拖放、分组或最近阅读；尚无多书 Android 排序、超大书架滚动与虚拟化性能证据 |
| 格式与引用 | EPUB2 首版仍是 UTF-8 XHTML / NCX 子集；CBZ 首版只有 JPEG / PNG，不含 RTL / spread / 区域标注；FB2 首版拒绝源 stylesheet、非 JPEG / PNG binary 与未知正文元素，FBZ 单成员受 16 MiB archive 上限；Kindle 首版不发布 `boko` raw API 无法读取的 KF8 flow stylesheet，也不支持 DRM、字典阅读、KFX、AZW4 或压缩字体；Markdown 不加载活动链接 / 图片，TXT 遗留编码识别是 best effort；未完成 UTF-16 EPUB、DTBook、完整 fallback、跨内容版本 Locator 重锚定或富文本迁移 |
| 数据与设备 | 没有加密、checkpoint、全应用备份或跨设备同步 |
| 交付 | 没有 CI、Linux / Windows 安装包或签名 Android 发布包；Linux 当前验证 debug Tauri 壳，Android 验证 x86_64 模拟器与 PCT-AL10 arm64 debug APK，不等同于签名发布验收 |
| 工程结构 | 旧 Wry / Tao host 尚未删除；reader runtime 仍由固定顺序组成单 bundle |
| 性能证据 | 基准没有完整设备指纹，也没有跨日期重复运行统计；PCT-AL10 已有词典 release 原生查词 / RSS、Tauri 应用 PSS 与缓存打开 1ms smoke，但仍缺 SAF 首次准备 / 书籍 I/O 及通用 WebView 基准 |

## 正式代码约定

正式后端使用 `backend/`，测试靠近所属 crate；P0 实验继续保留在 `p0/`。新增 module 或依赖必须由已获批准的 `accepted` change 与真实用例驱动，不能用空骨架预留。

## 相关文档

- 架构：`docs/architecture/OVERVIEW.md`
- 数据库：`docs/codebase/DATABASE.md`
- 路线图：`docs/roadmap/ROADMAP.md`

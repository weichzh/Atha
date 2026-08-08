# Android Markdown / TXT 纵向切片

## Status

implemented

## Problem

Atha 已在 Windows 与 Android 共用同一 ReaderManifest、BookRoot、Locator、本地书架和 reader runtime，但 LocalLibrary 仍只接受 EPUB / CBZ。Readest 已成熟支持 Markdown 与 TXT；用户也已指定 Markdown 使用仓库现有文档、TXT 使用 `fixtures/local` 中的真实网文，不允许为正向验收另造书籍。

当前真实 TXT 样本的只读预检得到 7,362,028 bytes、92,533 行和约 1,134 个高置信章节候选，无 BOM 且不是严格 UTF-8。最初候选实现把全局 sections 上限提高到 2,000 并一章一 section；API 36 x86_64 16 KiB AVD 实测目录可以绑定，但全文搜索经 Android 自定义协议逐个读取 1,135 个 XHTML，超过 5 分钟仍未完成，已按预设停止条件判定 no-go。最终切片必须保留章节目录语义，同时把协议请求数量压回有界范围。

## Scope

- 在 `backend::reader` 下增加具体 Markdown 与 TXT importer，由现有 `LocalLibrary::import` 按 `.md` / `.markdown` / `.txt` 严格分派，直接生成 schema 1 ReaderManifest 和受控 XHTML sections；继续使用内容指纹、同文件系统 staging、源未变校验、原子发布和已验证缓存，不经由临时 EPUB。
- Markdown 固定使用 `pulldown-cmark 0.13.4`（MIT）。只显式开启 GFM、表格、脚注、删除线、任务列表和 YAML-style metadata block；原始 HTML 作为普通文本转义，链接仅保留可读 label，图片仅保留 alt / 占位，不读取本机路径也不请求网络。第一个 H1 之前的可见内容为前言，每个 H1 开启一个 section，无 H1 时只有一个 section；不实现自定义 slug 或第二套 locator。Markdown 源文件限制为 16 MiB。
- TXT 固定使用 `encoding_rs 0.8.35`（MIT OR Apache-2.0）与 `chardetng 1.0.0`（MIT OR Apache-2.0）：BOM 优先，无 BOM 的严格 UTF-8 其次，其他遗留编码使用锁定 detector 的 best-effort 结果。为满足“至少两个主章节候选才分章”且不保留完整文本，锁定最小三遍本地读取：第一遍检测 BOM / 严格 UTF-8 并向 detector 喂样，第二遍有界解码并只计数章节候选，第三遍再有界解码并逐章写入 XHTML。不替换错误字节，不在 staging 持久化规范化正文，不在内存同时保留完整解码文本与全部 XHTML。无 BOM UTF-16 稳定拒绝，真实样本证明自动选码错误前不增加手工编码 UI。
- TXT 章节规则只使用已在锁文件中的 `regex 1.13.1`（MIT OR Apache-2.0）编译一个首尾锚定 pattern，识别不超过 80 个 Unicode 标量的整行“第…章 / 节 / 回 / 篇”与有限前后记词；至少两个主规则候选才启用语义分章，首候选前的非空正文单列前言 section 且不冒充章节 TOC。章节仍逐条进入最多 2,000 项 TOC，但相邻章节按 1 MiB 生成 XHTML 软上限合并，每章用受控 `chapter-N` fragment 定位；16 MiB 单 section 硬上限不变。不识别裸数字或固定段落数；无可信章节的大文本只在空行附近切成有界“正文片段”，不冒充语义章节。
- ReaderManifest **全局验证上限**保持 1,000 sections，backend 与 reader runtime 分别以具名 `MAX_MANIFEST_SECTIONS` 验证同一契约，不为跨 Rust / JavaScript 共享数字引入代码生成；EPUB `MAX_SECTIONS = 1_000` 与 CBZ `MAX_PAGES = 1_000` 的格式专属输入面不变。TOC 独立允许至多 2,000 项；Markdown 超过 1,000 个 H1 sections 稳定拒绝，TXT 超过 2,000 个语义章节稳定拒绝。
- Android 继续使用现有系统 picker 和 `PickerInput` cache bridge。直接路径使用文件扩展名；content URI 复用锁定 Tauri `2.11.5` 的 `app.path().file_name(content://...)`，由内置 PathPlugin 经 Android `ContentResolver` 取得文件名，再只保留 `.epub` / `.cbz` / `.md` / `.markdown` / `.txt` 允许列表中的后缀。不自建 Kotlin / plugin；缺失或未知后缀稳定拒绝，不用“Markdown 失败后试 TXT”或反向 fallback 猜格式。受控 display stem 只在内存中作为无元数据书籍的有界标题 fallback，不用于 cache 路径，也不进入日志或证据；Markdown 自身标题优先。
- 正向 Markdown 只使用版本控制中的 `README.md` 与 `docs/research/epub2-ncx-library-assessment.md`；正向 TXT 只使用用户放入 `fixtures/local` 的真实网文。不复制、截取、派生或提交本地书籍；最小合成输入仅用于 raw HTML、脚本 / 链接 / 图片越界、非法编码、超长行和数量 / 大小上限等信任边界，不冒充正向样书。
- Markdown 排版直接采用 Readest 已验证的最小规则：代码保留等宽字体并以 `pre-wrap` / `overflow-wrap` 换行，表格有可见边框，引用有内联缩进；多行代码块允许跨页，不作为不可分割原子元素。该样式固定写入 importer 生成的 XHTML，不复制 Readest UI 或 Markdown DOM 管线。
- Android edge-to-edge 复用 Readest 的 native inset 事实，但不复制其沉浸式导航栏策略：`MainActivity` 在 UI 线程观察 `systemBars | displayCutout`，bridge 只读缓存并控制阅读态状态栏显隐 / 图标明暗，Web 端只消费一套自有 safe-area 变量。左上章节标题在隐藏状态栏时仍位于 cutout 下方；工具打开时隐藏标题并让顶部栏完整避让状态栏，不做按设备参数微调。
- 日志复用现有 `log` + `tauri-plugin-log` 与 `atha::` target，只增加固定枚举 / 数字字段：`format`、`input_bytes`、TXT `encoding`、`sections`、各阶段 `*_ms`、`search_results`、`search_truncated`、`sections_scanned` 和稳定 stage / code。不记录标题、作者、正文、搜索词、路径、URI、URL、内容哈希或 detector 探测字节。

## Non-Goals

- 不建立多格式 factory、trait hierarchy、codec registry、第二套 reader / locator，不引入临时 EPUB 或 JavaScript Markdown DOM 管线；
- 第一片不支持 Markdown 活动链接、相对图片、网络资源、front matter 元数据或任意方言，不给单文件 Markdown 所在目录读权限；
- 不复制 Readest 的自写编码评分、宽松裸数字章节规则或固定段落切章，不在真实失败前增加手工编码选择；
- 不增加 TOC 虚拟化、并行 fetch 池、全文索引或第二套搜索引擎；分组后的搜索结果与注释筛选复用现有 TOC fragment，显示命中章节或诚实的章节范围；
- 不为 7.36 MiB 首个真实 TXT 预建 offset 索引、规范化正文临时文件或一遍复杂切分管线；只在 Android benchmark 证明三遍顺序读取是瓶颈时升级；
- 不把 x86_64 16 KiB AVD 的 PSS / P95 当作 ARM 真机、低端设备或发布包的性能承诺。

## Architecture Impact

present

- Design purpose: 在具体格式 importer 内把 Markdown 事件流和 TXT 有界解码流归一为现有 ReaderManifest / BookRoot，使书架、搜索、Locator、恢复、消息与 WebView2 安全边界不分叉。
- Drivers / quality scenarios: `A-TEXT-01`（高业务重要性 / 中技术风险，负责人：reader importer）；Android 用户经系统 picker 选择 Markdown / TXT 后，应得到可阅读的章节、目录、全书搜索和同一 Locator 恢复。`A-TEXT-SEC-01`（P0 内容安全，负责人：importer / BookRoot）；raw HTML、脚本、链接、图片、路径、无效编码和超限输入在发布书根前被转义或稳定拒绝，不产生网络 / 路径读取。`A-TEXT-PERF-01`（高技术风险，负责人：importer / Android gate）；一章一 section 与 256 KiB / 43 sections 两个候选均因首次真实 TXT 全文搜索超过 5 分钟而 no-go；修订方案保持 1,134 章 TOC，把正文合并为不超过 16 个约 1 MiB sections，再对导入、缓存打开、目录、全书搜索和强停恢复各运行 10 次，任一 OOM / ANR / renderer gone、功能失败或超过正式 gate timeout 即 no-go，否则以 median / nearest-rank P95 与固定阶段 PSS 进入本 change 验收。
- Modules / Interfaces / Seams / Adapters: 第一个 TDD public seam 是 `LocalLibrary::import`；只通过其返回的书籍身份与后续 `open_book` 可观测的已验证 `BookRoot` / `ReaderManifest` 断言行为，不锁定私有 parser helper。第二个 TDD public seam 是 Android 真实“系统 picker → 打开 → 搜索 / 定位 → 强停重启”链路。`reader::text` 是一个具体 adapter，内部只用固定格式枚举区分 Markdown / TXT 并共享发布事务；ReaderManifest、BookRoot、LibraryBook DTO、Locator、Search 和 MessageStore interface 不变。
- Candidate and tradeoffs: 采用 `pulldown-cmark 0.13.4`、`encoding_rs 0.8.35`、`chardetng 1.0.0` 与已有 `regex 1.13.1`；精确许可分别为 MIT、`(Apache-2.0 OR MIT) AND BSD-3-Clause`、Apache-2.0 OR MIT、MIT OR Apache-2.0，可与 Atha `AGPL-3.0-or-later` 组合，锁文件、crate LICENSE 与 `THIRD_PARTY_NOTICES.md` 已复核。拒绝 `comrak` 的完整 AST 功能面、自写 parser / decoder、通用格式工厂和临时 EPUB；TXT 保留三遍顺序本地读取。Readest 也逐章生成 XHTML，但其全文搜索已迁到独立 worker；Atha 不复制该子系统，只在现有 importer 合并物理 sections，保留章节 TOC / fragment 与现有 Locator。
- Evidence / ADR / review trigger: 一手研究与 Readest 固定基线见 `docs/research/markdown-txt-format-assessment.md`；首次 API 36 AVD 真实 TXT 运行在目录绑定后、全书搜索阶段超过 5 分钟，已停止且未启动余下 9 次。该证据触发本文件的已批准性能回退，不另建格式 factory 或搜索架构。若分组后 10 次 AVD 仍 no-go，再评审 Readest 式独立搜索 worker；若真实编码选择错误，再评审显式编码选择，不放宽猜测。仅在至少一台 ARM64 Android 真机用同一 release-like build 完成同口径基线后，才可宣称 Android 性能完成并冻结后续 P95 / 内存回归门槛；AVD 只证明目标端功能与同环境回归。

## Acceptance Criteria

- [x] 按照预先确认的两个 public seams 逐个执行 red → green：`LocalLibrary::import` → 已验证 BookRoot / ReaderManifest，以及 Android 真实 picker / open / search / restart；每次只增加足以通过当前行为的最小实现，不测试私有 helper。
- [x] 仓库现有 Markdown 文档经 `LocalLibrary::import` 得到正确的前言 / H1 sections、嵌套 TOC 和可读 XHTML；raw HTML 不执行，链接不导航，图片不读本机 / 网络，16 MiB 与 section / TOC 超限返回稳定错误。
- [x] `fixtures/local` 真实 TXT 由锁定 `chardetng` + `encoding_rs` 流式解码，保留 1,134 条章节 TOC、一个无 TOC 的前言，并按 1 MiB 软上限生成 2–16 个 XHTML sections；每个 TOC fragment 都可定位，Android gate 从运行结果记录实测 `sections` / `toc_items` 并断言该语义，不记录标题。编码标签由本地 gate 锁定；跨块多字节、CRLF / CR、BOM、非法序列、单个疑似标题、1,001–2,000 章与超限均通过信任边界检查；不提交或派生真实样本。
- [x] backend 和 reader runtime 都接受至多 1,000 个已验证 sections 与至多 2,000 个 TOC items；EPUB 仍在 1,000 sections、CBZ 仍在 1,000 pages 稳定拒绝，不出现第三个无名上限。
- [ ] Markdown 与 TXT 分别在固定 API 36 x86_64 16 KiB AVD 上从干净数据完成系统 picker、导入、打开、目录首 / 中 / 末定位、全书搜索、翻页、强停重启和 Locator 恢复，Picker cache 为空，无脚本执行、网络、panic、OOM、ANR 或 renderer gone。Windows 上的候选曾通过；搜索遥测、严格后缀和 fixture 清理修复后的最终候选改在 Linux 重跑。
- [ ] 真实 TXT 在固定 build / AVD 预热后对冷导入、缓存打开、首稳、目录首 / 中 / 末定位、翻页、强停恢复和全书搜索各取 10 个样本，记录 median / nearest-rank P95；在书架、导入完成、1,134 项 TOC 绑定后、首 / 中 / 末 section 与恢复后记录 app PSS，renderer 不可唯一归因时显式留空。Windows 上的修复前候选已有十样本；最终候选在 Linux 重跑前不继承该性能证据。
- [ ] 至少一台 ARM64 Android 真机使用同一 release-like build 重跑真实 TXT 功能、10 样本阶段耗时与 PSS 口径；在此之前只可声明 AVD 功能 / 同环境回归通过，不可声明 Android 性能完成。
- [ ] 导入、搜索、重启和失败日志只含固定 stage / code、格式、编码枚举、计数、耗时与布尔值；logcat 与全部 `Atha.log*` 不含标题、正文、搜索词、路径、URI、URL 或内容哈希。静态白名单已复核，最终 Linux AVD 仍需重跑双日志门。
- [ ] 锁文件与第三方 notices 复核上述精确版本和许可；Rust fmt / Clippy / tests、Svelte / Tauri check / build、AutoCorrect、required docs gate 与独立 Spec / Standards review 通过。用户已要求本 change 关闭后不再在 Windows 运行目标测试，后续 APK / 桌面门迁到 Linux；ARM64 真机仍由上一项单独跟踪。

## Files And Steps

1. 以 `LocalLibrary::import` 为唯一 backend public test seam，锁定 manifest 1,000 sections / 2,000 TOC items 与 EPUB / CBZ 1,000 专属上限，再在 backend 与 reader runtime 用同名具名契约实现 green。
2. 逐切片为 Markdown 安全事件变换、H1 sections / TOC 和原子书根发布建 red → green；正向验收只读仓库现有文档。
3. 逐切片为 TXT BOM / 严格 UTF-8 / detector、跨块解码、章节规则和有界写入建 red → green；章节目录写为当前分组 XHTML 的受控 fragment，当前 section 达到 1 MiB 软上限后只在下一章边界切分；实现仅保留“检测、解码计数、解码写入”三遍本地读取，正向编码、章节与规模只用 `fixtures/local` 真实样本验收。
4. 扩展现有 LocalLibrary / Tauri picker 的严格扩展分派；Android 用 Tauri 内置 `app.path().file_name` 取 content URI 后缀并校验允许列表，保持 LibraryBook DTO、open command 与 picker cache 边界不变。
5. 扩展现有 Windows / Android 正式 gate，完成 Markdown 与 TXT 真实链路、10 样本 AVD benchmark、PSS / 隐私日志和 1,000 sections / 2,000 TOC 上限验证；一章一 section 的首次 no-go 已记录，分组方案仍 no-go 时停止并评审独立搜索 worker。
6. 在 ARM64 真机用同一 release-like build 形成性能基线，再更新事实所有者、精确依赖 / 许可、证据和独立复审。

## Checks

- `cargo test --locked -p atha-backend` 中的 Markdown / TXT `LocalLibrary::import` 纵切、现有 EPUB / CBZ 回归、workspace fmt 与 Clippy `-D warnings`；
- `pnpm --dir reader/app check`、`pnpm --dir reader/app build` 与 reader manifest / 搜索相关检查；
- 现有 Windows Tauri / WebView2 正式入口分别只读仓库 Markdown 与本地 TXT，验证导入、目录、搜索、Locator 和无外联；
- `scripts/check-android-reader.ps1` 现有正式入口分别使用本地 Markdown / TXT，覆盖干净数据 picker / open / search / restart、10 样本耗时、PSS、健康和 `logcat` / `Atha.log*` 隐私扫描；
- 至少一台 ARM64 Android 真机的同 release-like build 功能 / 10 样本 / PSS 口径；
- 依赖元数据 / LICENSE / notices、AutoCorrect、`git diff --check`、required docs gate 与独立 Spec / Standards review。

## Rollback

删除两个具体 importer 与三个新直接依赖，恢复 picker 过滤 / 分派；EPUB / CBZ 专属上限、ReaderManifest schema、LibraryBook、Locator 和 MessageStore 无数据迁移。回滚不删除用户文本源或已导入缓存；候选期曾生成的超过 1,000 sections TXT 缓存可能无法打开，重新导入会生成分组书根且不改写源文件。

## Approval

用户于 2026-08-08 明确批准按路线图继续完成 Android 和 Readest 支持的非 PDF 格式，要求优先 Android、搜索成熟库、少造轮子、对性能敏感处 benchmark，并指定 Markdown 使用仓库现有文档、TXT 使用 `fixtures/local` 真实网文，不自造正向书籍。本 change 是该已批准路线图的当前最小纵向切片。

## Result

Markdown 与 TXT 已作为两个具体 adapter 接入现有 LocalLibrary、ReaderManifest / BookRoot、Locator、搜索、消息和 Android SAF 链路。Markdown 使用成熟事件 parser 做惰性安全投影，并按 Readest 的最小代码 / 表格 / 引用规则排版；TXT 使用锁定 decoder / detector 保留 1,134 条真实章节目录，同时合并为 12 个物理 sections，避免一章一次协议读取。相同字节的 Markdown / TXT 身份域隔离，EPUB / CBZ 既有身份不变。

Android edge-to-edge 同步修复为一套 native inset 事实：书架和工具面板避开系统栏，阅读态隐藏状态栏但左上章节标题仍在 cutout 下方；工具打开时标题隐藏、正文从工具栏下方开始。README 中的长 PowerShell 代码块已通过 Readest 同型换行规则和可跨页布局消除重叠，不增加 per-device 参数。

分页 `countCutRects()` 已从普通首开、字号、窗口重排和延迟资源错误链移出，只保留为 diagnostics / verify-sample / benchmark 指标；Android 正式入口把 ready 的 cut 数写入本地证据但不阻断阅读。普通图片在页内等比限幅，表格与代码进入受控页内滚动容器，长词按浏览器换行。Linux API 36 AVD 的 360×640 CSS viewport 探针证明原失败来自下一栏 `H2` 的 Range 矩形比安全的元素盒向上多出 4.75 CSS px，而非内容盒实际越界；这类浏览器测量差不再阻断整本阅读，安全、Locator 与持续不稳定布局错误保持不变。

## Review

- Blocking: 最终独立 Spec / Standards 复审均为 P0 0、P1 0。复审要求把 ready cut 明确保存为非阻断诊断、给真实超大媒体 / 结构内容提供局部恢复、固定研究中的 Atha 修复前基线，并保留作者图片尺寸；均已在当前候选关闭。
- Non-blocking: 修复前的 API 36 x86_64 16 KiB AVD 候选曾出现一次 Android 16 x86_64 / WebView 平台 renderer 进程崩溃；相同 APK 与链路立即重试后完成历史十样本。当前只记录为模拟器基础设施残余，不将其归因产品，也不当作最终候选或 ARM64 证据。
- Out-of-scope: 其他非 PDF 格式、CSS 编辑 / 模块 / 社区、阅读统计与离线词典继续按路线图后续分片。

## Evidence And Residual Risks

- 本地 importer 证据：Markdown / TXT、EPUB / CBZ 回归、身份域、编码 / 分块、章节规则、raw HTML / 链接 / 图片、大小 / 数量 / 源变化边界均由 `cargo test --locked -p atha-backend` 覆盖；正向 TXT ignored gate 只在本机显式 opt-in，得到 7,362,028 bytes、GBK、12 sections、1,134 TOC，不输出书名、路径、正文或哈希。
- Android Markdown 历史目标证据：复审修复前的 API 36 x86_64 16 KiB AVD 候选曾从干净数据对仓库 `README.md` 完成系统 picker、6 项目录首 / 中 / 末、完整搜索、翻页、强停恢复、Picker cache、PSS、健康和双日志隐私门；CDP 几何探针得到跨元素文字重叠 0。搜索遥测与 fixture 清理改变后不把这份 artifacts 当最终候选证据，Linux 重跑前仅用于说明链路曾打通。
- Android TXT 历史相对基线：同一修复前候选在 API 36 x86_64 16 KiB AVD 的 10 个成功样本中，冷导入 P50 / P95 为 3,142 / 3,216 ms，首稳 277.6 / 309.2 ms，全文搜索 741.5 / 896 ms，强停恢复 13,051.5 / 17,957 ms；首屏 app PSS 为 171,424.5 / 171,950 KiB，末屏为 171,577.5 / 184,101 KiB。renderer 进程不能唯一归因，因此显式留空；最终候选在 Linux 重跑前不继承这组数值。
- Linux Markdown 分页误报修复候选在 `Atha_API_36_16K`（API 36、x86_64、16 KiB、720×1280、320 dpi、WebView 133.0.6943.137）从干净数据通过系统 picker、导入、首稳 / ready、6 项目录首 / 中 / 末、全 section 搜索、翻页、强停恢复、Picker cache、应用健康及 `logcat` + `Atha.log*` 隐私门；书架 / 目录绑定 / 末页 / 恢复 app PSS 分别为 124,804 / 148,970 / 149,639 / 146,235 KiB，renderer 仍因无法唯一归因而留空。该次运行早于媒体限幅、结构滚动容器与 ready cut 证据字段，不能替代当前最终候选重跑。2 GiB AVD 首次运行曾由系统 low-memory killer 回收后台 app 与 WebView renderer；4 GiB 功能门随后稳定通过，该事件保留为 ARM64 真机低内存复核项，不归因于 `layout-cut` 修复，也不把 4 GiB 模拟器称为低端性能证据。
- `pulldown-cmark` 需要有界的完整 UTF-8 输入，因此 Markdown 保留 16 MiB 上限；TXT detector 对无 BOM 遗留编码只能 best effort，无 BOM UTF-16 不在承诺内。
- Tauri `2.11.5` 内置 PathPlugin 会经 Android `ContentResolver` 读 content URI 文件名；provider 未返回可用的允许列表后缀时会被拒绝，不自建 Kotlin / plugin，不用内容猜测换取更高召回率。
- 真实 TXT 的 7.36 MiB 规模保留三遍顺序读取；backend detect P50 / P95 为 2,017 / 2,082 ms，是当前冷导入主项，但 3.2 秒内稳定完成且不建立正文副本或 offset 索引。只有 ARM64 / 低端真机证明该路径不可接受时才升级。
- 1,134 项 TOC 仍会增加 Android 目录 DOM 与内存压力，但物理 sections 分组后不再把每章变成一次全书搜索协议请求；修复前 Windows / AVD 历史十样本未见功能失败、超时、OOM、ANR 或 renderer gone，不作最终候选证据。具体跨设备性能门槛仍只能在 ARM64 真机基线后冻结。
- 本地书籍的标题、正文、搜索词、路径与哈希不进入 Git、公共 CI、日志或分发包；证据只保留格式、输入字节数、编码枚举、section / 结果计数、布尔值和阶段耗时。
- 用户已决定本 change 关闭后的 APK 与桌面目标测试迁到 Linux GNOME 主机，Windows 只按明确要求使用；Linux 工具链与仓库副本在独立任务配置，不把尚未执行的 Linux gate 写成本 change 证据。

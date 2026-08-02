# Readest 阅读器能力与模块边界源码研究

## 结论

成熟阅读器不是一个“能显示书籍”的页面，而是一条依赖顺序明确的管线：书籍与位置契约先稳定，导入解析和受控资源把内容送入统一文档模型，渲染适配器只管理视图生命周期与事件，导航/进度、搜索、标注再依赖稳定 locator，TTS 与同步最后接入。Readest 已经出现这些边界，但仍有多个巨型编排器把本可独立的职责重新耦合到单文件中。

对 Atha 当前 reader-only 阶段，最有价值的不是复刻功能数量，而是先把现有巨型入口拆回清楚的职责，再按依赖逐项补齐能力：阅读会话与多章节输入 → locator 与导航 → 排版偏好 → 内容交互 → 进度与书签 → 搜索 → 标注与引用。完整格式导入、TTS 和同步都不应进入当前阅读器主线。

## 研究问题

1. 一个成熟阅读器从书籍打开到呈现、定位、搜索和标注，需要哪些独立职责？
2. 这些职责依赖什么稳定契约，应按什么顺序实现？
3. Readest 的实际源码已经暴露了哪些可复用的模块 seam，又有哪些巨型编排器应作为反例？
4. 在不采用 Readest 技术栈、视觉或交互设计的前提下，这些结构事实如何约束 Atha 的 reader-only 路线？

## 来源范围

- 一手来源仅为本地 `E:\Code\Atha\.tmp\readest-source`，Git 仓库为 `https://github.com/readest/readest.git`，本地快照分支为 `main`，HEAD 为 `2acb9fad0b578e590eec19b47f790b66461ac38f`，提交时间为 2026-07-31。
- 实际阅读范围集中在 `apps/readest-app/src/`、`apps/readest-app/src-tauri/src/`，并检查了 `apps/readest-calibre-plugin/`、`apps/readest.koplugin/` 的入口，以区分阅读内核与外部导入/同步集成。
- `packages/foliate-js` 在当前副本中只是未初始化 Gitlink，固定到 `df623dbe6610fd98a7c2d5d7a5c23bfcfc7d19f3`；目录为空。因此本文没有读过或评价其 renderer、parser、CFI、search、overlayer 内部实现。关于它的结论只来自 Readest 应用侧的 import、类型适配和调用点。来源：`.gitmodules`、`pnpm-workspace.yaml`、`apps/readest-app/package.json`。
- 其余多数 `packages/` 也是未初始化子模块；唯一有工作树内容的 `packages/swift-rs` 与 reader-only 职责无直接关系，没有据此扩展结论。
- 本研究是静态源码证据，没有构建、运行 Readest，也没有进行 UI、性能或真实同步验收。

文中源码路径均相对 `.tmp/readest-source`；同一段或表格行内，已给出完整目录后会简写相邻文件名。

## 架构观察

### 入口与实际打开链路

Readest 的 Web/App Router 入口最终都收敛到 `Reader`：`apps/readest-app/src/pages/reader/[ids].tsx::Page` 和 `apps/readest-app/src/app/reader/page.tsx::Page` 均挂载 `apps/readest-app/src/app/reader/components/Reader.tsx::Reader`。`Reader` 等待 library/settings 就绪并加载阅读所需外部资源，`ReaderContent` 再从路由 ids 创建每个 view key，调用 `readerStore.initViewState`，并负责关闭时保存、停止或移交 TTS、销毁 view。

实际依赖链如下：

1. `apps/readest-app/src/services/environment.ts::environmentConfig.getAppService` 选择已初始化的平台服务；`apps/readest-app/src/types/system.ts::AppService` 与 `FileSystem` 定义文件、数据库、导入、配置、导航缓存和内容读取端口。
2. `apps/readest-app/src/store/readerStore.ts::initViewState` 从 library 取 `Book`，经 `AppService.loadBookContent` 获取文件，再调用 `apps/readest-app/src/libs/document.ts::DocumentLoader.open` 得到统一 `BookDoc`。
3. 同一初始化过程加载 `BookConfig`，导入可用的第三方标注，加载或计算 `BookNav`，合并全局与单书 view settings，最后分别写入共享书籍数据和每视图会话状态。
4. `apps/readest-app/src/app/reader/components/FoliateViewer.tsx::openBook` 创建 `foliate-view`，调用 `view.open(bookDoc)`，设置 renderer 属性和样式，再以保存的 CFI 或起始 fraction 初始化位置。
5. `apps/readest-app/src/app/reader/hooks/useFoliateEvents.ts::useFoliateEvents` 把 `load`、`stabilized`、`relocate`、`navigate-start/end` 等 renderer 事件适配为应用事件；导航、进度、搜索和标注都从这个稳定事件/接口面继续工作。

这里最重要的结构事实是：页面组件不是解析器，也不是书籍存储；renderer 不直接拥有同步和持久化；所有后续能力都依赖 `BookDoc + locator + renderer events`。

### 稳定契约与分层

| 层 | 应拥有的职责 | Readest 源码证据 |
| --- | --- | --- |
| 领域契约 | 书籍身份、格式、元数据、section、TOC、locator、进度、标注和配置 schema | `apps/readest-app/src/types/book.ts::Book`、`BookNote`、`BookProgress`、`BookConfig`；`apps/readest-app/src/libs/document.ts::BookDoc`、`SectionItem`、`TOCItem` |
| 平台端口 | 文件系统、数据库、本地路径与平台能力；上层不按平台复制业务流程 | `apps/readest-app/src/types/system.ts::FileSystem`、`AppService`；`apps/readest-app/src/services/appService.ts::BaseAppService`；`NativeAppService`、`WebAppService`、`NodeAppService` |
| 导入与解析 | 格式探测、容器读取、元数据/封面、内容文档生成、损坏文件失败和可选原生快路径 | `apps/readest-app/src/libs/document.ts::DocumentLoader`；`apps/readest-app/src/services/bookService.ts::importBook`；`apps/readest-app/src-tauri/src/epub_parser.rs::parse_epub_full`、`parse_epub_metadata`；`apps/readest-app/src-tauri/src/mobi_parser.rs::parse_mobi_metadata` |
| 资源与内容变换 | 按 section 延迟加载资源，在渲染前执行确定的内容变换与安全处理 | `BookDoc.sections[].loadText/createDocument`；`apps/readest-app/src/services/transformService.ts::transformContent`；`TransformContext`、`availableTransformers` |
| 渲染适配 | 打开/关闭 view、应用布局参数、恢复位置、转发 renderer 生命周期事件，不拥有业务数据 | `apps/readest-app/src/types/view.ts::FoliateView`、`Renderer`、`wrappedFoliateView`；`FoliateViewer.tsx::openBook`；`useFoliateEvents` |
| 导航与位置 | TOC、section/fragment 位置、CFI 比较、缓存版本、位置恢复和外部 locator 转换 | `apps/readest-app/src/services/nav/index.ts::computeBookNav`、`hydrateBookNav`、`BOOK_NAV_VERSION`；`locations.ts::bakeLocationsAndCfis`；`utils/cfi.ts`；`utils/xcfi.ts::XCFI` |
| 应用用例 | 打开/关闭、翻页、保存位置、搜索会话、标注编辑、朗读会话 | `ReaderContent`、`usePagination`、`useProgressAutoSave`、`SearchBar.handleSearch`、`useAnnotationEditor`、`TTSSessionManager` |
| 持久化 | library、单书配置、导航/搜索缓存、数据库迁移；明确热状态与耐久状态 | `libraryService.ts`、`bookService.ts::loadBookConfig/saveBookConfig/loadBookNav/saveBookNav`、`persistence.ts::safeLoadJSON/safeSaveJSON`、`database/migrate.ts::migrate` |
| 同步 | wire schema、cursor、merge/tombstone、传输 provider 和编排相互分离 | `services/sync/file/wire.ts`、`merge.ts`、`provider.ts::FileSyncProvider`、`engine.ts::FileSyncEngine`；`replicaRegistry.ts::ReplicaAdapter`、`replicaSyncManager.ts::ReplicaSyncManager` |

依赖方向应保持单向：领域契约不依赖 UI、renderer 或同步；解析和存储实现契约；渲染适配依赖文档/资源契约；搜索、标注和 TTS 依赖 locator 与 render session；同步只复制已经定义清楚的耐久模型。

### 巨型编排器是反例，不是目标结构

| 文件 | 当前耦合 | 已存在、可作为拆分边界的 seam |
| --- | --- | --- |
| `apps/readest-app/src/app/reader/components/FoliateViewer.tsx`（1098 行） | renderer 创建、文档变换、样式、输入、翻页、进度、三类同步、媒体查看、辅助功能和平台差异集中在一个组件 | `DocumentLoader/BookDoc`、`FoliateView/Renderer`、`useFoliateEvents`、`usePagination`、`transformContent`、`useProgressAutoSave`、`useProgressSync`、`useFileSync` 已说明这些职责可以成为独立 session、pipeline 和 writer |
| `apps/readest-app/src/app/reader/components/annotator/Annotator.tsx`（1885 行） | 选择手势、overlay 绘制、标注 CRUD、字典/翻译/TTS 快捷动作、导入导出和多种同步集中 | `BookNote`、`useTextSelector`、`useAnnotationEditor`、`annotationIndex.ts`、`globalAnnotations.ts`、`services/annotation/`、`useNotesSync` 已给出 selection、domain、overlay、persistence、integration seam |
| `apps/readest-app/src/store/readerStore.ts`（577 行） | 打开书籍、解析、导航缓存、书籍共享数据、每视图状态和进度投影仍由同一 store 编排 | `bookDataStore.ts::BookData`、`readerProgressStore.ts`、`services/nav/` 已经按生命周期和更新频率拆出边界；剩余打开链路应是用例，不是 store 本身的责任 |
| `apps/readest-app/src/services/tts/TTSController.ts`（1507 行） | DOM/section 准备、客户端选择、播放状态机、跨章、highlight、timeline、缓存下载和 view attach 集中 | `TTSClient`、`TTSSessionManager`、`SectionTimeline`、`CachingProvider/TTSCacheStore`、`transformTTSSectionDocument` 已经提供 client、session、timeline、cache、document seam |

这四个文件的教训不是“再建一层万能 manager”，而是让现有 hook/service/interface 真正拥有职责：UI 只组合用例，render session 只管 renderer 生命周期，locator/progress writer 只管位置，annotation service 只管标注模型与持久化，TTS session 只协调播放器状态。拆分标准应是依赖和生命周期，而不是把大文件按行数机械切块。

## 能力清单

### 书籍、解析与资源

- 格式识别不能只信扩展名。`DocumentLoader.open` 检查空文件、ZIP EOCD、PDF magic、MOBI 探测，并将 TXT/Markdown、EPUB、PDF、MOBI/AZW、CBZ、FB2/FBZ 收敛为 `BookDoc`。Atha 当前只需一种真实来源，但需要保留“来源适配 → 统一内容文档”的方向。来源：`apps/readest-app/src/libs/document.ts::DocumentLoader`。
- 导入需要同时处理内容身份、作品/版本身份、元数据、封面、重复项和设备本地来源。Readest 以 `Book.hash` 标识内容，以 `metaHash` 聚合同作品版本，并明确 `filePath` 是设备本地字段。来源：`apps/readest-app/src/types/book.ts::Book`；`apps/readest-app/src/services/bookService.ts::importBook`、`mergeBooks`。
- 原生快路径只做机械工作，语义所有者保持唯一。Rust EPUB 代码负责 zip、partial MD5、封面和 OPF/nav/ncx 预取；JS bridge 明确保留 foliate-js 作为 OPF/MOBI 元数据语义所有者，并在失败时回退。来源：`apps/readest-app/src-tauri/src/epub_parser.rs::parse_epub_full`；`apps/readest-app/src/utils/tauriEpubBridge.ts::tryNativePrefetchEpub`；`apps/readest-app/src/utils/tauriMobiBridge.ts::tryNativeParseMobi`。
- 资源必须延迟、受控、可失败。`SectionItem` 暴露 `loadText/createDocument`，ZIP loader 只缓存打开热路径需要的文本，section 内容按需解压；`transformContent` 在送入 renderer 前串行应用明确的 transformer。来源：`apps/readest-app/src/libs/document.ts::SectionItem`、`DocumentLoader.makeZipLoader`；`apps/readest-app/src/services/transformService.ts::transformContent`。

### 渲染、导航与进度

- renderer 适配面至少需要 `open/init/close`、位置跳转、前后翻页、当前内容文档、CFI 生成、搜索、标注 overlay 和结构化事件。Readest 应用侧把这些收敛在 `FoliateView`/`Renderer` 类型，而不是让所有消费者碰内部 DOM。来源：`apps/readest-app/src/types/view.ts::FoliateView`、`Renderer`。
- 内容载入和视图稳定是不同阶段。`useFoliateEvents` 区分 `load`、`stabilized`、view `relocate`、renderer `relocate` 与导航开始/结束；`FoliateViewer.docLoadHandler` 处理 section 文档，而 `stabilizedHandler` 处理布局后工作。Atha 的 WebView2 协议也应区分“内容可用、布局稳定、位置变化”。来源：`useFoliateEvents.ts`；`FoliateViewer.tsx::docLoadHandler`、`stabilizedHandler`。
- locator 是搜索、标注、恢复和同步的共同基础。Readest 使用 CFI 作为 `BookConfig.location` 和 `BookNote.cfi`，为 TOC/fragment 计算 CFI 与 location，并以 `BOOK_NAV_VERSION` 使缓存随语义变化失效。来源：`types/book.ts::BookConfig`、`BookNote`；`services/nav/index.ts::computeBookNav`、`hydrateBookNav`；`services/nav/fragments.ts::buildSectionFragments`。
- 进度至少区分结构位置、显示页和总体 fraction。`BookProgress` 同时保存 CFI、section/page 信息、time 和 fraction；`readerStore.setProgress` 只让 primary view 写共享配置，并把高频进度放入独立 `readerProgressStore`。来源：`types/book.ts::BookProgress`；`store/readerStore.ts::setProgress`；`store/readerProgressStore.ts`。
- 热路径需要合并写入和最终 flush，而不是把每次 relocate 直接写全量状态。`FoliateViewer.progressRelocateHandler` 用一帧内最后事件更新进度，后台状态同步提交，卸载时 flush；`useProgressAutoSave` 跳过首次恢复和 deep-link preview，关闭时补写。来源：`FoliateViewer.tsx::progressRelocateHandler`；`hooks/useProgressAutoSave.ts`。

### 状态与存储

- 状态应按所有权与更新频率拆分：`libraryStore` 管书架索引，`bookDataStore` 管同一本书多 view 共享的 `Book/file/config/BookDoc`，`readerStore` 管每 view 会话，`readerProgressStore` 单独承载翻页热状态，`settingsStore` 管全局偏好。来源：`apps/readest-app/src/store/` 对应 store 与 `BookData`、`ViewState`。
- 耐久状态需要明确事实所有者。Readest 把小而关键的单书 `config.json` 立即写入，把全量 `library.json` 延迟合并保存；`safeSaveJSON` 先写 `.bak` 再写主文件，`saveLibraryBooks` 默认以磁盘内容为 floor 防止陈旧内存误删。来源：`store/bookDataStore.ts::saveConfig`；`services/persistence.ts::safeSaveJSON`；`services/libraryService.ts::saveLibraryBooks`。
- 派生数据必须带失效条件。导航缓存用 `BOOK_NAV_VERSION`；搜索缓存键包含书籍 hash、查询和 search config；TTS 缓存以 provider/voice/text 等内容寻址。来源：`services/nav/index.ts::BOOK_NAV_VERSION`；`components/sidebar/SearchBar.tsx::getSearchCacheKey`；`services/tts/providers/cache.ts::computeTTSCacheKey`。
- 数据库只用于需要查询、批量事务或大缓存的独立子域，并通过统一 migration 入口管理。来源：`services/database/migrate.ts::migrate`；`services/statistics/statisticsDb.ts::StatisticsDb`；`services/tts/providers/sqliteCacheStore.ts::SqliteTTSCacheStore`。这不是 Atha 当前立刻增加数据库表的理由。

### 搜索

- 搜索会话需要 query/config、范围、模式、渐进结果、进度、取消/替换、错误和 locator 结果；结果不应只是展示字符串。Readest 的 `BookSearchConfig` 支持 book/section scope 和多种模式，`FoliateView.search` 返回 async generator，匹配项携带 CFI。来源：`types/book.ts::BookSearchConfig`、`BookSearchMatch`、`BookSearchResult`；`types/view.ts::FoliateView.search`。
- 搜索的 UI 状态、执行和导航已是不同 seam：`SearchBar.handleSearch` 驱动 generator、缓存和取消；`useSearchNav` 把扁平结果与当前 CFI location 对齐并执行 `view.goTo(result.cfi)`。来源：`app/reader/components/sidebar/SearchBar.tsx`；`app/reader/hooks/useSearchNav.ts`。
- 搜索缓存属于可删除派生数据，键必须覆盖影响语义的配置，清除失败不能破坏书籍。来源：`SearchBar.tsx::getSearchCache/saveSearchCache/clearSearchCache`。

### 标注

- 最小标注记录需要稳定 id、类型、locator、原文、样式、笔记、创建/更新时间和删除 tombstone。Readest 的 `BookNote` 将 bookmark、annotation、excerpt 收敛为同一耐久模型，并用 `deletedAt` 支持同步删除。来源：`apps/readest-app/src/types/book.ts::BookNote`。
- selection、locator 生成、领域写入和 overlay 绘制是四个职责。`useTextSelector` 产生选择，view 负责 `getCFI` 与 `addAnnotation(..., remove)`，`useAnnotationEditor` 与 `bookDataStore.updateBooknotes/saveConfig` 更新耐久模型，`annotationIndex` 按 CFI spine prefix 为当前页筛选。来源：`apps/readest-app/src/app/reader/hooks/useTextSelector.ts`、`useAnnotationEditor.ts`；`apps/readest-app/src/app/reader/utils/annotationIndex.ts`；`apps/readest-app/src/types/view.ts::FoliateView`。
- 渲染中的 overlay 是耐久标注的投影，不是事实所有者。section `load` 时按 locator 重画，分页进度变化时只筛当前 section；全局重复标注另由 `globalAnnotations.ts` 生成 synthetic overlay key。来源：`Annotator.tsx::onLoad`；`annotationIndex.ts::buildAnnotationIndex/selectLocationAnnotations`；`globalAnnotations.ts::expandGlobalAnnotation`。
- 外部导入应通过 provider 适配后合并到统一模型，而不是让主流程识别每种外部格式。来源：`services/annotation/types.ts::AnnotationImportProvider`；`services/annotation/providers/foliate.ts::foliateProvider`；`providers/mrexpt.ts`。

### TTS 与同步（后置能力）

- TTS 依赖稳定的 section 文档、locator 和 renderer attach/detach。Readest 把 session 生命周期放在 `TTSSessionManager`，客户端能力放在 `TTSClient`，时间映射放在 `SectionTimeline`，缓存放在 `CachingProvider/TTSCacheStore`；`TTSController` 仍因同时承担 DOM、状态机、highlight、跨章和下载而过大。来源：`services/tts/TTSSessionManager.ts`、`TTSClient.ts`、`SectionTimeline.ts`、`providers/cache.ts`、`TTSController.ts`。
- 朗读所用文本必须经过与显示一致的确定性变换，否则 sentence/word offset 无法回到页面。来源：`services/tts/transformDoc.ts::transformTTSSectionDocument`；`TTSController` 中 `transformTTSSectionDocument` 与 highlight/timeline 调用。
- 同步不是“上传 JSON”一个步骤，而是耐久 schema、增量 cursor、冲突合并、删除 tombstone、设备本地字段排除、传输 provider 和编排。Readest 的文件同步将这些拆为 `wire.ts`、`merge.ts`、`provider.ts`、`engine.ts`；replica 同步再将 CRDT、adapter registry、pull/apply、publish 和加密 middleware 分开。来源：`services/sync/file/`；`services/sync/replicaRegistry.ts`、`replicaSyncManager.ts`、`replicaPullAndApply.ts`、`replicaPublish.ts`。
- 进度、书籍/配置/笔记、设置/字典等 replica、第三方文件后端和 KOReader 进度是不同同步通道；把它们塞进 renderer 会造成生命周期和冲突策略混乱。来源：`hooks/useProgressSync.ts`、`hooks/useFileSync.ts`、`hooks/useReplicaPull.ts`、`services/sync/KOSyncClient.ts`。

## 依赖顺序

| 顺序 | 先完成的契约或能力 | 为什么必须在后续能力之前 |
| --- | --- | --- |
| 1 | `ContentDocument/Section/ResourceRef` 与内容版本等最小领域契约 | 阅读器首先需要知道正在读取哪些有序内容；尚无消费者的标注、同步 schema 不应预建 |
| 2 | 一种受控内容输入、书根资源边界、内容验证与明确失败 | renderer 只能消费受控、规范化的内容；安全边界不能等 UI 完成后补；多格式工厂可以后置 |
| 3 | WebView2 render adapter/session：open、close、load、stable、relocate、navigate、resource request | 隔离具体浏览器承载，使上层功能依赖窄事件与 locator，而非 WebView DOM 细节 |
| 4 | `Locator/Range`、TOC/section 导航、位置恢复与进度写入 | 搜索和标注都需要可靠跳转、当前位置判断和关闭 flush |
| 5 | 排版偏好与内容交互 | 字体、边距、主题、选择、链接、脚注和媒体会改变布局或产生 range，必须先验证 locator 在真实阅读行为下稳定 |
| 6 | 搜索 session：范围、取消、渐进结果、locator 命中、可删缓存 | 只读能力，能先验证 locator、section 遍历与导航契约是否足够 |
| 7 | 标注：selection → locator → model → persistence → overlay projection | 写入用户数据，必须建立在已验证的 locator、恢复和持久化错误处理之上 |
| 8 | 一种真实封装格式的导入适配 | 只在阅读器输入契约稳定后接入，避免格式解析反向塑造 renderer |
| 9 | TTS | 依赖 section 文本、跨 section 导航、位置跟随、后台生命周期和媒体能力；不是阅读内核前置条件 |
| 10 | 同步 | 只能复制已经稳定且有版本/冲突/删除语义的耐久模型；过早同步会冻结错误 schema |

搜索先于标注还有一个验证价值：它以只读方式同时压测 section 遍历、文本提取、locator 生成、结果跳转和缓存失效，而不会先承担用户数据丢失风险。

## 对 Atha 的路线图启示

Atha 已明确 WebView2 是唯一阅读渲染技术，书内脚本禁用，外部网络资源默认拦截，并且当前尚无导入解析、locator 重锚定或产品书籍导入链路。来源：`docs/architecture/READER-CORE.md`、`docs/codebase/MAP.md`。因此 reader-only 阶段应沿现有技术决策补齐职责，而不是引入第二渲染引擎或复制 Readest 平台结构。

当前最先需要偿还的是结构债，而不是功能债：`reader/atha-reader-host/src/main.rs` 已有 664 行，同时承担参数、窗口、WebView、两个自定义协议、IPC、网络探针和 benchmark；`reader/atha-reader.js` 已有 536 行，同时承担内容安全校验、加载、公式适配、分页、交互、自检和 benchmark。继续把任何新功能放进这两个入口，都会重演 Readest 巨型编排器的问题。

建议把现有 M2 收敛为九个依次验收的 reader-only 切片；每个切片只增加一种长期能力，关闭并验证后再开始下一项：

1. **R0，整理现有切片。** 只拆现有职责，不加产品能力。Rust 入口只负责启动和组合，参数/窗口、受控协议、诊断与 benchmark 各自拥有实现；页面入口只组合内容加载、安全校验、分页和诊断。现有三样本、安全、几何与 benchmark 全部保持通过。
2. **R1，阅读会话与多章节输入。** 用受控书根和一份最小 manifest 表示内容版本、有序 section、资源和可选 TOC；先用现有解包样本，不做 EPUB importer 或多格式抽象。会话只提供 open、close、content loaded、layout stable 和明确错误。
3. **R2，Locator 与导航。** 定义 point/range locator 的序列化、解析、比较与回落；完成 section 前后跳转、TOC 跳转和跨重排位置恢复。页码只是显示结果，不能成为耐久坐标。
4. **R3，排版与阅读偏好。** 在固定设备像素页面契约上逐项加入字体、行距、边距、主题、书源样式与用户覆盖层；具体控件和视觉后续设计。每次重排都必须保持内容附近的位置并继续无裁切。
5. **R4，内容交互。** 依次处理键盘/鼠标/触摸翻页、文本选择与复制、书内/外链接、脚注、图片、表格、代码和公式交互；新控件同时满足键盘与读屏基本语义，不另建“最后补无障碍”的阶段。
6. **R5，进度、恢复与书签。** 分开会话热状态、每书耐久位置和用户偏好；进度写入合并并在关闭时 flush。书签先作为 locator 的首个耐久消费者，用它验证版本变化后的恢复。
7. **R6，书内搜索。** 先实现可取消的只读搜索，结果必须携带 locator 并能可靠跳转；只有真实大书证明需要时才加入 worker 或持久缓存。
8. **R7，标注与引用。** 第一版只做选择、range locator、原文快照、高亮/笔记、软删除、保存和重画，并产出未来消息链路可消费的 `SourceAnchor`；不同时实现 notebook、同步或外部导入。
9. **R8，阅读器门槛。** 用困难样本和大书复测安全失败、崩溃恢复、内存、冷开/热开、稳定页、翻页与重排；只优化测出的瓶颈。通过后才决定一种真实书籍来源的导入，然后再排书架、消息、TTS 或同步。

对应的防巨型文件 seam 应在实现前固定：

- 当前受控内容输入只产生统一内容文档；等第二种真实来源出现时才建立 `BookSource/Parser` 适配面；
- `ResourceResolver` 只执行书根与信任边界；
- `RenderSession` 只适配 WebView2 生命周期、命令与事件；
- `LocatorService` 只负责生成、比较、序列化和重锚；
- `NavigationProgressService` 只负责 TOC/跳转/恢复/进度；
- `SearchSession` 只产生带 locator 的结果；
- `AnnotationService` 拥有标注事实，overlay 只是 render projection；
- UI 组件只组合这些用例，不直接解析书籍、写文件或实现同步合并。

这些名字是职责边界，不要求预先建立一组空 interface。应在首个真实切片中只保留有两个以上消费者或需要隔离平台/信任边界的 seam。也不建议用硬行数上限机械拆文件：组合入口若开始实现业务规则，或加入一个功能需要同时修改三个不相干区域，就应先拆；行数只作为复核信号。

TTS 与同步明确延后：只有搜索和标注证明 section 文本、locator、后台恢复与耐久 schema 稳定后，才研究 TTS；只有本地标注/进度的版本、删除和冲突语义稳定后，才设计同步 wire。Atha 当前路线图也已把云同步暂缓，不应由本研究提前解锁。

## 明确不采纳项与不确定项

### 不采纳

- 不评价、不复制 Readest 的视觉层、工具栏、侧栏、弹窗、手势、分页动画、布局参数或交互信息架构。
- 不建议 Atha 采用 Readest 的 Next.js、React、Zustand、Tauri、Rust parser、foliate-js、Supabase、WebDAV provider 或任何其他技术栈；本文只提炼职责和依赖。
- 不采纳 Readest 可选执行书内 inline script 的路径。`FoliateViewer.evalInlineScripts` 在 `allowScript` 开启时执行 iframe 脚本，与 Atha“书内脚本禁用”的稳定信任边界冲突。
- 不把 Readest 的多格式、平行阅读、词典、翻译、AI、Word Lens、RSVP、TTS、多云同步或 KOReader/Calibre 集成列为 Atha reader-only 必做功能。
- 不复制 `FoliateViewer`、`Annotator`、`readerStore`、`TTSController` 这类巨型编排器，也不为避免它们而预建万能 manager、插件系统或空抽象。
- 不因 Readest 使用 JSON、SQLite 或多种缓存就提前固定 Atha 的数据库 schema；存储形式由 Atha 首个真实耐久用例决定。

### 不确定项

- `packages/foliate-js` 未初始化，因此 `foliate-view` 的分页、CFI、搜索、PDF、overlay 和 renderer 内部算法没有源码证据；本文只确认 Readest 应用侧所依赖的接口和事件。
- 没有运行 Readest，无法确认注释中描述的性能、平台回退、同步收敛或错误处理是否在真实环境成立。
- Readest 当前 `Book.hash` 是 partial MD5，`metaHash` 是元数据聚合键；这只能证明“内容身份与作品/版本身份需要区分”，不能证明其哈希算法或聚合规则适合 Atha。
- Readest 的 CFI/XPointer 兼容层证明 locator 需要版本、验证和互操作边界，但不能直接证明 Atha 的 locator schema；Atha 仍需以自己的书籍版本、引用回链和重锚验收定义。
- 本研究没有检查 Readest 历史提交、issue 或未初始化依赖，因此不把源码注释中的 issue 编号当成独立验证证据。

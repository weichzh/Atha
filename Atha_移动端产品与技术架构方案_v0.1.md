---
title: "Atha"
subtitle: "移动端产品与技术架构方案"
author: "架构提案 / 待 P0 实机验证"
date: "2026-08-01 · v0.1"
lang: zh-CN
---

\newpage

# 目录 {.unnumbered}

- 文档控制
- 执行摘要
- 产品定义
- 范围与非目标
- 核心体验设计
- 质量属性与工程原则
- 技术调研与选型结论
- 推荐总体架构
- 平台实现方案
- 阅读引擎与格式处理
- Telegram 式笔记系统
- 数据模型与存储
- MOBI/AZW 词典子系统
- 阅读统计
- 性能预算与 Benchmark 体系
- 日志、Trace 与生产可观测性
- 安全与隐私
- 测试策略
- 分阶段交付
- 团队工作流
- 风险与缓解
- 架构决策门禁
- 开放问题
- 附录 A–E
- 结论

\newpage

> **文档状态**：架构提案，供产品、客户端、基础设施、测试与商业负责人评审。本文中的性能数值均为首轮工程预算，不代表已经达成的实测结果；正式选型以 P0 实机基准结果和许可审查为准。

# 文档控制 {.unnumbered}

| 项目 | 内容 |
|---|---|
| 文档名称 | Atha 移动端产品与技术架构方案 |
| 版本 | v0.1 |
| 日期 | 2026-08-01 |
| 状态 | 架构提案 / 待 P0 验证 |
| 首发目标 | iOS、HarmonyOS；移动端优先 |
| 首发格式 | EPUB、TXT |
| 后续格式 | 无 DRM 的 MOBI、AZW、AZW3 |
| 明确排除 | PDF、OCR、桌面端、Web 端、书城、公共社区、DRM 破解 |
| 核心能力 | 阅读、富文本书摘与笔记、Telegram 式阅读对话、统计、可检索的阅读记忆 |
| 最高质量属性 | 性能确定性、数据可靠性、隐私、可导出性 |

# 执行摘要 {.unnumbered}

Atha 不是“大而全阅读器”，也不以替代微信读书、Kindle 或专业文献工具为目标。产品只解决一个清晰问题：用户阅读自己拥有的出版类电子书时，能够以极低摩擦记录反应，并在以后像查看聊天记录一样检索、回复和继续这段阅读关系。

产品的主循环为：

> 导入书籍 → 阅读 → 选择原文 → 形成引用消息 → 写下反应 → 继续阅读或与 AI 书友对话 → 搜索与回顾 → 形成长期阅读记录。

本方案给出以下首轮架构决策。

| 决策 | 结论 | 状态 |
|---|---|---|
| UI 跨平台策略 | 不强制以单一跨平台 UI 框架覆盖阅读器和长消息列表；采用原生平台壳与原生性能岛，跨端统一数据模型、接口和核心服务 | 推荐，待 P0 复核 |
| iOS 阅读后端 | Readium Swift Toolkit 3.8.x；SwiftUI 负责应用壳，阅读器与高负载列表允许使用 UIKit | 推荐 |
| HarmonyOS 阅读后端 | ArkUI/ArkTS + Reader Kit（API 16+），通过统一 `ReaderBackend` 接口隔离平台差异 | 推荐，必须真机验证 |
| 跨端核心 | 以稳定 C ABI 为边界；P0 同时验证 Rust 与 C++ 的构建、包体、启动、调试和崩溃符号化成本 | 暂定 Rust 优先，不提前锁死 |
| 聊天式笔记 | 原生虚拟化列表；数据层采用消息事件模型，不复制 Telegram 源码 | 推荐 |
| 数据库 | 应用随包固定版本 SQLite，WAL，单写队列，FTS5；不可变资产与可变用户数据分离 | 推荐 |
| MOBI/AZW | 非首发；HarmonyOS 可利用 Reader Kit，iOS 需单独适配。libmobi 仅作为技术验证候选，生产使用必须通过 LGPL 合规审查 | P2 决策 |
| MOBI/AZW 词典 | 首次导入离线构建 Atha 自有索引；启动和查询阶段不得重复解析整本词典；索引结构由基准决定 | 推荐 |
| 性能工程 | 基准、日志、Trace、语料、真机矩阵与 CI 门禁从 P0 建立，不能作为上线前补项 | 强制 |
| PDF | 不进入当前路线图 | 已决定 |

推荐的总体结构如下。

![Atha 推荐总体架构](/mnt/data/atha_architecture.png)

该结构的核心不是“少写两套 UI”，而是保证以下内容跨平台稳定：

1. `Work / Edition / Message / Locator` 等领域模型；
2. 用户笔记、书摘、统计、搜索与同步语义；
3. 字典导入与查询接口；
4. 性能事件和诊断数据的统一定义；
5. 对平台阅读引擎的可替换适配层。

平台 UI 和阅读渲染则允许使用最适合本平台的能力，以避免为了代码复用牺牲首屏、滚动、选择、排版与调试质量。

# 产品定义

## 产品定位

Atha 的产品定义为：

> **一个移动端优先、本地优先、以消息形式保存阅读反应的个人阅读系统。**

阅读器是内容入口；消息是最小记录单位；原文定位是消息可信度的基础；搜索与统计负责把一次性阅读转化为长期记忆；AI 书友是可选参与者，而不是产品成立的前提。

## 目标用户

首期目标用户不是“需要一个万能文件阅读器”的人，而是以下人群：

- 已使用微信读书、Kindle 或其他平台，但仍有大量本地 EPUB、TXT、MOBI、AZW/AZW3 书籍；
- 希望保留书摘、吐槽、疑问和思考，但不愿在每次记录时建立标题、双链和复杂分类；
- 希望阅读数据长期可搜索、可导出、可迁移；
- 对本地文件打开速度、翻页稳定性、词典查询速度有较高要求；
- 愿意把阅读记录视为“与书持续发生的对话”，而非一次性高亮集合。

## 产品原则

### 阅读器是基础设施，不是功能竞赛

首发阶段只实现影响日常阅读的能力：导入、书架、目录、搜索、进度、主题、字号、行距、边距、滚动或翻页、选择、书摘和回到原文。复杂排版参数不进入主界面；高级用户可通过版本化 CSS 主题模块扩展。

### 记录动作必须比整理动作更轻

选择一段文字后，用户可以直接形成引用卡片并发送一句话。标签、分类、标题、知识图谱均不得成为发送前置条件。

### 聊天是主要记录界面，不是唯一阅读界面

同一份消息数据至少提供三种投影：

- 当前书籍的对话流；
- 按章节聚合的书摘与笔记视图；
- 跨书籍的全局时间线与搜索结果。

### 原文位置优先于视觉气泡

任何引用消息必须可追溯到具体版本和具体位置。若原文件更新或重新导入，系统仍应利用章节、文本快照、前后文和内容哈希尝试重新锚定。

### 数据属于用户

基础导出、完整备份和删除不应成为高价订阅的人质功能。用户笔记、书摘和阅读统计必须能够导出为版本化 JSON；面向人类阅读的 Markdown/HTML 为第二导出层。

### AI 可拔除

不使用 AI 时，Atha 仍必须是一款完整、快速、可信的阅读记录产品。AI 不得污染定位、消息持久化、导出和离线阅读链路。

# 范围与非目标

## MVP 功能范围

| 领域 | MVP 必须交付 |
|---|---|
| 平台 | iOS、HarmonyOS 真机版本 |
| 格式 | EPUB、TXT |
| 书架 | 导入、封面、元数据、最近阅读、状态筛选 |
| 阅读 | 目录、全文搜索、位置恢复、翻页/滚动、基础排版设置、主题 |
| 选择 | 选择文本、复制、形成引用、精确跳回原文 |
| 笔记 | Telegram 式时间线、富文本消息、回复、编辑历史、删除、收藏 |
| 检索 | 书内消息搜索、全局消息与书摘搜索 |
| 统计 | 有效阅读时间、会话、进度、完成、笔记参与度、回顾次数 |
| 数据 | 本地 SQLite、备份、JSON 与 Markdown 导出 |
| 工程 | 固定语料、真机 Benchmark、统一 Trace、崩溃日志、隐私红线 |

## 后续范围

P2 进入无 DRM 的 MOBI、AZW、AZW3 普通书籍和词典。P3 再进入 AI 书友、跨设备同步和更高级的回顾与语言能力。

## 明确非目标

当前路线图明确不包含：

- PDF、OCR、页面手写标注；
- Windows、macOS、Web 和桌面端布局；
- 书城、版权内容分发和受 DRM 保护的 Kindle 内容；
- 公开书评社区、关注关系和粉丝系统；
- Obsidian 式双链、知识图谱、块引用网络；
- 多人协作与团队文献库；
- 插件市场；
- 自动生成整本书摘要作为核心卖点。

# 核心体验设计

## 核心用户流程

### 流程 A：记录即时反应

1. 用户在正文中长按选择文字；
2. Atha 在 120 ms 性能预算内显示上下文菜单；
3. 用户点击“引用”；
4. 引用卡片进入底部消息输入区，但不打断当前位置；
5. 用户输入文字并发送；
6. 消息立即本地可见，随后完成 SQLite 与 Outbox 提交；
7. 用户继续阅读，不被强制切换到笔记页。

### 流程 B：回看并继续一段阅读对话

1. 用户进入某本书的笔记页；
2. 系统恢复上次查看的消息锚点，而不是无条件跳到最底部；
3. 用户点击旧引用卡片；
4. 阅读器跳到原文并暂时高亮；
5. 用户返回笔记页，列表保持原滚动位置；
6. 用户可回复旧消息，形成跨时间的对话线程。

### 流程 C：与 AI 书友讨论

1. 用户选择原文或回复已有消息；
2. 用户选择一个书友人格，或在会话中直接发送；
3. 客户端只组装必要上下文：当前引用、前后文、用户消息、用户主动允许的相关历史；
4. AI 回复作为带来源元数据的消息写入；
5. 用户可以编辑、隐藏、删除或导出该回复；
6. 关闭 AI 后，用户原始记录保持完整。

AI 书友不进入 MVP 的关键路径，但消息模型必须从第一版预留 `author_type`、`model_id`、`context_ref` 和外部知识声明字段。

## Telegram 参考边界

Atha 参考 Telegram 的信息架构和交互原则，而不复制其代码或品牌资产。参考点包括：

- 消息为最小持久化单位；
- 稳定消息 ID 和事务式列表更新；
- 长时间线只渲染可见与邻近内容；
- 回复、编辑、删除、搜索与定位是消息的基本操作；
- 保持滚动锚点，不因加载旧消息或插入新消息产生跳动；
- 新消息仅在用户接近底部时自动吸附到底部；
- 输入区持有草稿、回复对象与附件状态。

Telegram iOS 源码采用自定义列表、事务队列、可见节点和受限预载等机制，但其开源许可要求不适合直接拷贝到闭源商业产品。Atha 只采用可独立实现的工程思想，并通过本项目基准证明自身实现。

# 质量属性与工程原则

## 优先级

质量属性按以下顺序处理：

1. 数据正确性与可恢复性；
2. 交互延迟与滚动稳定性；
3. 隐私和不泄露阅读内容；
4. 格式兼容性；
5. 跨端代码复用率；
6. 非核心功能数量。

当“复用率”与“性能确定性”冲突时，优先后者。跨平台的目标是统一语义和核心能力，而不是强求每个像素共享同一套实现。

## 性能工程原则

- 每个用户动作都必须有起点、终点和可测量定义；
- 区分冷、温、热三种缓存状态；
- 区分引擎耗时与用户可见耗时；
- 所有发布门禁必须在真实设备运行；
- 性能回归必须能定位到 commit、设备、OS、后端、语料和缓存状态；
- 不以平均值掩盖尾部延迟，主要观察 median、p90、p95 和变异度；
- 不允许用“感觉流畅”代替数据，也不允许用单次跑分宣布胜利；
- 不在生产日志中记录书名、路径、原文、笔记、词典查询词或 AI 提示词。

# 技术调研与选型结论

## HarmonyOS Reader Kit

现有公开资料表明，Reader Kit 在 HarmonyOS 5.0.4 / API 16 起提供 TXT、EPUB、MOBI、AZW、AZW3 的解析与阅读组件，并暴露 `bookParser`、`ReaderComponentController` 和 `ReadPageComponent` 等接口。它可以显著减少 HarmonyOS 端自建格式解析和排版的工作量，但存在以下架构约束：

- 仅支持特定 HarmonyOS 版本与设备类型；
- 模拟器不支持，必须使用真机测试；
- 当前服务范围有地区约束；
- 本地文件、排版和交互能力与 Reader Kit 组件耦合；
- 不应假定其 Locator、选择范围、词典和自定义 CSS 能力与 iOS 后端完全一致。

因此 Reader Kit 适合作为 HarmonyOS 的平台后端，而不是全项目共同核心。P0 必须验证：首帧、位置恢复、文本选择、引用定位、自定义样式、长章节、内存、异常文件和 Reader Kit 升级兼容性。

## iOS Readium Swift Toolkit

Readium Swift Toolkit 3.8.0 支持 iOS 15、Swift 6 和 Xcode 16.4，提供 EPUB 导航、位置模型、搜索、装饰和阅读偏好等能力。源码审阅得到的关键事实包括：

- EPUB 导航器提供前后位置预载，默认向前 2、向后 6；
- 3.8.0 将出版物资源从本地 HTTP 服务迁移到自定义 URL scheme handler；
- 资源服务实现了有界缓存、任务取消和 256 KiB 缓冲，并复用压缩资源以避免重复从起点解压；
- 工具包使用 BSD 3-Clause 许可，适合商业集成，但仍应固定版本并维护升级验证。

这说明成熟阅读器的性能策略不是“把整本书交给一个 WebView”，而是资源按需服务、有限预载、可取消任务和稳定位置模型。iOS 端应优先复用 Readium，并在其上构建 Atha 的统一定位和消息能力。

## 跨平台 UI 候选比较

下表为 P0 前的架构预评估，不是最终跑分。评分用于确定验证顺序，不能替代真机基准。

| 方案 | 平台与性能判断 | 主要风险 | 预结论 |
|---|---|---|---|
| 原生壳 + 稳定 C ABI 共享核心 | 两端平台能力和高负载页面最可控；核心语义可共享 | 两端 UI 成本；FFI 运维 | 推荐基线 |
| ArkUI-X + 原生性能岛 | HarmonyOS 适配强，普通 UI 复用潜力高；iOS 与高负载页面需实测 | iOS 生态、调试和控件差异 | P0 挑战者 |
| RNOH + 原生模块 | iOS 生态成熟，HarmonyOS 已有活跃实现；阅读器和列表仍需原生模块 | JS/原生边界、版本与插件质量 | RN 团队优先时验证 |
| Flutter Harmony 分支 + 原生插件 | iOS 成熟；HarmonyOS 不属于 Flutter 官方主支持矩阵 | 分支维护；两端仍需原生阅读后端 | 不作为基线 |
| Compose Multiplatform + Harmony 桥 | iOS/桌面方向成熟度提高；HarmonyOS 缺少官方一等支持 | 大量桥接和自维护 | 不作为首发方案 |

### 选型原则

Atha 的核心高负载界面恰好是跨平台框架最难做到“完全相同”的部分：电子书排版、文本选择、WebView/Reader Kit 生命周期、长消息列表、系统菜单、字体和无障碍。若为了共享 UI 而把这些能力封装成大量平台插件，最终会同时承担框架成本与两端原生成本。

因此本方案推荐：

- iOS 使用 SwiftUI 组织导航、设置、普通表单与书架；阅读器和聊天列表允许使用 UIKit；
- HarmonyOS 使用 ArkUI/ArkTS；阅读器接入 Reader Kit；
- 共享领域模型、数据库语义、搜索、字典、同步、统计和可观测性；
- ArkUI-X 与 RNOH 通过 P0 原型争取成为“普通产品 UI”的共享层，但无权绕过性能门禁。

## 跨端核心语言

Rust 对 iOS 和 OpenHarmony 均可形成原生库，但 OpenHarmony 工具链和 SDK 集成仍需额外封装。C++ 的平台工具链更传统，调试与第三方库集成成本更可预测。两者均可通过稳定 C ABI 暴露能力。

本方案不在纸面上把 Rust 写成不可撤销结论。P0 同时验证以下指标：

- iOS XCFramework 与 Harmony HAR/HSP 或 native library 的构建稳定性；
- Debug/Release 增量编译时间；
- 首次加载对冷启动和包体的影响；
- 1 次、100 次、10,000 次 FFI 调用的固定成本；
- 字符串、大块字节、批量消息和错误对象的所有权模型；
- 崩溃堆栈符号化、日志关联、内存泄漏检测；
- CI 缓存、交叉编译、依赖供应链和升级流程。

通过门禁后优先采用 Rust；任一平台的符号化、包体、构建或人才风险不可接受时，在相同 C ABI 后切换 C++，避免上层重写。

# 推荐总体架构

## 分层

Atha 分为五层。

### 产品界面层

负责书架、阅读、聊天、统计、设置和导入流程。平台可使用本地设计系统，但行为语义必须一致。

### 阅读后端层

以 `ReaderBackend` 隔离 Readium、Reader Kit 和未来其他实现。它负责打开出版物、显示、定位、选择、搜索、跳转和偏好应用，不直接持久化用户笔记。

### 可移植核心层

负责领域模型、消息、Locator 归一化、富文本 AST、数据库访问、全文搜索、字典、统计、导入任务、Outbox 和统一性能事件。

### 平台适配层

负责文件选择器、安全存储、系统分享、字体、网络、生命周期、后台任务、系统性能 API 和崩溃导出。

### 数据与同步层

本地 SQLite 和文件资产为事实源；云同步是可选扩展。用户即使不登录也可以使用核心阅读和笔记能力。

## ReaderBackend 接口

平台阅读器不得把平台私有对象扩散到业务层。建议接口如下：

```swift
protocol ReaderBackend {
    var capabilities: ReaderCapabilities { get }
    func open(_ source: PublicationSource,
              options: OpenOptions) async throws -> PublicationSession
}

protocol PublicationSession: AnyObject {
    func metadata() async throws -> PublicationMetadata
    func tableOfContents() async throws -> [TocNode]
    func currentLocator() async -> CanonicalLocator?
    func go(to locator: CanonicalLocator) async throws
    func apply(_ preferences: ReaderPreferences) async throws
    func search(_ query: String,
                options: SearchOptions) -> AsyncThrowingStream<SearchHit, Error>
    func selectedText() async -> TextSelection?
    func clearSelection() async
    func close() async
}
```

`ReaderCapabilities` 必须显式描述：

- 支持的格式；
- 翻页与滚动；
- 垂直书写、RTL、ruby、脚注；
- 自定义 CSS 能力；
- 文本选择和范围定位；
- 内置搜索；
- 自带词典或外部字典挂接能力；
- 可提供的性能指标。

业务层依据能力降级，不通过平台判断散落 `if iOS` / `if HarmonyOS`。

## Canonical Locator

平台 Locator 必须映射到 Atha 的规范定位结构：

```json
{
  "edition_id": "ed_...",
  "href": "Text/chapter03.xhtml",
  "type": "application/xhtml+xml",
  "locations": {
    "progression": 0.3821,
    "position": 1287,
    "fragment": "epubcfi(...)"
  },
  "text": {
    "before": "……",
    "highlight": "被引用的原文",
    "after": "……"
  },
  "backend": {
    "kind": "readium",
    "payload_version": 1,
    "payload": {}
  },
  "content_hash": "sha256:..."
}
```

规范定位用于同步、导出和重新锚定；`backend.payload` 只用于当前平台快速恢复。两者同时保存可以避免为了跨端抽象而丢失平台精度。

# 平台实现方案

## iOS

### 技术栈

- SwiftUI：应用壳、书架、设置、统计和普通页面；
- UIKit：Readium 导航器承载、聊天时间线与需要精确测量的复杂列表；
- Readium Swift Toolkit 3.8.x：EPUB 解析、导航、搜索、偏好与 Locator；
- 系统文件导入、Keychain、BackgroundTasks、MetricKit、OSLog/OSSignposter；
- 固定版本 SQLite，而不是完全依赖系统自带版本。

### 阅读性能策略

- 首次打开只完成识别、必要元数据和首个可读资源；
- 后台生成位置列表、全文索引和封面缓存；
- 使用 Readium 有界资源缓存和前后有限预载，不预解压整本 EPUB；
- 主题与字号变化采用锚点恢复，防止排版后位置漂移；
- 资源任务可取消，快速翻页时停止已经无用的图片和章节任务；
- 对超大单章节建立分段监测，必要时注入安全切分或降级到滚动模式；
- 阅读器进程内存告警时，优先清理远端章节、图片解码缓存和搜索临时对象。

## HarmonyOS

### 技术栈

- ArkUI / ArkTS：应用壳和产品 UI；
- Reader Kit：API 16+ 设备上的 EPUB、TXT，P2 扩展 MOBI/AZW/AZW3；
- ArkData/SQLite 或通过共享核心统一访问；
- HiLog、HiTrace、PerformanceAnalysisKit、Hypium/DevEco Testing；
- 可选 APMS 作为生产质量观察出口。

### 必须验证的 Reader Kit 事项

| 编号 | 验证项 | 通过条件 |
|---|---|---|
| H-R01 | 真机可用范围 | 目标发布区域和设备均可安装、初始化、打开测试书 |
| H-R02 | 首帧 | 满足本方案 `TTFR_BOOK` 预算 |
| H-R03 | 文本选择 | 返回稳定文本范围，可创建引用消息 |
| H-R04 | 位置恢复 | 字体、主题、旋转后仍能恢复到规范锚点 |
| H-R05 | CSS 与排版 | 支持产品所需最小主题能力；不足处有可接受降级 |
| H-R06 | 长章节 | 大型 XHTML 不出现持续掉帧、OOM 或不可取消任务 |
| H-R07 | 异常文件 | 损坏 ZIP、非法资源、超大图片能安全失败 |
| H-R08 | 升级 | Reader Kit 版本更新有兼容测试和回滚策略 |

若 H-R03 或 H-R04 不通过，Reader Kit 不能直接承担 Atha 的核心引用能力；此时需验证 ArkWeb + 自有统一出版物模型的后备路径。该后备路径不是默认实现，只有证据触发时才进入。

# 阅读引擎与格式处理

## 统一出版物模型

无论后端为何，业务层只接触以下概念：

- `PublicationMetadata`：题名、作者、语言、封面、标识符；
- `ReadingOrderItem`：阅读顺序资源；
- `TocNode`：层级目录；
- `PublicationResource`：HTML、CSS、图片、字体等；
- `CanonicalLocator`：位置；
- `TextSelection`：选中文本和范围；
- `SearchHit`：搜索结果和位置；
- `ReaderPreferences`：字体、字号、行距、边距、主题、流模式。

## EPUB

EPUB 是 ZIP 容器中的 HTML/CSS/资源集合。浏览器或 Reader Kit 负责排版，不意味着无需解析包结构。正确流程为：

1. 验证 ZIP、路径和资源上限；
2. 读取 `container.xml` 和 package document；
3. 解析 manifest、spine、导航与元数据；
4. 将资源以受控来源提供给渲染器；
5. 禁止默认脚本和任意外部网络；
6. 按需加载当前和邻近资源；
7. 记录 Locator、选择范围和内容快照。

## TXT

TXT 需要单独处理：

- 编码探测与用户覆盖；
- CRLF/LF/CR 标准化；
- 超大文件的流式分段；
- 章节启发式识别只能作为可撤销辅助，不改变原文件；
- 位置采用字节偏移、规范化字符偏移和上下文三重信息；
- 全文索引后台构建，不阻塞首次阅读。

## MOBI、AZW、AZW3

这些格式进入 P2，且仅支持无 DRM 文件。实现原则：

- HarmonyOS 首先验证 Reader Kit 的实际兼容性与性能；
- iOS 不把 JavaScript 解析器直接作为生产结论；
- libmobi 可用于内部兼容性验证，但其 LGPL v3-or-later 许可和 iOS 分发方式必须由法律与发布负责人审查；
- foliate-js 可作为 MIT 许可的参考实现和对照组，但其自身文档声明 API 不稳定，且 KF8 的 HUFF/CDIC 解压可能较慢，不宜未经基准直接嵌入；
- 最终生产适配器需固定版本、维护格式语料、对损坏文件安全失败，并输出统一出版物模型。

# Telegram 式笔记系统

## 信息模型

“聊天”不是把笔记存成一段 HTML，而是把阅读反应建模为事件。

| 消息类型 | 说明 |
|---|---|
| `quote` | 引用原文，必须关联 `SourceAnchor` |
| `user_text` | 用户富文本消息 |
| `assistant` | AI 书友回复，记录模型、上下文和外部知识声明 |
| `reading_event` | 可选的阅读里程碑，例如完成章节，不默认占据主时间线 |
| `system` | 导入、重锚定、冲突等系统事件 |

消息支持：回复、编辑版本、删除标记、收藏、标签、附件和跳转。线程关系通过 `reply_to_message_id` 表达，不做任意图结构。

## 长列表实现

长时间线必须采用原生虚拟化和事务更新。

### 必须具备的列表行为

- 以稳定 ID 计算插入、更新、删除；
- 只创建可见和邻近消息的视图；
- 文本排版和富文本解析尽量离开主线程；
- 对图片、引用预览和 AI 附件使用可取消预取；
- 向上加载旧消息时记录顶部锚点和像素偏移，更新后恢复；
- 用户不在底部时，新消息不强制改变位置；
- 快速滚动时允许使用轻量占位，但不能让已经可见的正文突然变空白；
- 每类消息有可缓存的测量结果，主题和宽度变化后按版本失效；
- 一次数据库查询按页返回，禁止把 100,000 条消息装入内存。

### 平台建议

- iOS：优先 `UICollectionView`、diffable/自有事务数据源和异步测量；只有在基准不达标时才实现更底层的节点系统；
- HarmonyOS：优先 `List`/`LazyForEach` 与稳定 key，结合实机测量决定是否需要自定义节点缓存；
- 两端共享事务语义和分页协议，不共享视图对象。

## 富文本

富文本内部不能只保存 HTML。推荐使用版本化 JSON AST：

```json
{
  "schema": "atha.richtext",
  "version": 1,
  "content": [
    {
      "type": "paragraph",
      "children": [
        {"type": "text", "text": "作者这里的论证", "marks": ["bold"]},
        {"type": "text", "text": "并不充分。"}
      ]
    }
  ]
}
```

同时保存：

- `plain_text`：FTS 与无障碍；
- `render_cache`：平台可失效缓存，不作为事实源；
- `schema_version`：迁移；
- `content_hash`：去重与诊断。

MVP 支持段落、加粗、斜体、删除线、列表、链接、行内代码和图片附件。表格、任意 HTML、嵌入脚本和复杂块组件延后。

# 数据模型与存储

## Work 与 Edition

同一作品可能有不同译本、排版和文件。必须区分：

- `Work`：抽象作品；
- `Edition`：具体版本与导入文件；
- `PublicationAsset`：原文件、封面、缓存和索引。

消息和定位首先关联 `Edition`；用户可以显式把多个 Edition 归并到同一个 Work。未经用户确认，不自动把相似书名视为同一作品。

## 核心实体

| 实体 | 责任 |
|---|---|
| Work | 作品级信息 |
| Edition | 具体文件、格式、指纹、解析后端 |
| PublicationAsset | 原文件、封面、资源缓存、索引版本 |
| Conversation | 书籍对话或全局对话 |
| Message | 消息头、作者、类型、当前内容 |
| MessageRevision | 编辑历史 |
| MessageAttachment | 图片与结构化附件 |
| SourceAnchor | 原文定位和文本快照 |
| ReadingSession | 有效阅读会话 |
| ReadingEvent | 位置、前后台、交互和完成事件 |
| Dictionary | 词典元数据和索引状态 |
| DictionaryHeadword | 规范化词头与条目引用 |
| DictionaryEntryBlock | 压缩后的释义块 |
| ImportJob | 可恢复导入任务 |
| OutboxEvent | 待同步事件 |

## SQLite 策略

- 应用随包固定 SQLite 版本，避免不同 OS 版本行为不一致；
- 版本必须包含 2026 年已修复的 WAL-reset 相关问题；
- 开启 WAL，但仍需要正确的 checkpoint、关闭和故障恢复测试；
- 单写队列，读连接池；
- 高频写入使用短事务和批处理；
- 用户可变数据与词典/全文不可变索引可使用不同数据库文件；
- FTS5 用于消息、书摘和书籍正文；
- `WITHOUT ROWID`、mmap 和自定义页大小只在基准证明收益后启用；
- 每次迁移必须可中断、可恢复、可回滚到备份；
- 所有数据库文件均有 schema 版本、应用版本和完整性检查记录。

## 同步模型

MVP 不使用 CRDT。理由是：消息主要为追加事件，复杂协同不是当前目标，过早引入 CRDT 会增加存储、迁移和调试成本。

推荐模型：

1. 本地事务写入事实表；
2. 同一事务写入 `OutboxEvent`；
3. 同步服务按事件 ID 幂等上传；
4. 服务端返回版本和确认；
5. 编辑冲突保留双方 revision，由客户端提示用户；
6. 删除采用墓碑，等待所有设备确认后再清理。

原始书籍文件默认不上传。云端优先同步笔记、定位、统计和元数据；用户主动开启时才同步书籍资产。

# MOBI/AZW 词典子系统

## 目标

词典功能的体验目标不是“能够查到”，而是：

- App 启动不因安装大型词典而明显变慢；
- 选择单词后迅速显示首条释义；
- 查询不重复解析整个 MOBI/AZW 文件；
- 支持词形变化、前缀建议和多个词典优先级；
- 词典文件损坏或索引中断不影响阅读；
- 索引可版本化重建，原文件保持不变。

## 格式难点

MOBI/KF8 词典并非简单文本。现有实现显示其可能涉及 INDX、TAGX、ORDT、词头 `orth`、变形 `infl`、条目偏移、PalmDOC 或 HUFF/CDIC 压缩和资源重建。因此不能在没有语料与基准的情况下提前断言“Trie 一定最快”或“SQLite 一定足够”。

## 导入流水线

1. 计算文件指纹、格式和解析器版本；
2. 创建可恢复 `ImportJob`；
3. 解析元数据、语言和词典类型；
4. 流式遍历词头、变形和条目位置；
5. 进行 Unicode NFKC、大小写折叠和语言相关规范化；
6. 写入临时索引和条目块；
7. 校验词头数量、偏移、块哈希和随机抽样查询；
8. 原子重命名为正式索引；
9. 更新字典状态并清理旧版本。

导入任务必须可暂停、取消和重启。索引期间阅读器仍可使用；只有该词典查询不可用或显示“正在建立索引”。

## 查询流水线

1. 读取用户选择文本；
2. 语言识别与 token 边界确认；
3. NFKC、casefold 和标点清理；
4. 精确词头查询；
5. 词形/变体映射；
6. 前缀建议；
7. 可选模糊查询；
8. 只读取命中条目所在块；
9. 清洗释义 HTML；
10. 渲染首条结果，其他词典并行补充。

## 索引候选

P0/P2 至少比较三种实现：

| 候选 | 优点 | 风险 | 适用判断 |
|---|---|---|---|
| SQLite B-tree / `WITHOUT ROWID` | 实现和迁移简单；事务可靠；便于诊断 | 前缀与超大词头集合可能有额外页访问 | 默认基线 |
| mmap 排序不可变表 + 二分 | 冷读路径清晰；内存可控；格式简单 | 多语言规范化、前缀和升级需自建 | 精确查询占主导时 |
| FST + 压缩条目块 | 词头和前缀紧凑；适合大词典 | 构建复杂；模糊/变形和调试成本高 | 大词典基准证明收益时 |

最终结构由以下指标决定：导入时间、索引大小、冷查询 p95、温查询 p95、热查询 p95、增量内存、随机 I/O、崩溃恢复和实现维护成本。

## 字典性能预算

| 指标 | 首轮预算 | 说明 |
|---|---:|---|
| `T_DICT_LOOKUP_ENGINE` 温查询 p95 | ≤ 20 ms | 规范化完成到释义模型返回 |
| `T_DICT_FIRST_RESULT` 温查询 p95 | ≤ 80 ms | 用户选择完成到首条释义可见 |
| 冷首查 p95 | ≤ 150 ms | 进程存活但索引页未热 |
| 精确查询额外内存 | ≤ 20 MB | 不包含 UI 固有开销 |
| 启动阶段词典解析 | 0 次 | 只打开索引元数据，不重建 |
| 索引失败恢复 | 自动 | 临时文件不得替换可用旧索引 |

上述数值将在 P0/P2 设备矩阵上校准。若某一语言的形态分析需要更长链路，应把“首条精确结果”和“补充变形结果”分阶段显示，而不是阻塞所有结果。

# 阅读统计

## 统计目的

统计不是为了制造连续打卡压力，而是回答三个问题：

1. 我实际把时间花在了什么书和章节上？
2. 阅读产生了哪些可复用的记录？
3. 哪些内容在读完后仍被回看和继续思考？

## 指标

### 阅读进度

- 当前稳定位置和完成比例；
- 开始、完成、搁置和重新开始时间；
- 本周、本月完成书籍；
- 预计完成时间，仅作为估算并标明误差。

### 阅读会话

- 有效阅读时长；
- 会话数量、平均值和中位数；
- 时间段分布；
- 前后台切换和闲置时间；
- 章节停留与回退。

有效阅读时间不能等于页面打开时间。建议规则：应用处于前台、阅读器可见、未超过闲置阈值，并在会话中检测位置、触摸、选择或翻页活动。阈值应可配置并纳入统计版本。

### 阅读参与度

- 每万稳定位置的引用和笔记数量；
- 笔记长度、回复深度和章节热力图；
- 引用跳回原文次数；
- 旧消息再次打开和继续回复次数；
- 导出与分享次数。

统计必须允许用户关闭；关闭后不再产生细粒度事件，只保留必要的阅读位置。

# 性能预算与 Benchmark 体系

## 指标定义

| 指标 | 起点 | 终点 |
|---|---|---|
| `TTI_APP` | 进程启动 | 书架首个可交互帧 |
| `TTFR_BOOK` | 点击书籍或调用打开 | 正文可读且导航状态可用 |
| `T_RESTORE` | 开始打开历史书籍 | 上次 Locator 已稳定可见 |
| `T_SELECT` | 用户结束选择手势 | 上下文菜单可响应 |
| `T_NOTE_COMMIT` | 点击发送 | 消息可见且 SQLite/Outbox 事务提交 |
| `T_CHAT_PREPEND` | 请求上一页消息 | 旧消息插入且原锚点稳定 |
| `T_DICT_LOOKUP_ENGINE` | 规范化查询提交 | 释义模型返回 |
| `T_DICT_FIRST_RESULT` | 选择单词完成 | 首条释义可见 |

## 首轮性能预算

| 场景 | 参考设备 p95 | 最低设备 p95 | 备注 |
|---|---:|---:|---|
| 冷启动 `TTI_APP` | ≤ 1.2 s | ≤ 1.8 s | 不等待全文索引 |
| 温恢复 | ≤ 350 ms | ≤ 500 ms | 从后台返回 |
| 已缓存书籍 `TTFR_BOOK` | ≤ 500 ms | ≤ 800 ms | 首个可读资源 |
| 未导入书籍元数据卡片 | ≤ 250 ms | ≤ 400 ms | 深度解析后台继续 |
| 60 Hz 翻页输入到呈现 | ≤ 50 ms | ≤ 66 ms | p95；持续卡顿率另算 |
| 连续滚动丢帧率 | < 1% | < 2% | 固定滚动脚本 |
| `T_SELECT` | ≤ 120 ms | ≤ 180 ms | 含菜单显示 |
| `T_NOTE_COMMIT` | ≤ 50 ms | ≤ 80 ms | 本地事务 |
| 首屏 50 条消息 | ≤ 100 ms | ≤ 160 ms | 数据已在本地 |
| 向上插入 50 条 | ≤ 150 ms | ≤ 220 ms | 锚点位移 ≤ 1 px |
| 普通书工作集 | ≤ 150 MB | ≤ 200 MB | p95 高水位 |
| 压力书工作集 | ≤ 250 MB | ≤ 320 MB | 不得 OOM |

这些预算是工程护栏，不是市场承诺。P0 应以设备分层重新校准，但任何放宽都必须记录原因和用户影响。

## Benchmark 分层

### Micro Benchmark

- ZIP central directory 与资源随机访问；
- EPUB manifest、spine、目录解析；
- TXT 编码和分段；
- MOBI PalmDOC/HUFF/CDIC 解压；
- INDX/TAGX/ORDT 解析；
- 字典规范化、索引构建和查询；
- Locator 序列化与重新锚定；
- 富文本 AST 解析、纯文本投影和测量缓存；
- SQLite 批量写、FTS 查询和迁移。

### Component Benchmark

- 导入书籍；
- 首次打开、再次打开和位置恢复；
- 翻页、连续滚动、主题和字号变更；
- 选择、引用、发送和跳回原文；
- 10,000 与 100,000 条消息的首次加载、向上分页和搜索；
- 词典导入、冷查、温查和热查。

### End-to-End Benchmark

- 冷启动 → 进入书架；
- 点击最近阅读 → 首个可读帧；
- 选择 → 引用 → 发送 → 继续阅读；
- 从搜索结果 → 消息 → 原文 → 返回；
- 安装大型词典 → 后台索引 → 查询；
- 应用升级 → 数据迁移 → 恢复阅读。

### Soak Test

- 30/60 分钟连续阅读；
- 高频翻页与主题切换；
- 1,000 次打开关闭；
- 100,000 消息滚动和搜索；
- 词典连续 10,000 次查询；
- 内存泄漏、电量、温控、后台恢复和崩溃恢复。

## 固定语料

语料必须可公开、可生成、可复现并带哈希。

| 类别 | 语料要求 |
|---|---|
| EPUB | 极小、普通、大型、图片密集、超大单章节、ruby、竖排、RTL、复杂脚注、异常 CSS、损坏 ZIP |
| TXT | UTF-8、UTF-16、GB18030 候选、不同换行、超大文件、无章节 |
| MOBI/KF8 | MOBI7、KF8、组合文件、不同压缩、资源密集、损坏索引 |
| 词典 | 5 万、20 万、50 万词头；含变形、重音符号、CJK、多释义 |
| 消息 | 100、1,000、10,000、100,000；短文本、长文本、引用、图片、编辑历史 |

CI 不使用用户受版权保护的书籍。内部兼容性语料若无法公开，必须记录来源权限、哈希和访问控制。

## 设备矩阵

- iOS 最低支持设备；
- iOS 近期 60 Hz 设备；
- iOS 近期 120 Hz 设备；
- HarmonyOS API 16 最低目标设备；
- HarmonyOS 中端设备；
- HarmonyOS 旗舰设备。

测试固定：OS 版本、构建类型、亮度、电量范围、低电量模式、网络、后台应用、温控状态和缓存状态。Reader Kit 不支持模拟器，因此 HarmonyOS 发布门禁不能以模拟器结果替代。

## CI 门禁

![Atha 性能门禁流水线](/mnt/data/atha_perf_pipeline.png)

| 阶段 | 频率 | 内容 |
|---|---|---|
| Pull Request | 每次 | 单元测试、确定性 Micro Benchmark、schema 迁移检查、静态隐私检查 |
| Nightly | 每晚 | 真机 Component Benchmark、内存与列表压力、错误语料 |
| Release Candidate | 每候选版 | 全量 E2E、Soak、升级、备份恢复、兼容设备矩阵 |
| Production | 抽样 | 无内容性能事件、崩溃、卡死、启动与页面指标 |

性能回归不能仅凭单次结果失败。建议同一 case 重复运行，记录 median、p90、p95、标准差或 MAD；当变化超过预设阈值并具有一致方向时阻断，边界结果进入人工 Trace 复核。

# 日志、Trace 与生产可观测性

## 统一事件结构

```json
{
  "ts_wall": "2026-08-01T12:34:56.789Z",
  "ts_mono_us": 842393942,
  "level": "info",
  "category": "dictionary.lookup",
  "event": "complete",
  "trace_id": "...",
  "span_id": "...",
  "duration_us": 14820,
  "result": "ok",
  "platform": "ios",
  "os_version": "...",
  "app_build": "...",
  "device_tier": "reference",
  "reader_backend": "readium",
  "format": "epub",
  "size_bucket": "10-50mb",
  "cache_state": "warm",
  "attributes": {}
}
```

禁止字段：书名、作者、原始路径、原文、笔记文本、词典查询词、AI prompt、用户自行设置的标签。需要关联同一本文件时使用本地旋转盐生成的不可逆短期标识，不上传稳定内容哈希。

## 事件分类

- `app.start`
- `library.import`
- `publication.open`
- `parser.*`
- `layout.*`
- `render.first_frame`
- `render.page_turn`
- `selection.*`
- `note.commit`
- `chat.layout`
- `chat.prepend`
- `dictionary.import`
- `dictionary.lookup`
- `db.*`
- `search.*`
- `sync.*`
- `ai.*`

## Span API

```swift
let span = perf.begin(
    "dictionary.lookup",
    attributes: [
        "dictionary_size_bucket": sizeBucket,
        "cache_state": cacheState,
        "lookup_mode": "exact"
    ]
)

defer { span.end(status: .ok) }
```

共享核心只依赖 Atha 的 `PerfSpan` 接口。平台导出器分别连接：

- iOS：OSSignposter、XCTest metrics、MetricKit、Instruments；
- HarmonyOS：HiTrace、HiLog、PerformanceAnalysisKit、Hypium/DevEco Testing；
- 生产可选：APMS 或其他服务，但事件结构和隐私规则由 Atha 控制。

## 本地诊断

- Debug 构建记录 100% 细粒度事件；
- Release 默认使用有界环形缓冲；
- 错误和崩溃保留必要 Trace；
- 用户主动导出“诊断包”时，对日志、设备信息、数据库 schema 和最近任务状态进行再次脱敏；
- 诊断包不包含书籍、书摘、笔记和词典查询历史；
- 日志容量、保存期和上传开关必须可见。

## 生产抽样

建议：

- 崩溃、数据库损坏和导入失败：100% 上报错误元数据，但仍不含内容；
- 常规性能：1%–5% 会话抽样；
- 高成本 Trace：远程开关且需用户同意；
- AI 请求：只记录模型、token 数、耗时和错误分类，不记录文本；
- 用户可完全退出性能分析。

# 安全与隐私

## 不可信出版物

导入文件均视为不可信输入。最低防护包括：

- 阻止 ZIP 路径穿越和符号链接逃逸；
- 对解压总量、单资源大小、资源数量、目录深度和压缩比设上限；
- EPUB 脚本默认禁用；
- 外部网络资源默认阻止；
- 每个 Edition 使用隔离 origin 或等价资源边界；
- CSP 限制脚本、连接、frame 和外部字体；
- 清洗消息富文本和词典释义 HTML；
- 大图先读取尺寸，再决定解码；
- 字体、SVG 和 CSS 进入独立审计语料；
- 外部链接需用户确认并交给系统浏览器。

## 数据安全

- 用户数据库使用平台文件保护和可选应用级加密；
- 密钥进入 Keychain / Harmony 安全存储；
- 备份文件可由用户设置密码；
- 原始书籍默认仅保存在设备；
- 删除账户前提供完整导出；
- AI 上下文发送前显示范围规则，默认最小必要；
- 第三方模型供应商和数据留存政策必须可选择、可关闭。

# 测试策略

## 测试金字塔

### 单元测试

领域模型、富文本迁移、Locator、数据库迁移、字典规范化、统计规则和隐私字段过滤。

### 格式一致性测试

同一语料在两个后端上的元数据、目录、阅读顺序、搜索和定位结果允许存在实现差异，但必须满足定义的规范语义。任何差异通过 golden result 和明确例外记录。

### 属性与模糊测试

- ZIP、XML、HTML、CSS、MOBI 索引和字典块；
- 富文本 AST 反序列化；
- Locator 和同步事件；
- 数据库迁移中断点。

### UI 自动化

覆盖导入、打开、选择、引用、发送、跳转、返回、搜索、主题和备份恢复。重点验证滚动锚点和消息可见性，而不是只截图。

### 故障注入

- 索引中途杀进程；
- 磁盘空间不足；
- 数据库写入失败；
- 文件被移动或删除；
- Reader Kit / WebView 进程终止；
- 同步重复、乱序和超时；
- AI 流式响应中断。

# 分阶段交付

## P0：架构与性能验证

目标不是做完整 App，而是消除会导致重写的技术不确定性。

必须产出：

- iOS Readium 与 Harmony Reader Kit 打开同一 EPUB/TXT 语料；
- 选择、规范 Locator、引用消息和跳回原文闭环；
- 10,000/100,000 消息原生列表原型；
- Rust 与 C++ 共享核心最小对照；
- SQLite、FTS5、Outbox 最小实现；
- 字典解析和三种索引候选的基准框架；
- 真机指标、Trace、日志脱敏和 CI 结果；
- 选型 ADR 和否决原因。

P0 退出条件：本文件“架构决策门禁”全部有实测结论，且没有关键路径依赖未验证的地区、许可或设备能力。

## P1：核心阅读与聊天笔记

- EPUB/TXT；
- 书架和导入；
- 阅读、目录、搜索和偏好；
- 引用、富文本消息、回复、编辑和跳转；
- 全局搜索；
- 基础统计；
- 本地备份和导出；
- 性能与崩溃观测上线。

## P2：MOBI/AZW 与词典

- 无 DRM 的 MOBI、AZW、AZW3；
- 生产解析器选型与许可闭环；
- 词典导入、索引和精确/变形查询；
- 多词典优先级；
- 查询面板和缓存；
- 索引迁移与故障恢复。

## P3：同步与 AI 书友

- 账户与可选同步；
- 多设备冲突和备份；
- AI 书友人格、上下文策略和费用控制；
- 来源提示和外部知识声明；
- 阅读回顾和高级统计。

# 团队工作流

建议建立以下工作流，不按平台完全割裂：

| 工作流 | 核心责任 |
|---|---|
| Product & UX | 阅读—引用—发送闭环、聊天信息架构、统计解释 |
| Reader iOS | Readium、WKWebView、定位、选择、资源性能 |
| Reader Harmony | Reader Kit、ArkUI、真机兼容与性能 |
| Portable Core | 数据模型、SQLite、搜索、字典、同步、FFI |
| Performance | 语料、基准、Trace、设备实验室和回归门禁 |
| Quality & Security | 错误文件、模糊测试、隐私、许可和发布审查 |

每个功能 PR 必须回答：新增了什么性能事件、如何在低端设备验证、失败如何恢复、是否引入内容日志、是否改变导出 schema。

# 风险与缓解

| 风险 | 影响 | 缓解 |
|---|---|---|
| Reader Kit 地区或版本限制 | HarmonyOS 目标市场缩小或功能不可用 | P0 真机和账号验证；通过 `ReaderBackend` 保留后备实现 |
| Reader Kit 定位能力不足 | 引用无法可靠跳回 | 规范 Locator + 文本上下文；必要时切换 ArkWeb 后端 |
| Readium 升级包含破坏性变化 | iOS 回归 | 固定版本；升级语料和基准；维护适配层 |
| 单一 UI 框架性能不达标 | 阅读和聊天体验不稳定 | 高负载页面原生化；普通 UI 才追求共享 |
| Rust/OpenHarmony 工具链成本 | 构建、调试和招聘风险 | 稳定 C ABI；P0 C++ 对照；不让上层依赖 Rust 类型 |
| MOBI/AZW 许可不清 | 无法商业发布 | 法律审查；参考实现与生产实现分离；准备 permissive 替代 |
| 词典索引过大或首查慢 | 杀手功能失败 | 三种索引实测；条目块按需；冷/温/热独立预算 |
| 长章节或异常 CSS | 卡顿、OOM、错版 | 语料、资源上限、可取消加载、降级路径 |
| 消息列表长期膨胀 | 内存和查询退化 | 分页、稳定 ID、异步测量、FTS、不可一次全载 |
| 统计误导用户 | 信任损失 | 指标版本、闲置规则透明、允许关闭和导出 |
| 遥测泄露阅读内容 | 高隐私风险 | 字段白名单、静态检查、脱敏诊断、用户退出 |
| 同步复杂度过早进入 | 延误核心体验 | P1 本地优先；Outbox 预留；P3 再做多端 |

# 架构决策门禁

| ADR | 待决策 | P0 证据 | 通过标准 | 否决后方案 |
|---|---|---|---|---|
| ADR-001 | 原生壳还是共享 UI | ArkUI-X、RNOH、原生对照 | 高负载场景均达预算；普通 UI 复用收益明确 | 保持原生壳 |
| ADR-002 | Rust 还是 C++ 核心 | 双端构建、FFI、包体、符号化 | 无阻断性运维问题，性能不劣于 C++ 对照 | C++ 保持同 C ABI |
| ADR-003 | Harmony Reader Kit 是否承担核心阅读 | 真机选择、定位、样式、长章节 | 引用与跳转可靠，性能达预算 | ArkWeb/统一 Web 后端 PoC |
| ADR-004 | iOS Readium 版本 | 3.8.x 语料和升级测试 | 功能、许可和性能通过 | 固定 fork 或替代后端 |
| ADR-005 | 聊天列表实现 | 10k/100k 消息测试 | 首屏、滚动、prepend、内存达预算 | 自定义布局/节点系统 |
| ADR-006 | SQLite 配置 | WAL、页大小、FTS、mmap 对照 | 正确性优先且性能达预算 | 关闭高风险优化 |
| ADR-007 | MOBI/AZW 生产解析器 | 兼容语料、许可证、性能 | 可商业分发、可维护、无内容损坏 | 自研适配或延后格式 |
| ADR-008 | 词典索引 | SQLite/mmap/FST 对照 | 冷/温/热、内存、索引大小综合最优 | SQLite 基线 |
| ADR-009 | 云同步范围 | 原始书籍与用户数据成本评估 | 隐私、成本、冲突模型可解释 | 仅同步笔记/位置 |

# 开放问题

以下问题需要产品或商业决策，不应由技术方案暗中代替：

1. HarmonyOS 首发是否只面向中国大陆，以及目标最低 API 版本；
2. iOS 最低支持版本和最低设备；
3. 是否在 P1 提供账户，还是完全本地免登录；
4. 原始书籍是否允许用户主动加密同步；
5. 富文本图片是本地附件、云附件还是两者兼有；
6. CSS 主题是否允许用户导入，如何隔离不可信 CSS；
7. 词典首发语言和合法测试语料来源；
8. AI 使用 BYOK、平台额度还是两者并存；
9. 商业模式是买断、低价订阅、同步订阅还是混合；
10. 是否公开性能基准和设备支持清单。

# 附录 A：建议数据库骨架

```sql
CREATE TABLE work (
    id              BLOB PRIMARY KEY,
    title           TEXT NOT NULL,
    created_at_ms   INTEGER NOT NULL,
    updated_at_ms   INTEGER NOT NULL
);

CREATE TABLE edition (
    id                  BLOB PRIMARY KEY,
    work_id             BLOB REFERENCES work(id),
    format              TEXT NOT NULL,
    file_fingerprint    BLOB NOT NULL,
    parser_backend      TEXT NOT NULL,
    parser_version      INTEGER NOT NULL,
    metadata_json       BLOB NOT NULL,
    imported_at_ms      INTEGER NOT NULL,
    UNIQUE(file_fingerprint)
);

CREATE TABLE conversation (
    id              BLOB PRIMARY KEY,
    edition_id      BLOB REFERENCES edition(id),
    kind            TEXT NOT NULL,
    created_at_ms   INTEGER NOT NULL
);

CREATE TABLE message (
    id                  BLOB PRIMARY KEY,
    conversation_id     BLOB NOT NULL REFERENCES conversation(id),
    author_type         TEXT NOT NULL,
    message_type        TEXT NOT NULL,
    reply_to_message_id BLOB REFERENCES message(id),
    current_revision_id BLOB,
    created_at_ms       INTEGER NOT NULL,
    deleted_at_ms       INTEGER
);

CREATE TABLE message_revision (
    id              BLOB PRIMARY KEY,
    message_id      BLOB NOT NULL REFERENCES message(id),
    schema_version  INTEGER NOT NULL,
    content_json    BLOB NOT NULL,
    plain_text      TEXT NOT NULL,
    created_at_ms   INTEGER NOT NULL
);

CREATE TABLE source_anchor (
    id                  BLOB PRIMARY KEY,
    message_id          BLOB NOT NULL REFERENCES message(id),
    edition_id          BLOB NOT NULL REFERENCES edition(id),
    canonical_json      BLOB NOT NULL,
    backend_json        BLOB,
    selected_text       TEXT NOT NULL,
    prefix_text         TEXT,
    suffix_text         TEXT,
    content_hash        BLOB NOT NULL
);

CREATE TABLE outbox_event (
    id              BLOB PRIMARY KEY,
    aggregate_type  TEXT NOT NULL,
    aggregate_id    BLOB NOT NULL,
    event_type      TEXT NOT NULL,
    payload_json    BLOB NOT NULL,
    created_at_ms   INTEGER NOT NULL,
    attempt_count   INTEGER NOT NULL DEFAULT 0,
    next_attempt_ms INTEGER
);
```

正式 schema 需要加入外键策略、索引、墓碑、租约、迁移日志和 FTS 外部内容表。代码示例仅表达边界，不作为直接生产 DDL。

# 附录 B：共享核心 ABI 示例

```c
typedef struct atha_runtime atha_runtime;
typedef struct atha_dictionary atha_dictionary;

typedef struct {
    int32_t code;
    const char *message;
} atha_result;

atha_result atha_runtime_create(
    const char *config_json,
    atha_runtime **out_runtime
);

atha_result atha_dictionary_open(
    atha_runtime *runtime,
    const char *dictionary_id,
    atha_dictionary **out_dictionary
);

atha_result atha_dictionary_lookup(
    atha_dictionary *dictionary,
    const char *query_utf8,
    const char *options_json,
    char **out_result_json
);

void atha_string_free(char *value);
void atha_dictionary_close(atha_dictionary *dictionary);
void atha_runtime_close(atha_runtime *runtime);
```

生产接口应优先提供批量调用，避免在长列表或索引阶段产生大量细粒度 FFI 往返。

# 附录 C：Benchmark Case 编号

| Case | 描述 | 关键指标 |
|---|---|---|
| B-APP-001 | 冷启动到书架 | TTI_APP、CPU、I/O、内存 |
| B-READ-001 | 打开已缓存 EPUB | TTFR_BOOK、首帧、资源命中 |
| B-READ-002 | 恢复历史位置 | T_RESTORE、锚点误差 |
| B-READ-003 | 60 Hz 连续翻页 | input-to-present、hitch |
| B-READ-004 | 超大单章节滚动 | 丢帧、内存、温控 |
| B-NOTE-001 | 选择并显示菜单 | T_SELECT |
| B-NOTE-002 | 引用并发送 | T_NOTE_COMMIT |
| B-CHAT-001 | 10k 消息首屏 | 首屏、内存、测量缓存 |
| B-CHAT-002 | 100k 消息向上分页 | T_CHAT_PREPEND、锚点位移 |
| B-DICT-001 | 20 万词头导入 | 时间、索引大小、峰值内存 |
| B-DICT-002 | 冷精确查询 | T_DICT_LOOKUP_ENGINE |
| B-DICT-003 | 温精确查询 | T_DICT_FIRST_RESULT |
| B-DB-001 | 10k 消息批量导入 | 事务时间、WAL、checkpoint |
| B-UPG-001 | 跨版本数据库迁移 | 时间、恢复、数据一致性 |

# 附录 D：代码审阅记录

## Readium Swift Toolkit

- 仓库：`readium/swift-toolkit`
- 审阅提交：`d82f44f4f05d87add9e22a8b75abbd61dce745dd`
- 关键文件：
  - `Sources/Navigator/EPUB/EPUBNavigatorViewController.swift`
  - `Sources/Navigator/EPUB/WebViewServer.swift`
- 观察：默认前后预载位置数分别为 2 和 6；资源通过自定义 `WKURLSchemeHandler` 提供；有界资源缓存与 256 KiB 缓冲；任务可取消。
- 采用方式：复用思想和 BSD 许可实现；固定版本；不让 Readium 类型穿透业务层。

## Telegram iOS

- 仓库：`TelegramMessenger/Telegram-iOS`
- 审阅提交：`6ad963e5b62d354da79040f388ae2b9132fb17b8`
- 关键文件：
  - `submodules/Display/Source/ListView.swift`
  - `submodules/Display/Source/ListViewTransactionQueue.swift`
  - `submodules/TelegramUI/Sources/ChatControllerNode.swift`
- 观察：自定义列表、事务队列、可见节点、显示链路调度和有限邻近预载。
- 采用方式：只采用独立工程原则；不复制 GPL 代码、资源或品牌。

## libmobi

- 仓库：`bfabiszewski/libmobi`
- 审阅提交：`906274205c11944b628da1c553b255acb1af7c55`
- 关键文件：
  - `README.md`
  - `src/index.c`
  - `src/parse_rawml.c`
- 观察：支持 MOBI/KF8/AZW/AZW3；重建 HTML/CSS/资源；处理词典 `orth`/`infl`；索引涉及 INDX/TAGX/ORDT；许可为 LGPL v3-or-later。
- 采用方式：P0/P2 兼容性与性能对照；生产集成必须先完成法律审查。

## foliate-js

- 仓库：`johnfactotum/foliate-js`
- 审阅提交：`78914aef4466eb960965702401634c2cb348e9b1`
- 关键文件：
  - `README.md`
  - `mobi.js`
  - `view.js`
- 观察：纯 JavaScript、模块化、支持 EPUB/MOBI/KF8；MOBI 按 pagebreak 切分；KF8 尽量按节解压，但文档说明当前 HUFF/CDIC 实现可能较慢；项目声明 API 尚不稳定；强调 EPUB CSP 安全。
- 采用方式：参考实现和对照组，不直接等同生产方案。

# 附录 E：资料来源

以下资料用于本方案的事实核验。访问日期均为 2026-08-01。

1. Huawei Developer, **Reader Kit**：https://developer.huawei.com/consumer/cn/sdk/reader-kit
2. HarmonyOS Reader Kit guide mirror, **Reader Kit 简介与约束**：https://developer.harmonyos.cool/docs/dev/app-dev/application-services/reader-kit-guide/reader-introduction/
3. Readium, **Swift Toolkit repository and compatibility matrix**：https://github.com/readium/swift-toolkit
4. Readium, **Swift Toolkit 3.8.0 release notes**：https://github.com/readium/swift-toolkit/releases/tag/3.8.0
5. Readium, **Locators model**：https://readium.org/architecture/models/locators/
6. W3C, **EPUB 3.3**：https://www.w3.org/TR/epub-33/
7. ArkUI-X, **Project and documentation**：https://github.com/arkui-x
8. React Native OpenHarmony, **RNOH project**：https://github.com/react-native-oh-library/react-native-harmony
9. Flutter, **Supported platforms**：https://docs.flutter.dev/reference/supported-platforms
10. JetBrains, **Compose Multiplatform supported platforms**：https://www.jetbrains.com/help/kotlin-multiplatform-dev/supported-platforms.html
11. Rust Project, **OpenHarmony target support**：https://doc.rust-lang.org/rustc/platform-support/openharmony.html
12. SQLite, **Write-Ahead Logging**：https://www.sqlite.org/wal.html
13. SQLite, **Release history**：https://www.sqlite.org/changes.html
14. SQLite, **FTS5 extension**：https://www.sqlite.org/fts5.html
15. SQLite, **WITHOUT ROWID tables**：https://www.sqlite.org/withoutrowid.html
16. SQLite, **Memory-Mapped I/O**：https://www.sqlite.org/mmap.html
17. Apple Developer, **Improving your app with Xcode metrics and Organizer**：https://developer.apple.com/documentation/xcode/improving-your-app-with-xcode-metrics
18. Apple Developer, **XCTest performance metrics**：https://developer.apple.com/documentation/xctest/performance-tests
19. Huawei Developer, **DevEco Testing / Hypium / APMS documentation**：https://developer.huawei.com/consumer/cn/doc/
20. Telegram, **Telegram iOS source**：https://github.com/TelegramMessenger/Telegram-iOS
21. libmobi, **MOBI/KF8 parser**：https://github.com/bfabiszewski/libmobi
22. foliate-js, **Browser ebook renderer**：https://github.com/johnfactotum/foliate-js

# 结论

Atha 的首要工程目标不是“用一个框架写出两个看起来相同的 App”，而是让两个平台都稳定实现同一条核心链路：

> 打开书足够快，阅读足够稳，选择原文足够准，发送笔记足够轻，长期消息足够可检索，词典查询足够快，所有数据足够可靠且属于用户。

因此，当前最稳妥的基线是：

- 平台原生壳和原生高负载界面；
- iOS Readium、HarmonyOS Reader Kit 的可替换后端；
- 稳定 C ABI 后的共享核心；
- SQLite 与消息事件模型；
- 首次导入构建的词典索引；
- 从 P0 即存在的真机 Benchmark、Trace、日志隐私和发布门禁。

在 P0 证据形成前，Rust、ArkUI-X、RNOH、MOBI 解析器和词典索引均是候选，而不是信仰。项目应允许数据推翻预设，同时确保任何替换不会破坏用户数据和产品语义。

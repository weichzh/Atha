# 路线图

路线图回答“为什么现在做这件事”。只有 `Now` 可直接进入实施；每个跨模块阶段仍按 `docs/workflow/PROTOCOL.md` 建立并验收一个最小 change。

核心阅读、主流非 PDF 格式、本地书架、消息式阅读、CSS、统计和离线词典已经形成完整主循环。当前目标是在保留 ReaderManifest、BookRoot、Locator、内容安全与隐私边界的前提下，把这些能力整理成可长期使用、可恢复、适合桌面工作的本地阅读产品。

## Done：离线词典

指定 MOBI / MDict 本地样本、当前 Kindle 词典早拒绝和成熟 parser / 索引方案已复核并落地。切片只实现本地索引、精确查词和安全释义渲染；释义保留固定白名单内的语义结构，并由应用统一提供词典排版，不加载来源 CSS、富 MDD 资源或网络内容。结果页按 Readest 模式提供设置入口，当前来源和六档独立释义字号可本地恢复。PCT-AL10 已覆盖安装同签名 arm64 release 本地测试候选并保留原书架与词典；ADB 自动化确认选区查词、安全富文本、设置页、六档字号及 150% 重启恢复，随后恢复原 100% 偏好。该轮是真实目标上的自动化取证，不替代自然手指触摸验收；既有真实长按和直接查词证据仍来自富文本改造前的同机版本。

日常功能与界面门继续使用 Linux Tauri / WebKitGTK；指定样本 benchmark 先在 Linux 固定环境形成可重复基线，Android 仅在需要验证 ARM 真机性能时运行专项门。停止条件是索引和首查 / 热查开销受控、释义不越过内容安全边界，并且阅读翻页、排版与 Locator 恢复不回退。

## Done：阅读操控与设置界面

以 PCT-AL10 上已安装的微信读书与 Readest 原图、真实拖动和滑动行为为准，右上设置已改为移动底部抽屉和分层页面，首行缩进、16–40 字号滑块与左右翻页 / 上下滚动两种模式已落地。字号以逻辑 CSS px 记录并按 DPR 换算内部设备像素；没有引入手势、动画或通用 UI 框架。

## Done：项目收口

`changes/` 只保留活动变更，`research/` 只保留尚未形成结论的问题；关闭过程和研究原文由 Git 与 `project-workflow` 追溯。阅读手势、公式密集章节的当前页优先加载和 PCT-AL10 空白尾页修复已经落入阅读内核事实所有者。

未实现的 TTS 假入口已经移除；PCT-AL10 上的词典富文本与设置也已完成同签名覆盖安装后的真实目标自动化复核。

## Done：完整本地数据生命周期

schema 1 `.atha-data` 已组合规范化书架、全部耐久书源、离线词典、MessageStore 完整备份和生产浏览器状态，并排除导入缓存、临时文件与日志。恢复先完成路径、容量、哈希和各事实所有者的语义校验，再以 staging、rollback、恢复日志和浏览器确认发布；书架同时提供分类占用，以及语义不同的“移出书架”和“删除本地数据”。

公开往返、损坏输入、prepared / publishing / committed 恢复、空间总计、owned root reparse-point 拒绝、两级删除、浏览器状态事务、Linux Tauri 360 / 1000 宽度管理界面和私有词典内容无输出往返已通过。Android 只复用既有 SAF bridge，本阶段没有把 Linux / 本地结果称为 PCT-AL10 资料库验收，也没有引入同步 schema、provider registry 或第二套数据根。

## Done：跨书阅读记忆中心

资料库已从现有 schema 1 阅读统计投影最近阅读，并在 MessageStore 既有 FTS5 上提供跨书消息搜索。搜索结果复用 Edition、当前根 Anchor 和 SourceSnapshot；只有完整内容身份仍在书架时才出现“跳回原书”，阅读器再次验证 Edition、根 Message、Locator 和正文位置后才定位对话，缺书结果只提供当前与历史 Snapshot。

实现未增加数据库、索引、统计 schema、同步模型或独立知识库事实。公开测试覆盖跨 Edition 排序、短查询、命中与根消息墓碑；Linux Tauri 真壳覆盖 360 / 1000 宽度、最近阅读、有书 / 缺书搜索结果、当前 / 历史 Snapshot、安全跳回和 AppLog 隐私。

## Done：桌面阅读工作区

目录、书内搜索和消息已经在宽屏投影为与书页共存的互斥左侧工作区，并补齐键鼠与焦点导航。分页使用真实 reader frame，经既有 Navigation / Locator 在工作区出现、隐藏和窗口变化后恢复正文位置；窄屏继续复用覆盖式工具面板，没有建立桌面专用状态或阅读内核。

## Done：日常入口与内部可安装候选

桌面资料库已经补齐原生拖放、网格 / 列表视图和冷启动文件关联；三个入口共用既有 importer、LocalLibrary、去重、失败投影和内容安全边界，没有增加队列、单实例服务或书架状态。

Linux 内部 AppImage 已由解包的 `AppRun` 通过 metadata、冷启动关联、重复关联、普通启动与完整多视口真壳回归；PCT-AL10 内部候选已通过同包同签名非降级覆盖安装、16 KiB、首次安装时间保持和启动烟测。Windows 仍只有打包配置静态检查，这些内部候选均未发布，也不等同于生产签名验收。

## Now：内部候选观察与平台补证

当前不新增产品功能。先用内部候选收集日常入口问题；下一份 change 只从真实缺口中选择一个纵向切片，优先级依次是 Windows NSIS 文件关联实机验收、真实 OS 鼠标拖放、PCT-AL10 自然触摸与移动功能 / 性能补证，最后才是生产签名、自动更新和正式分发。

数据丢失、内容安全、引用错位和 Android 性能回归始终高于体验扩展。每一项只在前一切片关闭后进入 `Now`，不预建跨格式工厂、社区服务或同步 schema。

## Later

- AI 书友、账户、云同步与多设备一致性；
- 基于 GitHub PR 的 CSS 社区；当前只保留 schema 1 模块包 codec，不预建登录、网络、provider registry、仓库或占位页面；
- iOS、HarmonyOS 和其他移动平台；
- PDF 与 OCR（明确不在本轮 Readest 格式目标内）；
- TTS、翻译、RSVP、平行阅读、商业化、遥测上传和生产发布。

这些方向不授权继续预建 adapter、数据库 schema、占位页面或后台服务。

## Done

| 阶段 | 已交付能力 | 当前事实所有者 |
| --- | --- | --- |
| M0 | 项目记忆、任务路由、检查入口与事实所有权 | `AGENTS.md`、`docs/agents/workflow.md` |
| M1 | Windows Rust workspace、正式后端与基础数据边界 | `docs/codebase/MAP.md`、`docs/codebase/DATABASE.md` |
| M2 | WebView2 阅读内核：会话、Locator、排版、交互、恢复、书签、搜索、标注与困难样本门槛 | `docs/architecture/READER-CORE.md` |
| M3 | EPUB3 导入、受控书根、稳定内容身份与真实书籍入口 | `docs/architecture/READER-CORE.md`、`docs/codebase/MAP.md` |
| M4 | Tauri / Svelte 产品壳、本地书架、消息式阅读、快照资产恢复、导出和消息数据库备份 | `docs/product/OVERVIEW.md`、`docs/architecture/MESSAGE-READING.md`、相关 ADR |
| Android EPUB 纵切 | Tauri Android 工程、系统 picker / content URI 桥、EPUB 导入阅读、持久恢复与正式模拟器门禁 | `docs/architecture/READER-CORE.md`、`docs/codebase/MAP.md` |
| EPUB2 / NCX 子集 | OPF2 `spine@toc`、有界 NCX 前序目录、legacy cover、XHTML 1.1 与 Android 目录 / 位置恢复 | `docs/architecture/READER-CORE.md` |
| CBZ JPEG / PNG | 自然序图片 section、ComicInfo 基础元数据、坏页继续、Windows WebView2 与 Android 16 KiB picker / 恢复 / PSS 门禁 | `docs/architecture/READER-CORE.md`、`docs/codebase/MAP.md` |
| 离线书架体验 | 本地标题 / 作者搜索、严格进度二态、稳定排序、显式批量选择、响应式三列与 Windows / Android 正式门 | `docs/architecture/READER-CORE.md`、`docs/codebase/MAP.md` |
| Markdown / TXT | 安全 Markdown 投影、遗留编码 TXT 章节、分组 sections、Android picker / 目录 / 搜索 / 恢复与十样本模拟器基线 | `docs/architecture/READER-CORE.md`、`docs/agents/references.md` |
| Linux Tauri 目标 | Tauri / WebKitGTK 日常 GUI 门、平台协议 URL、WebDriver 截图与日志隐私检查；Android 改为发布前或移动专项门 | `docs/architecture/READER-CORE.md`、`docs/agents/references.md` |
| FB2 / FBZ | 有界流式 XML、单根成员 FBZ、metadata / 封面 / 目录 / 内部链接投影、稳定内容身份与 Linux Tauri GUI 纵切 | `docs/architecture/READER-CORE.md`、`docs/agents/references.md` |
| MOBI / AZW / AZW3 | 固定 `boko 0.5.0` 的有界 adapter、PalmDOC / MOBI6 / 纯 KF8、图片 / 唯一目录投影、词典早拒绝、十次 release benchmark 与 Linux 真 GUI 纵切 | `docs/architecture/READER-CORE.md`、`docs/agents/references.md` |
| CSS 编辑与模块管理 | 可视排版、按需 CodeMirror 6、实时预览、32 个有界模块、筛选 / 排序 / 批量启停 / JSON 交换、旧状态迁移、失败回退与 Linux 真 GUI 门 | `docs/architecture/READER-CORE.md` |
| CSS 社区模块边界 | schema 1 模块包的独立解析、序列化、字段 / 大小 / 重复 ID / CSSOM 校验接口；没有网络、GitHub、登录或社区 UI | `reader/web/style-module-package.mjs`、`docs/architecture/READER-CORE.md` |
| 本地阅读统计 | 稳定 / 沉浸 / 可见 / 聚焦 / 活动计时、跨日本地聚合、今日 / 近 7 天 / 本书 / 连续阅读投影，以及 Linux 真 GUI 生命周期与性能门 | `reader/web/reader-state.mjs`、`docs/architecture/READER-CORE.md` |
| 完整本地数据生命周期 | `.atha-data` 完整备份 / 恢复、恢复日志、分类占用、两级删除和 Linux 真壳管理界面 | `docs/decisions/ADR-0010-local-data-lifecycle.md`、`docs/architecture/READER-CORE.md` |
| 跨书阅读记忆中心 | 最近阅读、跨书 Message 搜索、有书安全跳回、缺书及历史 Snapshot | `docs/architecture/MESSAGE-READING.md`、`docs/codebase/DATABASE.md` |
| 桌面阅读工作区 | 宽屏目录 / 搜索 / Message 侧栏、键鼠焦点、真实 frame 分页与窄屏回归 | `docs/architecture/READER-CORE.md`、`docs/codebase/MAP.md` |
| 日常入口与内部候选 | 桌面拖放、网格 / 列表、冷启动文件关联、Linux AppImage 与 PCT-AL10 内部候选门 | `docs/architecture/READER-CORE.md`、`docs/codebase/MAP.md` |
| 阅读操控与排版设置 | Readest 风格分层底部设置、16–40 字号滑块、DPR 设备像素换算、首行缩进、左右翻页 / 上下滚动和 PCT-AL10 真机手势 | `docs/architecture/READER-CORE.md`、`docs/codebase/READER-MOBILE-UI.md` |
| 性能切片 | 公式密集章节按当前页优先加载，固定样本继续受正式门槛保护 | `docs/architecture/READER-CORE.md` |

精确实施过程、历史验收数字和关闭收据由 Git 与 `project-workflow` 保存，不在路线图重复维护。

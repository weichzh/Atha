# 路线图

路线图回答“为什么现在做这件事”。只有 `Now` 可直接进入实施；每个跨模块阶段仍按 `docs/workflow/PROTOCOL.md` 建立并验收一个最小 change。

核心阅读、EPUB3、本地书架和消息式阅读已经形成完整主循环，Android EPUB 纵切也已通过真实模拟器链路。当前目标是在保留现有 ReaderManifest、BookRoot、Locator 与内容安全边界的前提下，逐步补齐 Readest 已成熟支持的非 PDF 格式，再交付 CSS、统计和离线词典能力。

## Now：CSS 编辑器与模块管理

Readest 对应的非 PDF 输入格式已经归一到同一 ReaderManifest、BookRoot、Locator 与安全渲染边界。下一切片增强现有每书用户 CSS：默认使用字号、字体、行距、段距、页边距、主题与翻页等可视控件和实时预览，原始 CSS 只作为高级模式；样式模块可组合、排序、启停、搜索、分组、导入导出和失败回退，不建立第二套渲染器或通用扩展运行时。

日常目标端检查和 GUI 调试继续使用 Linux GNOME 会话与 Tauri / WebKitGTK；Windows 只在用户明确要求时使用，Android 模拟器只在发布前或移动端专项验收时启动。界面优先借鉴微信读书的沉浸阅读、固定工具分组、排版即时预览与渐进高级设置，不引入其社交、排行、付费、音频或悬浮 AI 入口。停止条件是同一 CSS 安全规则贯穿编辑、预览、模块组合、持久化和渲染，任一无效模块可定位、禁用并回退到上次有效组合。

## Next：按格式风险逐片交付

1. **CSS 社区**：GitHub 仓库作为存储与 review 边界，用户登录 GitHub 后以 pull request 投稿，不自建账号或审核后端；
2. **阅读统计**：本地优先记录阅读时长、进度与连续阅读，先明确暂停、后台和跨设备语义；
3. **离线词典**：先实现本地索引、查词与安全释义渲染，再用指定 MOBI / MDict 样本在 Android ARM 真机做基准；AGPL 兼容不替代词典内容版权与分发审查。

数据丢失、内容安全、引用错位和 Android 性能回归始终高于体验扩展。每一项只在前一切片关闭后进入 `Now`，不预建跨格式工厂、社区服务或同步 schema。

## Later

- AI 书友、账户、云同步与多设备一致性；
- iOS、HarmonyOS 和其他移动平台；
- PDF 与 OCR（明确不在本轮 Readest 格式目标内）；
- 翻译、RSVP、平行阅读、商业化、遥测上传和生产发布。

这些方向不授权预建 interface、adapter、数据库 schema、占位页面或后台服务。

## Done

| 阶段 | 已交付能力 | 当前事实所有者 |
| --- | --- | --- |
| M0 | 项目记忆、任务路由、检查入口与事实所有权 | `AGENTS.md`、`docs/agents/workflow.md` |
| M1 | Windows Rust workspace、正式后端与基础数据边界 | `docs/codebase/MAP.md`、`docs/codebase/DATABASE.md` |
| M2 | WebView2 阅读内核：会话、Locator、排版、交互、恢复、书签、搜索、标注与困难样本门槛 | `docs/architecture/READER-CORE.md` |
| M3 | EPUB3 导入、受控书根、稳定内容身份与真实书籍入口 | `docs/architecture/READER-CORE.md`、`docs/codebase/MAP.md` |
| M4 | Tauri / Svelte 产品壳、本地书架、消息式阅读、快照资产恢复、导出和全库备份 | `docs/product/OVERVIEW.md`、`docs/architecture/MESSAGE-READING.md`、相关 ADR |
| Android EPUB 纵切 | Tauri Android 工程、系统 picker / content URI 桥、EPUB 导入阅读、持久恢复与正式模拟器门禁 | `docs/changes/android-epub-vertical-slice.md`、`docs/codebase/MAP.md` |
| EPUB2 / NCX 子集 | OPF2 `spine@toc`、有界 NCX 前序目录、legacy cover、XHTML 1.1 与 Android 目录 / 位置恢复 | `docs/architecture/READER-CORE.md`、`docs/changes/android-epub2-ncx-compatibility.md` |
| CBZ JPEG / PNG | 自然序图片 section、ComicInfo 基础元数据、坏页继续、Windows WebView2 与 Android 16 KiB picker / 恢复 / PSS 门禁 | `docs/architecture/READER-CORE.md`、`docs/changes/android-cbz-vertical-slice.md` |
| 离线书架体验 | 本地标题 / 作者搜索、严格进度二态、稳定排序、显式批量选择、响应式三列与 Windows / Android 正式门 | `docs/architecture/READER-CORE.md`、`docs/changes/weread-offline-library-ui.md` |
| Markdown / TXT | 安全 Markdown 投影、遗留编码 TXT 章节、分组 sections、Android picker / 目录 / 搜索 / 恢复与十样本模拟器基线 | `docs/architecture/READER-CORE.md`、`docs/changes/android-markdown-txt-vertical-slice.md` |
| Linux Tauri 目标 | Tauri / WebKitGTK 日常 GUI 门、平台协议 URL、WebDriver 截图与日志隐私检查；Android 改为发布前或移动专项门 | `docs/architecture/READER-CORE.md`、`docs/agents/references.md` |
| FB2 / FBZ | 有界流式 XML、单根成员 FBZ、metadata / 封面 / 目录 / 内部链接投影、稳定内容身份与 Linux Tauri GUI 纵切 | `docs/architecture/READER-CORE.md`、`docs/changes/android-fb2-vertical-slice.md` |
| MOBI / AZW / AZW3 | 固定 `boko 0.5.0` 的有界 adapter、PalmDOC / MOBI6 / 纯 KF8、图片 / 唯一目录投影、词典早拒绝、十次 release benchmark 与 Linux 真 GUI 纵切 | `docs/architecture/READER-CORE.md`、`docs/changes/kindle-format-vertical-slice.md` |
| 性能切片 | 公式密集章节按当前页优先加载，固定样本继续受正式门槛保护 | `docs/architecture/READER-CORE.md` |

精确实施过程、历史验收数字和关闭收据由 Git 与 `project-workflow` 保存，不在路线图重复维护。

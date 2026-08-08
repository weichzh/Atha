# 路线图

路线图回答“为什么现在做这件事”。只有 `Now` 可直接进入实施；每个跨模块阶段仍按 `docs/workflow/PROTOCOL.md` 建立并验收一个最小 change。

核心阅读、EPUB3、本地书架和消息式阅读已经形成完整主循环，Android EPUB 纵切也已通过真实模拟器链路。当前目标是在保留现有 ReaderManifest、BookRoot、Locator 与内容安全边界的前提下，逐步补齐 Readest 已成熟支持的非 PDF 格式，再交付 CSS、统计和离线词典能力。

## Now：阅读统计

Readest 对应的非 PDF 输入格式与本地 CSS 编辑 / 模块管理已经归一到同一 ReaderManifest、BookRoot、Locator 与安全渲染边界。下一切片本地优先记录阅读时长、进度与连续阅读，先明确前台阅读、暂停、后台、跨日和重开语义，再选择最小持久化与界面投影。

统计保持本地、可解释且不上传；停止条件是时长计量在失焦、休眠、异常退出和重启后不虚增，日 / 周 / 书籍投影可验证，并且记录开销不影响翻页、排版或 Locator 恢复。

## Next：按格式风险逐片交付

1. **离线词典**：先实现本地索引、查词与安全释义渲染，再用指定 MOBI / MDict 样本在 Linux 日常门与 Android ARM 真机专项做基准；AGPL 兼容不替代词典内容版权与分发审查。

数据丢失、内容安全、引用错位和 Android 性能回归始终高于体验扩展。每一项只在前一切片关闭后进入 `Now`，不预建跨格式工厂、社区服务或同步 schema。

## Later

- AI 书友、账户、云同步与多设备一致性；
- 基于 GitHub PR 的 CSS 社区；当前只保留 schema 1 模块包 codec，不预建登录、网络、provider registry、仓库或占位页面；
- iOS、HarmonyOS 和其他移动平台；
- PDF 与 OCR（明确不在本轮 Readest 格式目标内）；
- 翻译、RSVP、平行阅读、商业化、遥测上传和生产发布。

这些方向不授权继续预建 adapter、数据库 schema、占位页面或后台服务。

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
| CSS 编辑与模块管理 | 可视排版、按需 CodeMirror 6、实时预览、32 个有界模块、筛选 / 排序 / 批量启停 / JSON 交换、旧状态迁移、失败回退与 Linux 真 GUI 门 | `docs/architecture/READER-CORE.md`、`docs/changes/css-editor-module-management.md` |
| CSS 社区模块边界 | schema 1 模块包的独立解析、序列化、字段 / 大小 / 重复 ID / CSSOM 校验接口；没有网络、GitHub、登录或社区 UI | `reader/web/style-module-package.mjs`、`docs/architecture/READER-CORE.md` |
| 性能切片 | 公式密集章节按当前页优先加载，固定样本继续受正式门槛保护 | `docs/architecture/READER-CORE.md` |

精确实施过程、历史验收数字和关闭收据由 Git 与 `project-workflow` 保存，不在路线图重复维护。

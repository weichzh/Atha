# 路线图

路线图回答“为什么现在做这件事”。只有 `Now` 可直接进入实施；每个跨模块阶段仍按 `docs/workflow/PROTOCOL.md` 建立并验收一个最小 change。

核心阅读、EPUB3、本地书架和消息式阅读已经形成完整主循环，Android EPUB 纵切也已通过真实模拟器链路。当前目标是在保留现有 ReaderManifest、BookRoot、Locator 与内容安全边界的前提下，逐步补齐 Readest 已成熟支持的非 PDF 格式，再交付 CSS、统计和离线词典能力。

## Now：Markdown / TXT

复用现有 ReaderManifest / BookRoot / Locator，把 Markdown 与纯文本转换为受控 section，不建立第二套 reader 或通用格式 factory。正向兼容和性能验收直接使用用户放入 `fixtures/local` 的本机书籍，不再为每种格式另造书籍样本；最小恶意输入只用于可重复的信任边界测试，不提交本地书籍或其派生内容。

停止条件：`fixtures/local` 中代表性的 Markdown / TXT 能在 Android 系统 picker 中导入、排版、搜索、定位并在强停后恢复；转换层保持有界且不放行原始 HTML 主动内容。成熟 Markdown parser 已覆盖语法时不自写 parser。

## Next：按格式风险逐片交付

1. **FB2**：优先采用成熟解析库并保持相同 Locator 与安全渲染边界；
2. **MOBI / AZW / KF8 / AZW3**：借鉴 Readest 的 foliate-js 能力边界，最终方案先过许可证审查，并以 Android 真机内存、冷开、翻页和重排门槛验收；
3. **CSS 编辑器与模块管理**：每书覆盖、可组合模块、预览、撤销、导入导出和失败回退；
4. **CSS 社区**：GitHub 仓库作为存储与 review 边界，用户登录 GitHub 后以 pull request 投稿，不自建账号或审核后端；
5. **阅读统计**：本地优先记录阅读时长、进度与连续阅读，先明确暂停、后台和跨设备语义；
6. **离线词典**：先实现本地索引、查词与安全释义渲染，再用指定 MOBI / MDict 样本在 Android ARM 真机做基准；AGPL 兼容不替代词典内容版权与分发审查。

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
| 性能切片 | 公式密集章节按当前页优先加载，固定样本继续受正式门槛保护 | `docs/architecture/READER-CORE.md` |

精确实施过程、历史验收数字和关闭收据由 Git 与 `project-workflow` 保存，不在路线图重复维护。

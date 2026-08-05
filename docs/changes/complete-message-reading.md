---
description: 完整实现本地消息式阅读、引用存档、关系回顾和可迁移数据。
---

# 完整消息式阅读

## Status

accepted

## Problem

阅读器已经完成原生选择、标注、笔记、range Locator、重锚和 `SourceAnchor`，P0 也验证了 SQLite、消息修订、FTS5 与事务 Outbox 的基本语义；但这些仍是消息式阅读的前置能力，不是产品主循环本身。当前没有正式 `Conversation`、`Message`、不可变修订、历史渲染快照、消息引用关系、对话回顾、消息搜索或可迁移导出。

如果继续在 localStorage Annotation 和正式 Message 之间维护两套事实，同一条阅读反应会拥有两套编辑、删除、重锚、搜索和导出状态，无法保证一致。标注、笔记和对话的差别应留在投影与当前修订，不应成为互相复制的领域对象。

## Domain Model

- `Conversation` 是一个 Edition 内按创建顺序组织的消息集合，由一条引用原文的根 Message 开始；
- `Message` 是标注、笔记、回复和引用共享的稳定身份与关系节点，软删除不删除其修订、引用或快照；
- `MessageRevision` 是不可变内容版本；source-only 修订表示纯标注，text 修订保存笔记或回复；添加笔记和编辑只追加修订并原子切换当前修订；第一版正文为限长纯文本，持久化为 schema 1 结构化 JSON 与纯文本投影；
- `SourceAnchor` 保存不可变的原始 Locator、原文、上下文和内容哈希，并单独保存可唯一重锚更新的当前 Locator；它负责跳回当前 Edition；
- `SourceSnapshot` 是引用创建时的不可变历史呈现，包含已校验的选区 HTML、当时实际启用的 reader/book/user CSS、阅读呈现参数和被选区引用的本地资源；它负责历史展示；
- 用户主动重选原文会生成新的 `SourceAnchor` 与 `SourceSnapshot` 版本并切换当前版本，旧版本仍可回看；自动重锚只更新当前 Locator；
- `MessageReference` 是从一条消息指向另一条既有消息的有向引用；回复是至多一个父消息，引用可以有多个目标；
- `OutboxEvent` 与消息事实同事务写入，只保存未来异步处理所需的本地事件，不在本次发送网络请求。

## Scope

### 正式后端与数据

- 按 ADR-0002 固定引入 `rusqlite 0.40.1` bundled，在正式后端提供仅向前、`PRAGMA user_version`、`IMMEDIATE` 事务迁移；
- 新建本地消息数据库，正式保存 Work、Edition、Conversation、Message、MessageRevision、SourceAnchor、SourceSnapshot、MessageReference、SnapshotResource 与 OutboxEvent；
- 以现有 EPUB 内容哈希登记 Edition；书架移除不删除消息、快照或导出所需资产，同一内容重新导入后恢复导航；
- 以内容寻址文件保存快照资源；数据库只登记资源映射、媒体类型、长度和 SHA-256；
- 写入接口完成创建 source-only 标注、为同一根 Message 添加笔记、回复、引用既有消息、重选原文、追加修订与软删除；查询接口完成本书对话、单个对话、章节过滤、当前修订全文检索、正反向引用和删除墓碑；
- 每次事实写入和对应 OutboxEvent 同事务提交；版本冲突、损坏数据、未来 schema、未知 Edition/Message、越界内容和资产失败均返回脱敏稳定错误，不能部分成功；
- 导出单个对话或一本书的全部对话为自包含归档，包含 schema、修订历史、关系、引用、快照与资源；导出不包含书籍源路径、诊断信息或其他书籍数据。

### 阅读器与应用

- 选择工具继续只显示复制、标注和笔记，不增加重复的“引用”按钮；标注创建 source-only 根 Message，笔记创建或更新带正文的同一根 Message；
- 首次启用正式消息数据库时，把现有有效 localStorage 标注与笔记逐条导入为 Message；全部提交成功并校验数量与哈希后才标记迁移完成，失败时保留原数据并可重试；
- 阅读内核从已校验 DOM 生成 `SourceAnchor` 与 `SourceSnapshot` 候选，只暴露一个捕获接口；Tauri 后端再次执行字段、大小、资源路径、媒体类型和哈希验证；
- 创建笔记或打开已有标注后可在阅读页展开可收起、可调整大小的对话浮层，显示短引用预览和消息编辑框；保存、关闭或点击正文后可立即回到阅读；
- 浮层支持查看当前对话、回复、引用对话内既有消息、编辑自己的消息、软删除、查看修订历史、查看历史引用快照和跳回当前原文；
- 笔记全屏页投影所有根 Message：source-only 显示为标注，text 显示为笔记；同一页面进入对话、按章节和搜索词筛选及引用关系回顾，不维护第二份前端事实；
- 应用重启、书籍重开、字号或样式改变后恢复对话；历史快照显示保持不变，跳转使用当前 Locator 并在可唯一重锚时更新当前 Locator；
- 所有普通用户界面使用稳定中文文案，不暴露数据库路径、内部 ID、源路径、SQL 或后端字段名。

### Interface And Module Shape

- `backend::messages` 是深 module：迁移、严格模型校验、事务、FTS、关系、快照资产和导出都藏在 `MessageStore` interface 后；只有 SQLite 这一种真实实现，不增加 trait 或 repository adapter；
- Tauri command 是产品界面与 `MessageStore` 的窄 seam，只接受受限 DTO，不转发 SQL、文件路径或任意资源 URL；
- 阅读内核的 `message-capture` module 只负责从当前已验证 Range 构造候选和重锚，并把现有高亮、笔记列表改为 Message 投影；Svelte 对话浮层只负责交互状态，不拥有耐久消息事实；
- 测试通过与调用方相同的 `MessageStore` interface 使用临时 SQLite 文件，不测试内部表实现细节。

### Approved Workflow Interruption

2026-08-04，用户在实现期间明确要求改进全局 `project-workflow`：项目必须提供固定的官方文档入口和快速使用说明，CLI 在入口缺失时拒绝 start，setup 负责创建或修复。该流程改动不是消息产品能力，但经用户单独批准；因 Atha docs gate 检查整个脏工作树，项目端 `docs/agents/references.md`、workflow/INDEX/CONTEXT 更新由当前 task 明确 adopt，避免另一个 task 与同一工作树互相阻塞。

## Non-Goals

- 不实现 AI 调用、AI 角色、群组、账户、云同步、网络发送或多人冲突合并；
- 不实现脱离书籍的通用聊天首页、即时通讯在线状态、通知、语音、附件上传或表情系统；
- 不提供富文本编辑器；schema 1 只支持纯文本消息，但保留不可变结构化修订；
- 不物理删除旧 localStorage Annotation；成功迁移后只停止继续写入，保留一个 release 的只读回退窗口；
- 不把 P0 DDL、SQLite CLI、故障脚本或 benchmark 直接复制进正式迁移；
- 不因消息功能改写 WebView2 分页、EPUB importer、书架身份或既有阅读性能路径。

## Acceptance Criteria

- [x] 正式 SQLite 从空库迁移、重复打开、事务回滚、未来版本拒绝、外键、FTS5 与 integrity check 均通过；应用数据库不是 P0 文件；
- [ ] 真实书籍选区可创建带不可变 `SourceSnapshot` 的 source-only 标注或带正文笔记；两者是同一 Message 事实的不同当前修订和投影；
- [x] 已有 localStorage 标注与笔记可原子、幂等迁移；失败不丢失或覆盖旧数据，成功后数量、原文哈希、笔记、墓碑和跳转保持；
- [x] 可回复消息、引用一个或多个既有消息、查询正反向关系，并拒绝跨 Edition、未知、删除或自引用目标；
- [x] 编辑追加 `MessageRevision` 且旧修订可查看；并发旧版本编辑明确冲突；软删除保留历史和关系但默认列表不显示正文；
- [x] 当前修订全文搜索只返回当前未删除消息，可按书籍和章节过滤并跳转；完整对话、章节回顾和关系视图来自同一份后端事实；
- [ ] 历史快照包含已校验 HTML、实际 CSS、呈现参数与所需本地资源；更改主题、字号、本书 CSS 或重新打开后历史呈现内容和资产哈希不变；
- [ ] 对话浮层可收起、可调整大小，键盘和读屏基础完整；标注、笔记、回复、编辑、重选、删除、修订、引用、搜索、快照与跳转均有可见成功或失败状态；
- [ ] 应用重启和同内容重新导入后消息可恢复并跳回；书架移除不删除消息；损坏数据库或快照不能被静默覆盖；
- [x] 对话或本书消息可导出为自包含归档，重新校验归档可证明 schema、关系、修订、快照和资源完整，不泄漏原始路径；
- [ ] 真实《数学及其历史》完成“选择 → 标注 → 同一记录添加笔记 → 回复 → 引用消息 → 编辑 → 重选 → 搜索 → 快照 → 跳回 → 重启恢复 → 导出”闭环；
- [x] 现有书架、四样本阅读器、安全、标注、持久化与 Tauri benchmark 无 blocking 回退，消息列表与输入规模基线有固定门槛；
- [ ] 独立 standards 与 spec review 均无 blocking。

## Files And Steps

1. 在正式后端实现迁移和 `MessageStore`，先用 interface 级集成测试固定领域不变量、失败原子性和 FTS 当前修订语义；
2. 实现内容寻址 `SourceSnapshot` 资产和自包含导出，验证资源边界、完整性和删除/重导语义；
3. 增加受限 Tauri commands 与前端 TypeScript client，不把数据库和文件系统细节暴露给阅读内核；
4. 把现有 Annotation Store 收敛为一次性迁移输入，在阅读内核增加选区捕获与 Message 投影，在 Svelte 增加对话浮层和本书消息回顾；
5. 用真实样书完成端到端恢复、历史呈现、搜索、关系和导出检查，再运行现有 reader、library 与 Tauri 回归；
6. 更新消息架构、数据库事实、代码地图和路线图，独立 review 后关闭 change。
7. 按用户批准补充全局 workflow 的参考地图门禁，并在 Atha 建立 Tauri、Svelte、WebView2、EPUB 与 SQLite 官方入口。

## Checks

- `cargo fmt --all -- --check`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo test --workspace`
- `cargo doc --workspace --no-deps`
- `pnpm --dir reader/app check`
- `pnpm --dir reader/app build`
- `pwsh -NoProfile -File scripts/check-message-reading.ps1`
- `pwsh -NoProfile -File scripts/check-reader-samples.ps1`
- `pwsh -NoProfile -File scripts/check-library-shelf.ps1`
- `pwsh -NoProfile -File scripts/check-tauri-reader.ps1`
- `pwsh -NoProfile -File scripts/Invoke-Atha.ps1 check docs -Activity validation -Scope project-workflow`
- `autocorrect --fix` 与 `autocorrect --lint` 仅处理本次中文 Markdown
- `git diff --check`

## Rollback

代码按本 change 的提交整体回退。迁移只向前，不提供 down migration；回退应用前保留消息数据库与快照资产，由升级前备份或兼容版本恢复，任何回退操作都不得删除用户消息。

## Approval

2026-08-04：用户确认现有引用与笔记前置能力已经完成；消息式阅读是重要功能，要求按既有产品和架构文档完整实现，并批准开始。同日进一步确认标注、笔记与引用不应强制分成不同事实，采用统一 Message 模型。

同日，用户指出反复诊断没有优先查官方 Tauri 文档，明确批准中断实现并优化全局 `project-workflow` 与项目参考地图；完成后继续消息实现。

## Result

正式消息数据与阅读器交互已经实现：`MessageStore` 拥有 schema v2 SQLite、快照资产、迁移、关系、搜索、导出和事务 Outbox；Tauri 只暴露受限 command，阅读内核只捕获候选并投影根 Message；Svelte 阅读页提供全屏笔记页和对话浮层。旧 localStorage 标注只作为一次性迁移输入，不再与正式事实双写。

现有四样本、书架、Tauri 产品构建与 benchmark 回归均通过。真实《数学及其历史》的旧阅读能力已经通过实际 WebView2 runner；新消息 UI 的完整人工闭环留给用户试用验收，因此本 change 暂不标记 `implemented`。

2026-08-05 用户首次按普通 `--epub` 产品入口试用时发现 `invalid-manifest`。根因是消息模式在 `session.open()` 加载 manifest 前调用 `session.describe()`；原有 `--verify-import` 会禁用消息模式，因而未覆盖该顺序。消息会话现改为在阅读会话打开后创建，Tauri 验收同时增加普通 `--epub` 启动并断言阅读页进入 `pass`，不再只验证导入探针。

## Review

- Blocking：两轮独立 Standards/Spec review 发现的 presentation 只存不读、CSS 网络函数/转义可绕过和显示端静默改写均已修复，最终 Standards 复核无 blocking。当前只剩规格本身要求的真实《数学及其历史》消息 UI 完整闭环与重启恢复验收，不能用本地接口和 build 证据替代。
- Non-blocking：打开对话会从全屏笔记页回到阅读浮层；这是当前阅读页优先的交互，但“同页关系回顾”的最终设计可在用户试用后调整。`write.rs` 与 `legacy.rs` 有相似的内部写入形状；在第二种迁移或真实漂移出现前不增加 helper。
- Out-of-scope：备份、加密、同步、AI、富文本、附件和通用聊天仍按 Non-Goals 暂缓。参考地图是用户单独批准的流程中断，已在本 change 补记范围，不属于未授权产品扩张。

## Evidence And Residual Risks

- Windows 本地：`scripts/check-message-reading.ps1` 通过 14 个消息 interface 集成测试、历史呈现参数单元检查、Svelte check/build 与 Tauri/host 测试；`scripts/check-backend.ps1` 的 fmt、clippy、workspace test 和 doc 全部通过。
- Windows 真实 WebView2：`scripts/check-reader-samples.ps1` 四样本明暗、真实输入、持久化和跨 host 恢复通过；过长的单次 `agent-browser eval` 已拆成两个阶段，不再触发默认 25 秒超时或 daemon busy。
- Windows 真实 Tauri/本地：`scripts/check-library-shelf.ps1` 与 `scripts/check-tauri-reader.ps1` 通过。书架原生就绪条件改为稳定根节点，不再错误假设用户书架为空；Tauri 检查现同时覆盖普通 `--epub` 消息模式启动和 `--verify-import`。
- 性能：基准 `1785859567155-20612` 的 cold start / first stable / hot open / page turn / font reflow P95 分别为 739.307 / 148.700 / 27.300 / 31.900 / 41.600ms，低于 2000 / 750 / 120 / 50 / 150ms 门槛；没有同时间旧代码对照，不能归因性能变化。
- 修复后基准 `1785889633788-21736` 的上述 P95 分别为 724.373 / 133.500 / 21.500 / 6.700 / 41.700ms，仍低于固定门槛；该轮用于回归，不用于性能归因。
- 残余：尚未由用户在真实 Tauri 消息 UI 完成“选择至导出”的全链路，也未验证关闭应用后的消息 UI 恢复；当前证据不能替代该真实目标验收。
- 兼容性：若曾在本次未交付的中间提交上手工创建消息，旧快照可能仍含未冻结的 system theme、额外 presentation 字段或 reader CSS 子资源，严格显示会拒绝这些开发期记录；正式实现不为未发布中间格式增加迁移。

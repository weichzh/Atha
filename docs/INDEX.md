# 文档索引

先读 `docs/ACTIVE.md`。只有 `ACTIVE` 信息不足时才使用本索引。

## 工作流入口

- `README.md`：面向人的稳定仓库入口，只指向权威项目记忆。
- `AGENTS.md`：仓库级启动顺序、权威顺序和代码门禁。
- `docs/ACTIVE.md`：当前状态和下一动作。
- `docs/workflow/PROTOCOL.md`：完整文档协作协议。
- `.agents/skills/project-workflow/SKILL.md`：项目工作流技能。
- `docs/agents/issue-tracker.md`：工程技能使用的 GitHub issue tracker 约定。
- `docs/agents/triage-labels.md`：工程技能使用的默认 triage 标签映射。
- `docs/agents/domain.md`：工程技能读取领域文档和 ADR 的 single-context 约定。

## 当前里程碑

- 暂无；M2 已完成，下一里程碑尚未选择。
- `docs/milestones/M1-windows-backend-foundation.md`
  - 状态：`completed`。
  - 作用：记录正式 Cargo workspace 和最小后端 crate 的初始化结果。
  - 读取时机：审阅工程基线。

## 产品定义

- `docs/product/OVERVIEW.md`
  - 作用：定义 Atha 要解决的问题、核心阅读与共读体验、长期边界和验收原则。
  - 读取时机：讨论产品方向，或为规格和计划选择实现切片。

## 当前规格

- `docs/specs/SPEC-0002-html-paged-reader-slice.md`
  - 状态：`accepted`；三样本与夜间模式扩展已复审。
  - 作用：定义本地 XHTML、公式缩放和无行裁切分页的首个行为边界。
- `docs/specs/SPEC-0001-windows-backend-foundation.md`
  - 状态：`accepted`。
  - 作用：定义 M1 的范围、行为、验收标准和边界情况。

## 当前计划

- `docs/plans/PLAN-0002-html-paged-reader-slice.md`
  - 状态：`implemented`；本轮三样本与夜间模式扩展已复审并验收。
  - 作用：定义首个阅读切片的技术预检、实现步骤和验收。
- `docs/plans/PLAN-0001-windows-backend-foundation.md`
  - 状态：`implemented`。
  - 作用：定义 M1 的逐文件实现、验证和回滚。

## 已完成里程碑

- `docs/milestones/M2-html-reader-core-foundation.md`
  - 作用：建立本地 XHTML 的首个分页阅读切片、三样本验收与系统夜间模式。
- `docs/milestones/M0-document-workflow.md`
  - 作用：记录项目工作流和既有项目记忆迁移的完成状态。

## 架构

- `docs/architecture/OVERVIEW.md`
  - 作用：系统分层、长期边界和架构入口。
  - 读取时机：任务影响领域、存储、平台或模块边界。
- `docs/architecture/READER-CORE.md`
  - 作用：定义 HTML 阅读内核、样式覆盖、安全边界与性能原则。
  - 读取时机：渲染、缓存、样式、外部资源或阅读位置相关任务。
- `docs/architecture/MESSAGE-READING.md`
  - 作用：定义消息式摘录、引用存档与未来 AI 书友的边界。
  - 读取时机：消息、引用、搜索、存储或共读相关任务。

## 已接受决策

- `docs/decisions/ADR-0003-webview2-reader-host.md`
  - 作用：固定 Windows 阅读 host、自定义资源协议、安全边界、公式倍率和分页模型。
  - 读取时机：实现或审阅 HTML 阅读切片。

- `docs/decisions/ADR-0001-windows-backend-first.md`
  - 作用：确定 Windows 当前唯一实施平台、后端先于前端以及 RsProxy。
  - 读取时机：平台、顺序、工具链或前后端边界发生疑问。
- `docs/decisions/ADR-0002-sqlite-and-migrations.md`
  - 作用：固定 M2 的随包 SQLite 依赖和顺序迁移政策。
  - 读取时机：数据库依赖、连接初始化、迁移或 SQLite 升级。

## 代码库记忆

- `docs/codebase/MAP.md`
  - 作用：现有目录、已实现实验、验证证据和缺口。
- `docs/codebase/DATABASE.md`
  - 作用：P0 SQLite schema、不变量和未决项。

## 路线图

- `docs/roadmap/ROADMAP.md`
  - 作用：M0 至 M4 的严格顺序和暂缓范围。
  - 读取时机：选择或划分下一里程碑。

## 评审

- `docs/reviews/REVIEW-0003-html-paged-reader-slice.md`
  - 作用：M2 阅读切片、公式反馈修复和 Windows 本地验收结果。
- `docs/reviews/REVIEW-0001-doc-workflow-bootstrap.md`
  - 作用：M0 工作流初始化和迁移结果。
- `docs/reviews/REVIEW-0002-windows-backend-foundation.md`
  - 作用：M1 工程初始化、计划符合性和本地验证结果。

## 历史研究

- `docs/studies/ARCHIVE-0001-mobile-architecture-v0.1.md`
  - 作用：记录已失效的移动端 v0.1 提案及仍可复用结论。
  - 权威性：不权威；发生冲突时以 accepted 决策和当前架构为准。

## 模板

- `docs/templates/spec.md`：规格和自审。
- `docs/templates/plan.md`：实施计划和交叉审阅。
- `docs/templates/review.md`：实施评审。
- `docs/templates/decision.md`：架构决策。
- `docs/templates/milestone.md`：里程碑。

## 未来文档位置

- `docs/specs/`：规格；
- `docs/plans/`：实施计划；
- `docs/reviews/`：评审；
- `docs/decisions/`：已接受决策；
- `docs/discussions/`：非权威讨论；
- `docs/codebase/`：代码地图和 schema；
- `docs/studies/`：历史来源与研究。

## 权威顺序

1. `docs/ACTIVE.md`；
2. 当前里程碑；
3. accepted 规格；
4. accepted 决策；
5. 架构；
6. 代码地图和 schema；
7. 路线图；
8. 评审；
9. 讨论和历史研究。

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

- `docs/milestones/M1-windows-backend-foundation.md`
  - 状态：`completed`。
  - 作用：记录正式 Cargo workspace 和最小后端 crate 的初始化结果。
  - 读取时机：开始 M2 或审阅工程基线。

## 当前规格

- `docs/specs/SPEC-0001-windows-backend-foundation.md`
  - 状态：`accepted`。
  - 作用：定义 M1 的范围、行为、验收标准和边界情况。

## 当前计划

- `docs/plans/PLAN-0001-windows-backend-foundation.md`
  - 状态：`implemented`。
  - 作用：定义 M1 的逐文件实现、验证和回滚。

## 已完成里程碑

- `docs/milestones/M0-document-workflow.md`
  - 作用：记录项目工作流和既有项目记忆迁移的完成状态。

## 架构

- `docs/architecture/OVERVIEW.md`
  - 作用：当前产品方向、范围、质量优先级和后端边界。
  - 读取时机：任务影响领域、存储、平台或模块边界。

## 已接受决策

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

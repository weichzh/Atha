# M0：建立文档驱动项目工作流

## 状态

completed

## 日期

- 开始：2026-08-01
- 目标完成：2026-08-01
- 完成：2026-08-01

## 目标

建立可由后续 Codex 会话重复执行的最小项目工作流，并把旧移动端提案迁移为当前 Windows 后端优先的权威项目记忆。

## 范围

- 根 `AGENTS.md`；
- `ACTIVE`、`INDEX` 和协议；
- 规格、计划、评审、决策和里程碑模板；
- 项目工作流技能；
- 文档同步和长度守卫；
- 架构、决策、代码地图、数据库基线和路线图；
- 旧提案的历史归档说明。

## 非目标

- 创建根 Cargo workspace；
- 修改 P0 或生产代码；
- 选择正式后端依赖；
- 实现后端用例；
- 创建 Windows 或移动端前端。

## 退出条件

- [x] Codex 启动顺序为 `ACTIVE` 优先；
- [x] 生产代码受规格、计划和交叉审阅门禁约束；
- [x] 旧提案已迁移且不再作为当前权威来源；
- [x] 当前平台和实施顺序有 accepted ADR；
- [x] 现有代码、数据库与验证基线有独立文档；
- [x] `scripts/doc_guard.py` 通过；
- [x] `scripts/doc_length_check.py` 通过；
- [x] 中文 Markdown 排版检查通过；
- [x] M0 形成独立提交且工作树范围清楚。

## 活跃文档

- 决策：`docs/decisions/ADR-0001-windows-backend-first.md`
- 评审：`docs/reviews/REVIEW-0001-doc-workflow-bootstrap.md`
- 规格：docs-only bootstrap 不要求。
- 计划：由 bootstrap 技能和当前用户指令定义。

## 风险

- 守卫脚本已覆盖未来 `backend/`、现有 `p0/` 和 `scripts/`。
- RsProxy 项目配置属于本次仓库初始化，不包含依赖或业务行为。
- 工作流过重会阻塞轻量任务；本规则只对生产行为、架构、数据和测试变更启用完整门禁。

## 说明

M0 只建立工作方式。后续生产里程碑必须独立建立规格和计划，不得直接进入实现。

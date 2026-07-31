# ACTIVE

## 当前模式

discussion

## 当前里程碑

- 文件：`docs/milestones/M0-current.md`
- 目标：建立并验证文档驱动的项目工作流。
- 状态：completed

## 当前任务

- 任务：`DOCFLOW-0001`
- 类型：已完成的文档初始化与既有项目记忆迁移。
- 规格：该 docs-only bootstrap 不要求规格。
- 计划：用户已要求按最佳实践分步初始化。
- 评审：`docs/reviews/REVIEW-0001-doc-workflow-bootstrap.md`
- 分支：`main`

## 允许动作

- [x] 讨论；
- [x] 修改文档和工作流脚本；
- [ ] 修改测试；
- [ ] 修改生产代码；
- [ ] 推送或发布。

在新里程碑、accepted 规格、accepted 计划、交叉审阅和用户批准全部存在前，生产代码保持冻结。

## 当前状态

- 已完成：Git 初始化、P0 FFI 对照、P0 SQLite/FTS5/Outbox 对照。
- 已完成：项目工作流脚手架和既有项目记忆迁移。
- 已完成：Windows 后端优先 ADR、架构、代码地图、数据库基线和路线图。
- 已完成：RsProxy 项目配置及用户级 rustup 镜像切换。
- 进行中：无。
- 阻塞：无。
- 风险：正式后端工程仍未初始化，下一阶段必须先完成规格与计划。

## 当前权威文档

- `docs/architecture/OVERVIEW.md`
- `docs/decisions/ADR-0001-windows-backend-first.md`
- `docs/codebase/MAP.md`
- `docs/codebase/DATABASE.md`
- `docs/roadmap/ROADMAP.md`
- `docs/milestones/M0-current.md`

## 下一会话所需上下文

依次只读：

1. `AGENTS.md`；
2. `docs/ACTIVE.md`；
3. `docs/roadmap/ROADMAP.md`；
4. 创建 M1 时再读架构、代码地图和数据库基线。

## 检查

- `python scripts/doc_guard.py`：通过；
- `python scripts/doc_length_check.py`：通过；
- 中文 Markdown `autocorrect --fix`：完成；
- 中文 Markdown `autocorrect --lint`：通过；
- `pwsh -NoProfile -File scripts/check-p0-ffi.ps1`：CTest 1/1、Rust 2/2 通过；
- `git diff --check`：通过。

## 本次触碰文档

- `AGENTS.md`
- `README.md`
- `.agents/skills/project-workflow/SKILL.md`
- `docs/ACTIVE.md`
- `docs/INDEX.md`
- `docs/workflow/PROTOCOL.md`
- `docs/templates/`
- `docs/architecture/OVERVIEW.md`
- `docs/decisions/ADR-0001-windows-backend-first.md`
- `docs/codebase/MAP.md`
- `docs/codebase/DATABASE.md`
- `docs/roadmap/ROADMAP.md`
- `docs/milestones/M0-current.md`
- `docs/reviews/REVIEW-0001-doc-workflow-bootstrap.md`
- `docs/studies/ARCHIVE-0001-mobile-architecture-v0.1.md`

## 本次触碰代码

无生产后端代码。新增工作流守卫脚本；`.cargo/config.toml` 和 `scripts/check-p0-ffi.ps1` 已切换到 RsProxy。

## 下一动作

提交 M0 初始化检查点。下一次生产工作先创建 M1 里程碑和后端工程初始化规格，不直接写代码。

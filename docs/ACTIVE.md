# ACTIVE

## 当前模式

specification

## 当前里程碑

- 文件：`docs/milestones/M1-windows-backend-foundation.md`
- 目标：建立正式根 Cargo workspace、最小后端 crate 和单一验证入口。
- 状态：active

## 当前任务

- 任务：`BACKEND-INIT-0001`
- 类型：Windows 后端工程初始化规格。
- 规格：`docs/specs/SPEC-0001-windows-backend-foundation.md`，状态 `self-reviewed`。
- 计划：规格被用户接受后创建。
- 评审：计划完成后由独立 reviewer 或子 agent 交叉审阅。
- 分支：`main`

## 允许动作

- [x] 讨论；
- [x] 修改规格和相关项目文档；
- [ ] 修改测试；
- [ ] 修改生产代码；
- [ ] 推送或发布。

在规格被接受、计划被接受、交叉审阅通过和用户批准实施前，生产代码保持冻结。

## 当前状态

- 已完成：M0 工作流初始化，提交 `fc104e0`。
- 已完成：M1 里程碑和工程初始化规格草案。
- 已完成：规格自审；正式 workspace 限定为一个零依赖后端 crate。
- 进行中：等待用户确认 `SPEC-0001`。
- 阻塞：无技术阻塞；规格确认是下一道流程门禁。
- 风险：正式后端尚未初始化，不能把 P0 实验继续扩写为生产代码。

## 当前权威文档

- `docs/milestones/M1-windows-backend-foundation.md`
- `docs/specs/SPEC-0001-windows-backend-foundation.md`
- `docs/decisions/ADR-0001-windows-backend-first.md`
- `docs/architecture/OVERVIEW.md`
- `docs/codebase/MAP.md`
- `docs/codebase/DATABASE.md`
- `docs/roadmap/ROADMAP.md`

## 下一会话所需上下文

依次只读：

1. `AGENTS.md`；
2. `docs/ACTIVE.md`；
3. `docs/milestones/M1-windows-backend-foundation.md`；
4. `docs/specs/SPEC-0001-windows-backend-foundation.md`。

## 检查

- M1 规格自审：通过；
- 中文 Markdown `autocorrect --fix`：完成；
- 项目 Markdown formatter：未配置；
- 中文 Markdown `autocorrect --lint`：通过；
- `python scripts/doc_guard.py`：通过；
- `python scripts/doc_length_check.py`：通过；
- `git diff --check`：通过。

## 本次触碰文档

- `docs/ACTIVE.md`
- `docs/INDEX.md`
- `docs/decisions/ADR-0001-windows-backend-first.md`
- `docs/roadmap/ROADMAP.md`
- `docs/milestones/M0-document-workflow.md`
- `docs/milestones/M1-windows-backend-foundation.md`
- `docs/specs/SPEC-0001-windows-backend-foundation.md`

## 本次触碰代码

无。

## 下一动作

请用户审阅并确认 `SPEC-0001`。接受后进入 planning 模式，起草逐文件实施计划并执行独立交叉审阅；门禁完成前不初始化后端代码。

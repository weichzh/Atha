# Atha 工作流架构路由

## Status

accepted

## Problem

软件架构规范已经定义风险驱动、质量场景、候选比较和证据要求，但 Atha 当前 workflow 尚未明确何时触发这些要求，也没有把项目 activity、预期失败和检查顺序收敛为稳定规则。2026-08-07 的全局 workflow 基准包含 678 次运行，全部来自 Atha；其中有 22 次 adopted dirty claims、141 次 command failed、10 次失败等待超过 120 秒，以及 57 种 activity（30 种只出现一次）。这些数据足以优化 Atha 项目适配层，但不足以修改全局 `project-workflow`。

## Scope

- 在 Atha 工作流契约中固定已有 activity 词汇，任务细节由 scope 和真实命令表达；
- 规定预期拒绝类探针的成功 / 失败语义，并按成本从低到高组织检查；
- 约束 claim、adopt、handoff 和最终 gate 的使用；
- 为新 change 增加显式 Architecture Impact 分类，并由 `doc_guard.py` 对可交付代码变更执行最小校验；
- 把 `doc_guard.py` 自检加入 `workflow-self-check` gate；
- 保持全局 CLI、产品架构和现有正式产品 runner 不变。

## Non-Goals

- 不修改用户级 `project-workflow` skill、CLI、状态格式或全局日志；
- 不把 141 次非零退出都解释为流程缺陷，也不隐藏真正失败；
- 不删除 `Measure-Workflow.ps1`，除非后续专项审计证明全局日志已覆盖其独有价值；
- 不在本 change 中审计或重构 Atha 产品架构；
- 不追溯改写已关闭 change。

## Architecture Impact

none

本 change 只改变项目工程治理与文档门禁，不改变 Atha 产品的 Module、Interface、Seam、Adapter、运行拓扑、数据语义或信任模型。

## Acceptance Criteria

- [x] `docs/agents/workflow.md` 定义稳定 activity 词汇、低成本优先检查、预期拒绝语义和 claim 纪律；
- [x] `docs/workflow/PROTOCOL.md` 明确哪些 change 必须声明 Architecture Impact，以及 `present` 时的最小设计输入；
- [x] `docs/templates/change.md` 提供 Architecture Impact 模板；
- [x] `doc_guard.py` 在代码变更时拒绝缺少或使用未知 Architecture Impact 值的活动 change；
- [x] `doc_guard.py --self-check` 覆盖 `none`、`present`、缺失和未知值；
- [x] `workflow-self-check` 同时运行本地日志自检和文档门禁自检；
- [ ] 中文 Markdown 排版、docs gate、workflow-self-check、diff 检查和独立 review 通过。

## Files And Steps

1. 更新工作流契约、生命周期协议和 change 模板。
2. 为 `doc_guard.py` 增加 Architecture Impact 解析与自检。
3. 更新 `ACTIVE.md`，运行排版和目标检查。
4. 提交后运行两个 required gate，并完成独立 Standards / Spec review。

## Checks

- `python scripts/doc_guard.py --self-check`；
- `python scripts/doc_guard.py`；
- `pwsh -NoProfile -File scripts/Measure-Workflow.ps1 -Action SelfCheck`；
- `autocorrect --fix` 与 `autocorrect --lint` 仅作用于本次中文 Markdown；
- `python scripts/doc_length_check.py`；
- `git diff --check`；
- `project_workflow.py station atha-workflow-architecture-routing --activity verification --gate docs`；
- `project_workflow.py station atha-workflow-architecture-routing --activity verification --gate workflow-self-check`。

## Rollback

回退本次协议、模板和 `doc_guard.py` 变更即可；不涉及产品代码、依赖、数据或外部系统。

## Approval

用户于 2026-08-07 明确要求按已区分的流程优化与 Atha 架构重设计方案设定计划并开始实现，已批准本 change 的项目 workflow 范围。

## Result

- Atha 新工站只使用八个稳定 activity，具体 red/green、模块和重跑信息保留在 scope 或命令；
- 项目协议明确低成本预检、目标检查、required gate 和真实目标 / benchmark 的递进顺序，并规范预期拒绝类检查的退出语义；
- change 模板新增 `Architecture Impact: none|present`；`present` 时必须记录驱动因素、受影响结构、候选权衡和证据 / ADR；
- `doc_guard.py` 将 Architecture Impact 纳入活动 change 校验，并提供无依赖的 `--self-check`；
- `workflow-self-check` gate 同时运行本地日志自检与文档门禁自检；全局 `project-workflow` 未修改。

## Review

- Blocking: 待 review。
- Non-blocking: 待 review。
- Out-of-scope: 待 review。

## Evidence And Residual Risks

- 测量证据：2026-08-07 的 `project_workflow.py benchmark --all` 共 678 次运行、1 个仓库；22 次 adopted dirty claims、141 次 command failed、10 次 failed wait over 120s、57 种 activity 和 30 种单次 activity；
- 本地静态证据：`doc_guard.py --self-check`、正常 `doc_guard.py`、文档长度检查、workflow log 自检、Markdown lint 和 diff 检查通过；
- 残余风险：Architecture Impact 只机械校验分类值，`present` 内容质量仍由独立 review 和架构规范判断；activity 词汇由项目契约约束，但全局 CLI 当前不读取项目级枚举；
- 当前只有 Atha 一个仓库样本，任何全局 CLI 修改仍需至少两个仓库出现同类摩擦。`Measure-Workflow.ps1` 的去留仍待比较其独有信号后决定。

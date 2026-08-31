---
description: 用现有 change、工站和检查入口建立可停止、可追溯的迭代修复闭环。
---

# 迭代修复工作流

## Status

accepted

## Problem

Atha 已有事实所有者、task claim、工站、gate 和独立 review，但高成本验证失败后尚未统一记录本轮发现、最小修复、实际验证、剩余差异和停止原因。历史工作流基准可提示重复失败与长等待，却不能直接解释根因；若只靠终端文本和下一轮人工回忆，容易重复运行昂贵门禁，或把自动化证据越界解释为真实目标验收。

## Scope

- 在现有 `audit`、`change`、工站和 review 语义内定义 `Review -> Repair -> Validate` 闭环；
- 为 change 增加单次或迭代模式，以及迭代上限、差异停滞和人工 / 真实目标停止条件；
- 规定每轮只保存结构化摘要，不复制完整日志、源码、私密样本或设备信息；
- 用文档守卫校验新 change 声明合法的迭代模式；
- 在官方参考地图登记本次采用的 Codex 工作流资料。

## Non-Goals

- 不新增 `GOALS.md`、`PLANS.md`、`PROMPTS.md`、`harness/`、第二套 build log 或通用修复服务；
- 不自动修改全局 `project-workflow`，不从历史失败计数直接推导规则；
- 不让模型判断替代确定性测试、真实 GUI、真机或人工体验验收；
- 不改变产品、架构、数据、依赖、发布或外部写入边界。

## Architecture Impact

none

## Acceptance Criteria

- `WORKFLOW-LOOP-01`：协议明确区分只读 Review、最小 Repair 和实际 Validate，并要求失败反馈进入下一轮 `remaining_delta`；
- `WORKFLOW-LOOP-02`：迭代在验证通过、达到上限、剩余差异停滞或需要人工 / 真实目标决定时停止；
- `WORKFLOW-LOOP-03`：change 模板要求声明 `single-pass` 或 `iterative`，文档守卫拒绝缺失或未知值；
- `WORKFLOW-LOOP-04`：结构化摘要复用 change、工站日志与既有忽略证据目录，不建立平行事实源；
- `WORKFLOW-LOOP-05`：工作流自检、文档 gate、中文 Markdown lint 和独立复审通过。

## Files And Steps

1. 更新项目工作流契约和 change 生命周期，明确闭环阶段、每轮摘要与停止条件。
2. 更新 change 模板和 `doc_guard.py`，以最小解析检查迭代模式。
3. 把两篇 OpenAI 官方资料加入参考地图，说明只采用原则、不复制示例目录结构。
4. 运行红 / 绿自检、required gate 和独立 Standards / Spec 复审。

## Checks

- `mise exec -- python3 scripts/doc_guard.py --self-check`；
- `bash scripts/check-workflow.sh`；
- `autocorrect --fix/--lint` 仅针对本次中文 Markdown；
- `git diff --check`。

## Iteration Control

- Mode: `iterative`
- Maximum repair iterations: `2`
- Unchanged-delta limit: `1`
- Trigger: 首轮独立复审发现文档守卫可放行纯文档 change 与重复 Mode，继续最小修复。
- Human stop: contract digest 无法批准、规则所有权不清或机械检查需要扩大到产品 / 架构范围。
- Per-iteration receipt: change 的 `Result` 与 `Evidence And Residual Risks`，详细命令结果仍由 `project-workflow` 工站保存。

## Rollback

回退本次文档、模板和 `doc_guard.py` 修改，并重新批准恢复后的工作流 contract digest；不涉及产品数据或外部系统。

## Approval

用户在阅读两篇官方资料与 Atha 对照审计后，于 2026-08-31 明确要求“现在帮我更新流程”。

## Result

### Iteration 1

- Iteration: `1`
- Review findings: 独立 Standards / Spec 复审发现纯文档 active change 可绕过校验、同一区段重复 `Mode` 会被接受，以及生命周期语义在两份文档重复。
- Repair: 主入口先校验所有 active change；`Mode` 改为恰好一个；迭代生命周期归 `PROTOCOL.md`，工站文档只保留映射。
- Validation cases: 红探针确认未知 mode 曾以零退出，重复 mode self-check 曾失败；修复后未知 mode 被拒绝，self-check、工作流检查和文档 lint 通过。
- Remaining delta: 重复 `Iteration Control` 区段仍可能隐藏第二个 mode；最终复审和 required gate 尚未完成。
- Evidence level: 静态与本地机械检查。
- Stop reason: 继续处理独立复审的 blocking 发现。

### Iteration 2

- Iteration: `2`
- Review findings: 两路复审均确认重复 `Iteration Control` 区段仍会被放行。
- Repair: 解析器要求全文恰好一个 `Iteration Control` 区段，并增加对应 self-check 断言。
- Validation cases: 新断言先红，修复后 `doc_guard.py --self-check` 通过；最终 required gate 待候选提交后执行。
- Remaining delta: 最终独立复审、required gate 与关闭收据。
- Evidence level: 静态与本地机械检查。
- Stop reason: 达到最大修复轮数；只执行复审、验证和收尾，不再扩大实现。

## Review

- Blocking: 前两轮复审发现均已进入对应 Repair；最终复审待执行。
- Non-blocking: 无。
- Out-of-scope: 无。

## Evidence And Residual Risks

- 已覆盖：模式缺失、未知值、同一区段重复值、重复区段，以及纯文档 active change 的主入口拒绝。
- 证据等级：静态代码复审与本地机械检查；本次不改变产品行为，无需 GUI、真机或生产等价验收。
- 待完成：最终独立复审、required gate 和 task 关闭。

---
description: 按现有消融证据重构 UI/UX 产品基线的范围、验收与证据边界。
---

# UI/UX 文档证据重构

## Status

implemented

## Problem

`docs/product/OVERVIEW.md` 当前把产品选择、不可违反的硬门、设计比较协议、已实现布局和仍待验证的审美判断写在同一层级。UI 约束消融已经证明，这些主张的证据强度不同：安全与可操作问题必须由交互探针发现；响应式投影和作用域文案有当前夹具下的直接证据；渐进披露与消息视觉语法仍随评价前提变化。继续使用同一种确定语气，会让后续开发把价值选择误认为行业定论，或把本地原型观察扩大为跨平台结论。

## Scope

- 只重构 `docs/product/OVERVIEW.md` 的 UI/UX 决策基线，把主张区分为产品公理、硬门、已批准的体验合同、本地证据结论和未决设计假设。
- 保留 Atha 的产品身份、消息语义、安全边界和既有响应式方向，不修改生产代码、数据模型、架构、依赖或路线图。
- 用现有 `artifacts/local/experiments/ui-ablation-20260904/report.md` 与 `docs/changes/ui-constraint-ablation.md` 限定证据强度；不重新运行视觉实验，也不把 Chromium 原型升级为真实 Tauri、Windows、Android 或用户研究证据。
- 接管此前未提交的 `docs/product/OVERVIEW.md` 与 `docs/ACTIVE.md` 改动，只调整本 change 所述段落，不回退其他任务留下的内容。

## Non-goals

- 不决定右上角设置入口是否错误，不决定常驻工具栏与渐进披露的最终形式。
- 不决定时间线、气泡或混合消息语法的最终视觉方案。
- 不把 `V1-V9` 词典序描述为已经验证的唯一最优价值排序，也不新增第二套设计规范或评分框架。
- 不修改 `docs/architecture/`、`docs/codebase/`、`docs/roadmap/` 或实验原型。

## Architecture Impact

none

本次只改变产品事实所有者中主张的分类、状态与证据口径，不改变 Module、Interface、Seam、Adapter、数据语义、信任模型、运行拓扑、依赖或长期质量门槛。

## Acceptance Criteria

- 产品文档明确区分：`P` 产品公理、`G` 硬门、`C` 已批准体验合同、`E` 本地证据结论、`H` 未决假设。
- `G0/G1` 仍先于审美比较，并明确截图不能替代删除取消、焦点、键盘、触控目标与状态可达性的可执行检查。
- A/B/C 合同明确 `A` 为正文、`B` 为笔记 / 对话列表、`C` 为当前消息线程；横屏保留 `A+B`、`B+C`、`A+B+C`，竖屏保留正文上的同一悬浮窗口及 `B -> C -> B` 两页，不把正文放在 `B` 与 `C` 之间。
- 设置问题拆成作用域、入口位置和首屏层级：当前证据只支持作用域必须可辨；入口位置与渐进披露仍为未决假设。
- 消息文档把真实原文、用户消息与回复 / 引用关系作为稳定语义，把时间线、气泡与混合布局保留为候选视觉语法；不伪造作者或 AI 身份。
- 文档公开当前证据只能支持的范围、可能推翻结论的事实和禁止声称项；`V1-V9` 只保留为本轮实验采用但尚未对照验证的比较顺序。
- 不新增独立设计规范文件，不修改允许清单之外的版本控制文件。

## Iteration Control

- Mode: `single-pass`

## Files And Steps

1. 重写 `docs/product/OVERVIEW.md` 的 UI/UX 决策基线，保留稳定产品目标，删除不同证据等级共享同一种确定语气的写法。
2. 更新 `docs/ACTIVE.md`，仅把当前执行指针和最小 Context Bundle 指向本 change 及其证据。
3. 对本次三个中文 Markdown 文件运行 `autocorrect --fix`、文档 gate、`autocorrect --lint` 与 `git diff --check`。
4. 由未参与正文修改的子 agent 做独立 Standards / Spec review；阻断发现修复后重新验证。

## Checks

- `autocorrect --fix docs/product/OVERVIEW.md docs/ACTIVE.md docs/changes/uiux-document-evidence-restructure.md`
- `bash scripts/check-docs.sh`
- `autocorrect --lint docs/product/OVERVIEW.md docs/ACTIVE.md docs/changes/uiux-document-evidence-restructure.md`
- `git diff --check -- docs/product/OVERVIEW.md docs/ACTIVE.md docs/changes/uiux-document-evidence-restructure.md`

## Result

- `docs/product/OVERVIEW.md` 已用 `P/G/C/E/H` 分开产品公理、不可补偿硬门、已批准体验合同、有范围的本地证据和未决假设，并为不同类型规定各自的最小收据。
- A/B/C 合同现在明确允许 `A | B`、`B | C`、`A | B | C`，禁止 `B | A | C`；竖屏或窄视口保留正文底层和同一悬浮窗口中的 `B → C → B` 两页。`800px` 与 `1440px` 被限定为当前实现参考值，不再冒充最优产品阈值。
- 设置被拆成已批准的作用域合同与未决的入口位置、首屏层级；消息被拆成稳定关系语义与未决的气泡、时间线或混合视觉语法。
- `V1-V9` 只解释本轮预注册实验，不再作为已经验证的永久价值排序；截图、夹具交互、真实平台和长期用户证据分别保留边界。
- 未修改生产界面、架构、数据、依赖或路线图。`autocorrect --fix`、`bash scripts/check-docs.sh`、`autocorrect --lint` 与定点 `git diff --check` 均通过。

## Review

- Standards：独立子 agent 发现 change 生命周期尚未收口，以及 `44 × 44 CSS px` 位于 `G1` 中可能被误读为 WCAG AA 统一阈值。正文已明确其为 Atha 项目目标，并只在 change 把动作登记为主要触控动作时单独形成该任务的失败条件；本节也改为按主张类型要求不同收据。更新 Result、Review 与 Status 后，该 Standards 阻断关闭。
- Spec：独立复核确认 `P/G/C/E/H`、A/B/C 顺序、竖屏两页、设置拆分、A6/A7 未决性、`V1-V9` 限定和 E1-E4 证据范围均满足 Acceptance Criteria，无阻断发现。
- Evidence：视觉证据复核确认正文没有把 Linux 本地 Vite / Chromium 原型升级为 Tauri / WebKitGTK、Windows、Android、真实用户或生产结论。`800px` 没有单独捕获、`1000px` 的部分 A3 观察主要记录在报告正文而非代表拼版，作为非阻断可追溯性边界保留。

## Evidence And Residual Risks

- 直接证据来自本地离线 Chromium 固定视口、交互探针和两份盲评记录；它能够区分当前夹具中的构图、状态可达性和文案问题，但不能证明跨平台外观、自然触摸、真实输入法、辅助技术、长期疲劳或普遍审美偏好。
- 微信读书、Readest 与 Telegram 只提供可复核的既有设计实例，不自动成为 Atha 的规范来源。
- 用户已经批准本轮文档重构；该批准确认产品价值与范围，不把尚未验证的设计假设提升为事实。

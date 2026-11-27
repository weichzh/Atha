# 采用 GitHub Flow

## Status

accepted

## Problem

Atha 需要明确新工作从哪个基线开分支、如何经 PR 合入，以及合并后仍待真实验收的任务怎么交接。本变更接续 [PR #3](https://github.com/weichzh/Atha/pull/3) 的 Issue 管理草案，将分支和 PR 作为日常开发路径，Issue 用于需要跨会话的交接。

## Scope

- 依据用户的最新决定，将 GitHub Flow 设为新开发的默认路径；保留 `audit`、`fast`、`change` 和现有仓库验证入口。
- 更新流程文档、Issue 和 PR 模板，将公开候选交到 PR #3；整理 [Issue #1](https://github.com/weichzh/Atha/issues/1) 和 [Issue #2](https://github.com/weichzh/Atha/issues/2) 的验收交接。

## Architecture Impact

none

## Acceptance Criteria

- 新会话能找到开分支、审查、验证、合并与收尾规则，并区分独立任务和依赖未公开提交的任务。
- 小修走分支与 PR；跨会话任务通过 Issue 交接候选提交、证据和下一步，待验收的 Issue 保持开放。
- PR 差异只含流程文档与模板；两条 Issue 都有可以接续的验收步骤。

## Files And Steps

1. 在 `AGENTS.md` 指向默认流程，在协作指南和 Issue 指南定义分支、PR 与交接。
2. 对齐 README、文档索引、路线图、代码地图和模板。
3. 检查公开差异，运行仓库流程自检，将候选交到 PR 审查；分别更新两个 Issue 的验收交接。

## Checks

- 只对本次触碰的中文 Markdown 执行 `autocorrect --fix`、适用格式化和 `autocorrect --lint`。
- 运行 `bash scripts/check-workflow.sh`（包含文档检查）和 `git diff --check`。
- 以 [Issue #1](https://github.com/weichzh/Atha/issues/1) 为输入，只读试走一次待验收阅读链路的交接。

## Iteration Control

- Mode: `single-pass`

## Result

入口文档和模板已统一采用 GitHub Flow；检查结果见 PR。

试走 [Issue #1](https://github.com/weichzh/Atha/issues/1)：10 月 17 日的交接记录提供了 44 项合成事件检查的结果与 GUI 实验步骤，但未标记受测提交。下一位 GUI 验收执行者先确认候选并记录提交；再用同一原创 EPUB、固定视口走完阅读、选区、笔记或对话及原文回跳。此轮交接试走核对了记录。

## Review

待在 PR #3 独立审查：核对范围、检查证据和 Issue 的关闭条件。

## Evidence And Residual Risks

候选只基于已公开的分支，保留公开 `main` 的产品规则与现有验收指针。本 PR 验证流程文档和模板；产品验收按 Issue #1 与 #2 的交接分别进行。

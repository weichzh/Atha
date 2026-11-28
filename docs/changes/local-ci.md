# 本地提审检查与发版验收

## Status

implemented

## Problem

推送前自动补跑的实验把检查、收据和 Git 推送耦合起来；重复运行不能修正过时的验收脚本。现有 Linux 阅读脚本也仍引用旧书架控件，不能把它的覆盖范围当作现在的验收结果。

## Scope

按用户批准，保留本机执行和现有仓库检查，撤掉实验 runner、收据机制和 Atha 专属推送钩子，不改全局 Markdown 钩子。借鉴 Readest：分开开发回归、提审检查和发版验收；修复当前 Linux 书架验收入口，不改产品行为、不运行私人样本、不操作手机、不发布。

## Architecture Impact

none

保持现有产品、平台和安全边界；仅改变仓库验证的编排和取证。

## Acceptance Criteria

- 推送不再触发实验 CI；提审时使用已有检查，在干净候选上按影响范围跑一遍，失败先修复。同一候选的有效证据不重跑。
- 纯文档差异不需要产品构建；提审能检查已提交差异，原来的工作区检查用法不变；适用的浏览器回归可使用锁定依赖。
- 真实 Linux WebView 使用隔离测试书库，对齐当前书架的筛选、视图、开书和返回；专项结果不冒充完整 Linux 或真机验收。
- PR 写清被测提交、环境和缺口；合并只针对已审查的候选；发版独立固定源码和包，并验证对应平台实际需要的操作。

## Files And Steps

在现有工作流与 PR 模板明确证据和时机。把当前书架验收接入现有 Linux WebDriver 入口，利用其原有隔离和公开样本。移除实验运行器和 Git 钩子；保留文档门和浏览器依赖。

## Checks

文档与流程门、提交差异正反例、现有 Linux 实际 WebView 链路，及相关的浏览器回归。

## Iteration Control

- Mode: `iterative`
- Maximum repair iterations: `3`
- Unchanged-delta limit: `1`
- Human stop: 真实设备、发布或私人资料需要独立授权。

## Result

- 原全量 Linux 入口：以当前源码构建应用，真实 WebView 启动后等不到旧 `.library-book-open`。这复现了选择器漂移，没有进入后续全量矩阵。
- 书架专项：在原隔离入口增加 `--shelf`，使用当前书架控件。在 GNOME Wayland / WebKitGTK 2.54.3 上用同一构建验证公开 FB2 与合成 TXT，导入、收藏筛选、窄窗列表排序、搜索、开书、返回状态与焦点及 AppLog 隐私通过。额外的私有公式或文件关联参数在启动前拒绝。
- 提审流程：用已有仓库检查和 PR 验证栏记录证据；Atha 推送钩子已关闭。实际 Git 夹具验证纯文档通过、提交中缺 change 被拒、获准的 fast 通过、补 change 后通过、脏候选被拒。生产前端的 Chromium 阅读回归通过。

## Review

保留原检查脚本的默认调用以及全局 Markdown 提交钩子；全量 Linux 和其下游保持原合同，不因专项通过而改变结果。合并时仍需独立审查实际 PR head。参照 [Readest 验收规则](https://raw.githubusercontent.com/readest/readest/main/apps/readest-app/.claude/rules/acceptance.md) 与 [发版 QA](https://raw.githubusercontent.com/readest/readest/main/apps/readest-app/.claude/skills/release-qa/SKILL.md)。

## Evidence And Residual Risks

本机日志与专项 JSON 在 `artifacts/local/audits/local-review/`。当前全量、AppImage 和公式验收受旧 Linux 入口阻塞；未运行真机和签名发布包。

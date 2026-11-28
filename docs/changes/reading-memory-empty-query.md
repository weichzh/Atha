# 打开阅读记忆时显示最近片段

## Status

accepted

## Problem

原生「阅读记忆」首次载入和清空搜索时发出空查询。原生拒绝空查询，显示读取失败；浏览器 demo 返回最近消息，未覆盖该真实差异。

## Scope

用户已批准独立修复：空查询及仅空白查询展示最近可见的阅读记忆，保留所有非空搜索规则。不更改数据模型、权限、真实书库或当前设备。

## Architecture Impact

present

仅明确既有 `reading_memory_search` 请求的空查询模式。它沿用资料库来源校验、消息可见性、根 Anchor 和现有响应；非空请求仍用原搜索路径。与新增一条只用于初次载入的 IPC 相比，复用接口无需为首次进入、清空或返回分别维护请求生命周期。以 MessageStore 集成测试、浏览器与 Linux WebView 原生路径验证，迁移与分发无变化。

## Acceptance Criteria

- 首次打开或清空搜索，按消息更新时间降序显示最多 200 条；同时间用消息身份稳定排序。只返回可见消息和有效根，空书库显示空态而非错误。
- 非空查询继续用现有短词、FTS、长度、排序和安全规则。最近列表、搜索失败重试和回跳互不改写。
- 用固定消息的隔离资料目录验证最近列表和清空恢复，记录真实运行层级。

## Files And Steps

补消息库的空查询分支及回归；同步页面空词标签和两种客户端的共同合同；在完整候选中以真实原生书架到阅读记忆复验。

## Checks

真实 SQLite 消息库、Svelte 和相关 Chromium 回归；Linux 实际 WebView 开放后验证首次打开及清空后恢复。

## Iteration Control

- Mode: `iterative`
- Maximum repair iterations: `3`
- Unchanged-delta limit: `1`
- Human stop: 不以私有样本或设备代替原创夹具，不发布软件。

## Result

真实 Linux 页面和现有 `InvalidInput` 集成测试共同定位到原生与 demo 的空值行为不一致。

## Review

核对空查询和正文搜索的分流、结果规模、删除过滤、排序和既有 IPC 来源校验。

## Evidence And Residual Risks

本机证据放在 `artifacts/local/`；Windows 和 Android 原生验收分别记录。

# Atha 0.1.1 发版

## Status

implemented

## Problem

正式开发基线已完成本地原生验收。首份 GitHub Release 需要统一版本，按独立验包流程核对待上传的实际制品。

## Scope

- 将已验收的基线随 `v0.1.1` 交付；发源码和 Linux x86_64 AppImage。Windows 和 Android 不随本次发包。
- 将 `0.1.x` 定为后续发版默认版本线；仅在用户明确要求时提高次版本或主版本。

## Architecture Impact

none

## Acceptance Criteria

- Rust workspace、前端和 Tauri 版本一致，Cargo 锁文件可用。
- 新包通过 Linux AppImage 验包，包含文件关联、实际阅读工作区及隔离数据验证；同一包文件可直接启动。
- PR 按实际候选检查并审查；标签指向合入 `main` 后与受测代码一致的提交。GitHub Release 上传验收的同一包。

## Files And Steps

统一版本配置和 workspace 锁文件；将长期版本规则写入[发版流程](../agents/workflow.md#发版单独验收)。基线改动与证据见[正式开发基线](repository-readiness.md)，关联 [Issue #1](https://github.com/weichzh/Atha/issues/1) 和 [Issue #2](https://github.com/weichzh/Atha/issues/2) 的跨平台验收继续保留。

## Checks

候选文档门、锁定依赖检查、实际 `check-reader-candidate.sh` 与同包直启书架。

## Iteration Control

- Mode: `single-pass`
- Maximum repair iterations: `1`
- Unchanged-delta limit: `1`
- Human stop: 候选未通过或平台证据不足时，不发布受影响的制品。

## Approval

用户批准提审并发布 0.1.1，同时明确后续维持 0.1.x。

## Result

首份正式版本以上一公开主线 `a4523be` 为基底；无上一公开标签。

候选 `1b779a6` 的版本均为 0.1.1，Linux 锁定依赖解析与候选文档门通过。由该候选构建的 `Atha_0.1.1_amd64.AppImage` 在 GNOME Wayland／WebKitGTK 通过完整包装器和同包直启书架。文件关联冷启、重复投递与普通启动均通过。原创 EPUB 在 360、1000 和 1600 CSS px 回到实际原文，排除同章错页；阅读记忆新库、首次进入及清空搜索、长单段回复和继续发送通过。独立内核诊断完成默认 220 次测量，产品日志隐私检查通过。发包使用这份原始制品，标签和状态以 GitHub Release 为准。

## Review

基线已处理独立审查发现，见[正式开发基线](repository-readiness.md)。发版元数据、发布范围和最终 head 在同一 PR 审查。

## Evidence And Residual Risks

本次无更早公开安装包可做覆盖升级验收；真实数据备份及恢复与跨平台证据继续按各自入口记录。

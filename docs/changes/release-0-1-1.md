# Atha 0.1.1 发版

## Status

accepted

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

## Review

先验包，再按 PR 实际 head 提审。

## Evidence And Residual Risks

本次无更早公开安装包可做覆盖升级验收；真实数据备份及恢复与跨平台证据继续按各自入口记录。

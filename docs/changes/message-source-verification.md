# 消息验证入口覆盖异步存储回归

## Status

accepted

## Problem

独立的 PowerShell 消息门没有运行已有的消息存储 Node 回归，并会在运行测试前查找已经迁移的 Svelte 文件。

## Scope

只恢复源检查入口：保留 Tauri 命令的注册和授权检查，把取消备份 / 恢复的源断言对齐当前资料库 adapter；将已有存储用例纳入消息门。不改变消息接口、资料库备份行为或权限。

## Architecture Impact

none

## Acceptance Criteria

- 以当前源代码通过消息与维护命令静态断言和取消状态检查。
- 同一次 Node 测试执行包含会话与存储测试，有失败则消息门退出非零。

## Files And Steps

`scripts/check-message-reading.ps1`。关联 [Issue #2](https://github.com/weichzh/Atha/issues/2)。

## Checks

PowerShell AST 解析并执行两个实际静态断言；单独运行脚本所调用的 Node 测试和 Rust 消息集成测试。

## Iteration Control

- Mode: `single-pass`

## Approval

按逐批交付剩余验证工具的授权迁移现有源测试。

## Result

Linux：PowerShell 解析与两个静态断言通过；Node 24.1.0 的会话与消息存储共 23 项通过；Rust 1.97.1 release 消息集成 24 项通过、1 项私有样本忽略。

## Review

实现内复核保留了既有消息、维护 IPC 的命令集合和权限检查，Node 退出码由现有 `Invoke-Checked` 传播。独立审查在集成交付时进行。

## Evidence And Residual Risks

未运行整条 PowerShell 门、原生 Windows 或真实书籍恢复。消息存储用例不替代重启后快照资产与原文往返验收。

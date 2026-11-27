---
description: 在 Unix 上同步快照资产目录后再提交数据库引用，并验证失败回滚与重试。
---

# 快照资产发布耐久性

## Status

implemented

## Problem

快照资产文件经同步并重命名后，目录项可能尚未同步，消息数据库便提交引用。失败后重试复用完整资产也必须经过相同发布屏障。

## Scope

按已批准的本地实现单独交付 MessageStore 的 Unix 目录同步、错误传播、文档和故障回归。消息、IPC、路径校验、数据库 schema 与非 Unix 的既有文件同步合同保持不变。

## Architecture Impact

present

- 目标：目录同步失败时保留旧数据库事实，不留下新引用；完整孤儿重试时再次同步。
- 接口：`messages/store.rs` 的普通资产写入与恢复共享同步屏障，`backup.rs` 保持先发布资产再提交活动库。
- 权衡：延续文件和数据库两阶段发布，避免迁移资产格式；真实断电或跨平台等价保证需要单独证据。
- 证据：Unix 定向故障注入、独立数据库观察与现有消息备份回归。

## Acceptance Criteria

- [x] 新资产和复用资产的目录同步在数据库提交前；报错沿用既有错误边界。
- [x] 创建、重选、失败重试、备份恢复和无资源对照的故障测试通过。
- [x] [PR #5](https://github.com/weichzh/Atha/pull/5) 已合入 `ec7ae8d`；真实书库和硬件验收继续在 [Issue #2](https://github.com/weichzh/Atha/issues/2) 跟进。

## Files And Steps

提取已审过的消息存储改动、同目录测试和数据库合同；按隔离仓库校验。Issue #2 保留其余验收。

## Checks

Unix 定向耐久性故障测试、消息读写和备份恢复测试、工作流和文档检查。

## Iteration Control

- Mode: `single-pass`

## Result

发布与恢复共用 Unix 目录同步屏障；同步失败沿用既有错误，保留原事实，重开可清理不受引用的完整孤儿。当前分支的隔离 Linux 检查：5 项目录同步故障回归通过；22 项消息读写与备份恢复通过，1 项既有真实样本测试跳过。文档和工作流自检通过。

## Review

代码与安全复审均覆盖交付候选 `e6529f2`。

## Evidence And Residual Risks

验证目录同步错误的调用顺序与数据可读性。Windows、真实掉电、首次创建整棵数据目录及恢复吞吐不属于本证据。

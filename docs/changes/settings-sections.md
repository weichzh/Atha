---
description: 桌面按组找设置，窄屏进入专注子页并返回同一位置。
---

# 可搜索的分组设置

## Status

implemented

## Problem

外观、书架、排版、词典与备份挤在同一长页。到达常用设置要反复滚动，窄屏尤其明显。

## Scope

从已获准拆分的移动体验改动移植设置分组、搜索与窄屏子页。保留当前外观、书架布局、全书排版重置、词典管理和备份/恢复的功能及确认。尚未接线的翻页和阅读手势不提前出现在设置中。

## Architecture Impact

none

使用当前应用的返回分发和既有数据适配器，不改 IPC 或备份格式。

## Acceptance Criteria

- [x] 桌面同时显示设置索引及当前组；窄屏显示简洁索引，打开一组后仅显示其内容。
- [x] 搜索命中分组与常用词，例如「字号」「词典」「备份」；空结果可清除。窄屏返回保留查询、滚动和触发焦点。
- [x] 实际变更的外观及书架偏好继续生效；词典和备份的原入口仍可达。返回先关闭确认，再退出子页。

## Files And Steps

在 `AppSettingsView.svelte` 增加分组入口和局部返回。保持现有设置内容与存储。由 [Issue #1](https://github.com/weichzh/Atha/issues/1) 跟踪阅读体验。

## Checks

Svelte、Vite、工作流与桌面/390px Chromium 回归，覆盖搜索、退回及已有设置操作。

## Iteration Control

- Mode: `single-pass`

## Result

桌面在选项旁显示当前设置，390px 按组选入子页。Chromium 验证了全角 MOBI 与常用设置关键词、清除空结果、返回保留查询与发起焦点、确认先关闭再退出，以及词典导入和备份入口可达；既有浅色/暗色和书架偏好设置仍按原路径生效。以已合并 Android 适配的 `9d6c5c6` 为基底，Svelte 检查、Vite 和完整 Chromium 通过；已查看桌面和窄屏实际画面。

## Review

分支只改设置导航、布局和对应回归，不改变适配器或数据格式。独立审查在 PR 中进行。

## Evidence And Residual Risks

原生 WebView 与真实资料恢复另行验收。

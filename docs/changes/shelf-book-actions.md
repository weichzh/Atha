---
description: 长按或右键选择书籍，在同一底栏管理单册或多册。
---

# 书架多选和批量操作

## Status

accepted

## Problem

每本书独立的封面菜单遮挡封面，整理多册时需要反复打开菜单。书架需要一个随选择数量变化的操作区。

## Scope

从已获准分批交付的阅读交互 V2 中提取书架多选。保留现有搜索、筛选、排序、布局、开书过渡与焦点恢复；继续用现有书籍适配器和确认边界。不在本批改书架布局、继续阅读区域、设置、系统栏或阅读控件。

## Architecture Impact

none

仅改变书架上的操作入口及选择状态，仍按现有接口逐册写入本地书库。

## Acceptance Criteria

- [ ] 长按、右键、键盘或可见的「选择」能进入同一模式；短按仍打开原书，滚动不误触。
- [ ] 浮动底栏随选择数量变化；保留原单册操作，多册可收藏、移出和删除，涉及移出或删除必须先确认。
- [ ] 搜索与筛选继续可用；范围变化不会把不可见书籍带入批量操作。取消选择或关闭确认保留书架位置。
- [ ] 批量写入遇到故障时，准确报告完成数量，停止后续写入。待恢复的删除沿用原有恢复流程。

## Files And Steps

在 `LibraryView.svelte` 接入选择和原生返回，沿用 `ConfirmDialog` 与书架适配器。补充隔离书库的浏览器用例；[Issue #1](https://github.com/weichzh/Atha/issues/1) 跟踪真实链路。

## Checks

Svelte、Vite、仓库工作流检查；桌面和 390px Chromium 复验三种入口、单/多册操作、确认取消、过滤与开书返回。

## Iteration Control

- Mode: `single-pass`
- 停止条件：新入口不误开书，现有书架操作与数据边界通过回归。

## Result

待实现与验证。

## Review

待独立审查。

## Evidence And Residual Risks

原交互 V2 候选在 Android 16 模拟器验证过长按多选；不沿用为拆分候选的实测结果。Windows 和实体手机另行验收。

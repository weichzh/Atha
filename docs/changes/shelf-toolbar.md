---
description: 把搜索与书架操作合在一行，直接切换排序。
---

# 紧凑书架工具栏

## Status

implemented

## Problem

窄屏的搜索、筛选、排序和导入堆成几行，首批封面被推离首屏。切换常用排序要先展开菜单。

## Scope

按已批准的 V2 书架改动收紧工具栏。搜索、筛选、视图切换、多选同排；导入移到标题旁；排序和收藏进入一条横向滑动栏。保留全部筛选维度、排序方向、选择范围、续读和开书/返回行为。

## Architecture Impact

none

沿用原有查询、筛选和书架会话格式。

## Acceptance Criteria

- [x] 窄屏搜索与书架操作不换行或溢出；导入仍可见。
- [x] 所有排序可直接触摸或用键盘切换；收藏快捷入口与原有筛选条件一致。
- [x] 清单／封面可互切；已有筛选、多选、最近阅读及开书返回通过回归。

## Files And Steps

修改 `LibraryView.svelte`，沿用书架数据投影及 Bits UI 完整筛选。更新 [Issue #1](https://github.com/weichzh/Atha/issues/1) 的交付证据。

## Checks

Svelte、Vite、文档、工作流、Chromium 桌面和窄屏交互与画面。

## Iteration Control

- Mode: `single-pass`

## Result

搜索、筛选、布局切换和多选共用一行；导入在书架标题旁。排序栏保留两种书名方向及其他排序；收藏快捷入口和完整筛选写入同一个条件。原生返回优先关闭弹层或多选，软键盘由宿主收起后，可再次返回清除搜索。

Chromium 实测 360px 工具栏同排，排序栏按内容横滑，键盘切换书名正倒序，收藏与完整筛选同步，保留导入和清单/封面入口。完整 Chromium 书架多选、续读和阅读往返通过；Svelte 与 Vite 通过，已查看窄屏和桌面画面。

分组设置合入后，保留其设置和词典确认用例，与书架用例共用同一浏览器入口。叠加卷页修复 `63d7efb` 的候选组合，Vite、完整 Chromium 和卷页专项均通过。

## Review

发布前确认仅替换工具栏入口，不删除原排序、筛选维度、书籍操作或会话字段。独立审查在 PR 中进行。

## Evidence And Residual Risks

浏览器使用隔离原创书库；原生 WebView 另行验收。

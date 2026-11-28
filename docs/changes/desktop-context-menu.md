---
description: 由 Atha 统一接管桌面应用内的右键菜单。
---

# 桌面右键菜单

## Status

implemented

## Problem

正文已拦截 WebView 菜单，书架空白等界面仍会显示后退、前进与重新加载。应用缺少统一的右键归属和文本编辑菜单。

## Scope

桌面空白处不弹菜单，书卡和文本编辑提供 Atha 操作，真正命中的正文选区保留现有操作。Android 长按、选区手柄和输入行为保持原状。

## Architecture Impact

present

- Design purpose: 应用壳收住 WebView 默认菜单，业务组件决定当前目标可做什么。
- Drivers / quality scenarios: 书架、阅读和输入之间切换都不露出浏览器菜单，编辑操作能读写系统纯文本。
- Modules / Interfaces / Seams / Adapters: 壳拥有桌面菜单，书架提供书卡动作；桌面主 WebView 通过 Tauri 官方剪贴板插件读写纯文本。
- Candidate and tradeoffs: 不开启 WebView 剪贴板权限或为全部平台启用插件。仅在桌面主窗授权文本读写，不开放图片、文件或 HTML 命令。
- Evidence / ADR / review trigger: Linux 实际 WebKitGTK 验证；若 Windows WebView2 出现绕过 DOM 的默认菜单，再处理对应平台的窗口拦截。

## Acceptance Criteria

- 在书架、记忆、设置及阅读空白处右键，不出现浏览器菜单，也不显示无关操作。
- 书卡右键提供针对本书的操作，初次右键不改变多选状态；键盘可打开菜单。
- 可编辑文本提供可用的剪切、复制、纯文本粘贴与全选；真实正文选区保留 Atha 操作。Android 不套用桌面菜单。

## Files And Steps

1. 由应用壳接管桌面右键，复用现有菜单组件的焦点和边界定位。
2. 接入书架、编辑命令和桌面主窗纯文本剪贴板。
3. 本地复验空白、书卡、正文、输入和草稿。

## Checks

Svelte 检查、应用构建、定向 Linux WebKitGTK 交互与文档门。不跑全量测试。

## Iteration Control

- Mode: `iterative`
- Maximum repair iterations: `3`
- Unchanged-delta limit: `1`
- Human stop: 需操作真实用户数据、扩大剪贴板权限或提审。
- Per-iteration receipt: 汇总到 Result，实机证据留在本机。

## Approval

用户批准 Atha 自己实现桌面菜单，覆盖应用各处。仅本地试用。

## Rollback

回退此切片即可移除应用壳菜单和桌面剪贴板权限，无数据迁移。

## Result

- 第 1 轮：Svelte 检查与构建通过；隔离 WebKitGTK 确认书架空白与书卡菜单，弹窗内粘贴未生效。发现桌面 capability 未加入显式启用列表，下一轮补齐后复验。
- 第 2 轮：补齐桌面主窗 capability 的启用列表。隔离 Linux WebKitGTK／X11 的真实鼠标检查确认书架、记忆、设置和阅读空白处不弹菜单；书卡只在主动选中后进入多选，键盘可打开并按 Esc 恢复焦点。搜索框的剪切、复制、粘贴、全选通过；弹窗和富文本草稿均能粘贴。正文在真实选区上保留操作，复制写入系统剪贴板。Linux 验收差异关闭。

## Review

- 对照本地 Readest：Linux 书卡用应用内菜单，Windows 通过应用定义的菜单项目调用原生控件。Atha 在两端共用应用菜单组件，利用同一套键盘和边缘定位逻辑，不复用浏览器菜单项。
- 弹窗内的菜单挂在同一个弹窗内，不把焦点带出编辑区。剪贴板只在用户执行对应操作时访问；异步剪切或粘贴完成前若焦点或选区已变，文本不再修改。书籍来源权限和 Android 行为保持原状。

## Evidence And Residual Risks

- Linux 日志与截图：`artifacts/local/desktop-context-menu/`。Svelte 检查、Vite 生产构建、锁定依赖的 Linux debug 构建通过。全量测试保持暂停；未实测 Windows WebView2 或 GNOME Wayland 的系统剪贴板。

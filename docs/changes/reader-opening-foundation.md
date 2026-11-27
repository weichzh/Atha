---
description: 将开书动效所需的新阅读器、书架适配与宿主接口独立交付。
---

# 开书动效的阅读器前置

## Status

implemented

## Problem

原主线阅读入口使用旧 ReaderCanvas。已通过手机试读的开书动效依赖新版 ReaderView、书架的受控打开与位置恢复。先交付前置，让审查与验证区分阅读器基线和动效。

## Scope

用户批准单独提交并合入开书所需的阅读器前置，再提交动效。移植已验过的无框线 Svelte 阅读器与书架壳，以及运行、打开和返回所需的仓库接口。保留既有产品数据和外部文件打开契约。开书动效、应用图标和其他试装中的产品变更各自交付。

## Architecture Impact

present

- 目的：书架到真实正文具备稳定的页面宿主，后续在同一渲染路径上加过渡。
- 接口：Svelte ReaderView、ReaderEngine 与既有 WebView 阅读内核、书架和 Tauri IPC。保留静态阅读宿主。
- 权衡：交付已验过的新阅读器，而不是把动效另写给即将替换的旧壳；仅带实现和验证所需的直接依赖。
- 证据：前端构建、仓库快速门、真实 Linux 宿主打开和返回。后续若改书页定位或打开请求身份，重新检查边界。

## Acceptance Criteria

- [x] 应用入口、书架与 ReaderView 构建可用，原有打开、恢复与返回不回归。
- [x] 原生桌面文件入口和静态阅读宿主保留；未纳入其他在途改动。
- [x] 仓库快检各项、Chromium 和隔离 Linux 原生开书验证通过。
- [x] 代码与安全复审通过；前置 PR 合入 `main`（`326e7e7`）。

## Files And Steps

1. 从远端主线提取既有组件树、阅读内核桥接与直接宿主依赖。
2. 验证构建、书架开书、返回与桌面文件入口，核对可公开差异。
3. 审查前置 PR，再准备独立的动效 PR。

## Checks

已执行工作流/文档检查、Rust 格式与冻结锁离线 workspace 测试、Node 单测、Svelte 检查、Vite 构建、真实 Chromium 浏览器测试及隔离 Xvfb + Tauri/WebKitGTK 文件关联和开书返回门。

## Iteration Control

- Mode: `iterative`
- Maximum repair iterations: `3`
- Unchanged-delta limit: `1`
- Human stop: 未包含在批准范围内的产品变更，或缺少必需的真实目标验收。

## Result

从公开主线提取新版组件树、书架组织和打开所需的 Tauri 接口。编译确认书架投影依赖分类读取，补齐 EPUB、CBZ、FB2 与 Kindle 分类侧车及对应测试；新分类缓存和原渲染缓存的并发发布各自验证。Kindle 解码与其他未发布导入器改动未进入候选。

Node 137 项全过；Rust workspace 测试通过；Svelte 零错误和警告、Vite 构建通过。Chromium 完成 11 个界面场景。隔离 Linux 真实宿主通过冷/热文件打开、FIFO、位置恢复、返回书架、失败回退和日志隐私检查。

审查修复后的 Chromium 回归覆盖真实选区创建富文本根笔记、IPC 发送期间继续编辑、离线快照图片，以及按保存锚点跳到后续章节。消息与备份测试 24 项通过，1 项既有真实样本测试跳过；重新构建原生宿主并复核 Linux 打开、返回与失败回退。

## Review

[PR #4](https://github.com/weichzh/Atha/pull/4) 的独立审查提出五项阅读行为回归：原文选区冻结、根笔记一次性保存富文本、异步回复不清除新输入、按保存的原文锚点跳转，以及历史快照按捕获的字节还原图片。修复后，代码和安全复审均覆盖 `2f42221`，无未解决意见。

## Evidence And Residual Risks

本 change 负责前置版本交付。前置版本的浏览器与 Linux 证据独立于先前 PCT-AL10 动效试装。未对本候选执行 Windows 原生、Android 原生安装和自然触摸验收。完整本机收据在 Git 忽略目录。

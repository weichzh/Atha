# SVG 阅读主题兼容修复

## Status

implemented

## Problem

系统与 Atha 的显式阅读主题不同时，部分 WebView 将系统主题传给 SVG 图片。书页和 SVG 各自命中不同配色；仅设置嵌入元素的 `color-scheme` 在 WebKitGTK 2.54.3 不足以修复。

## Scope

保留上轮已验证的书源 CSS 色彩偏好投影。在阅读内容层复用 SVG 安全校验，只在引擎不传递指定的颜色方案、用户选定显式书页主题且 SVG 含可识别的作者颜色媒体查询时，生成会话内图片变体。未命中的资源保持原样，不变更书源、持久身份、权限或系统主题。

## Architecture Impact

none

## Approval

用户批准按已讨论的针对性兼容边界实施，并要求记录其长期原则。

## Acceptance Criteria

- 在 WebKitGTK 的系统深色环境中，浅白／暖纸主题下的自适应 SVG 与书源样式使用浅色配色；显式深色、系统主题以及切章恢复保持各自含义。
- 浏览器支持原生传递时使用原图；无颜色媒体查询的 SVG 和位图保持原样。所有变体走原有安全校验和 `img`，历史快照不变。

## Files And Steps

阅读内容层仅在 Linux WebKitGTK 路径投影可识别的 SVG 声明，切章和关闭时释放临时图片地址；正文、预览和新快照复用已验证资源。长期原则归入阅读内核兼容性契约。

## Checks

复用本机 WebKitGTK 和 Chromium，验证相反的系统／阅读配色，核对正文、图片预览与资源身份；保留组合色彩规则和未命中资源的回归。在 PCT-AL10 上复验原故障场景和夜间主题。

## Iteration Control

- Mode: `iterative`
- Maximum repair iterations: `3`
- Unchanged-delta limit: `1`
- Human stop: 真实目标安装、发布或扩大 SVG 兼容范围前单独确认。

## Rollback

回退本分支的渲染层修复；无须迁移书库。已保存的快照继续按现有契约呈现。

## Result

- Iteration: 1 — Canvas 读回把 Chromium 的原生呈现误判为不支持；兼容入口改为已复现缺口的 Linux WebKitGTK，Chromium 继续用原图。
- Iteration: 2 — 切回系统主题的回归发现 Chromium 会复用被修改的实时 CSSOM；改为投影 detached stylesheet 后再应用，切回系统时恢复原书 CSS。新增回归对基线失败、对候选通过；覆盖加载、预览、快照、过章及不适用的图片。
- Iteration: 3 — 独立审查指出样式表级媒体条件及同路径的两种快照呈现。排除含样式表级主题条件的 SVG；快照仅在同文件的原图与变体并存时为变体分配独立资源键。新增对照验证两种节点顺序和原始字节保留。
- Validation: 本机 WebKitGTK 2.54.3 中，系统深色／阅读浅白、系统浅色／阅读深色均显示书内对应配色；点击独立公式仍呈现浅色变体，关闭后留在原页。Chromium 保留原 SVG，夹具覆盖两种相反配色。Svelte 检查、前端和 Tauri 本机构建、文档门通过。
- Validation: 2026-11-28，PCT-AL10（Android API 29，华为系统 WebView 114.0.5.302）同签名覆盖安装基于 `09b3e9c` 的 arm64 测试包（0.1.1，versionCode 1001）。系统保留深色：同一原故障章节的行内和独立公式在「浅白」下为白底黑字，在「夜间」下为深底浅字；切回「浅白」后留在该节。
- Remaining delta: Linux 系统主题在大图已打开时切换，正文会刷新，已打开的大图重新打开后才更新。非 Linux 目标证据见下。

## Review

独立只读审查发现的两项阻塞问题已在本地修复，定向回归通过。未扩大资源授权、脚本能力或作用平台。

## Evidence And Residual Risks

本机原生 WebKitGTK、Chromium 和 PCT-AL10 真机证据分开记录；Windows 目标仍按 Issue #1 的平台验收跟进。WebKitGTK 原生通过系统／阅读主题相反时的 SVG 像素回归后，移除 Linux 投影分支；不以 SVG 能否在 Canvas 中读回作为退役判断。

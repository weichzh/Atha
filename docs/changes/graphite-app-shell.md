---
description: 恢复黑曜石暗色应用壳的石墨分层与暖铜强调，保留书内配色。
---

# 石墨暗色应用壳

## Status

implemented

## Problem

拆分移动体验时，书架、阅读记忆和设置已分别交付，但原候选中独立的石墨暗色应用壳未迁入主线。

## Scope

从已批准拆分的本地 V2 候选提取现有「黑曜石暗色」的石墨表面、暖铜强调和不透明导航。保留浅色样式、默认主题、导航尺寸与交互、设置入口及阅读器主题。

## Architecture Impact

none

## Acceptance Criteria

- [x] 黑曜石暗色在书架、阅读记忆和设置使用同一套应用壳颜色；导航保留选中和键盘焦点。
- [x] 桌面与窄屏切换主题时导航尺寸和位置不变；阅读正文不继承应用壳色板。

## Files And Steps

在 `tokens.css` 将色板限定于应用壳，沿用主导航组件现有布局。将结果交接给 [Issue #1](https://github.com/weichzh/Atha/issues/1)。

## Checks

Svelte、Vite、Chromium 桌面及 360px 窄屏、文档和工作流。

## Iteration Control

- Mode: `single-pass`

## Result

新色板只作用于暗色应用壳；桌面侧栏和窄屏底部导航由玻璃效果改为不透明石墨色。浅色外观及导航尺寸保持不变。强制颜色模式下，选中项仍有系统色轮廓。

Svelte 检查、Vite 与完整 Chromium 回归通过。1440px 和 360px 均验证主题切换前后导航几何一致、不透明导航与书架收藏可见。360px 另测强制颜色的导航焦点和选中态，以及从暗色书架开书时正文不继承壳色。

## Review

查看桌面和窄屏的书架及设置。发现暖铜色在固定浅色收藏徽章上偏淡，改为深铜后复验。独立审查记录在 PR。

## Evidence And Residual Risks

目标环境为桌面和窄屏 Chromium；原生 WebView 另行验收。

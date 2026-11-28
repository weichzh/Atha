---
description: 桌面横屏试行真实对页、中缝翻书与按可用宽度切换的笔记工作区。
---

# 桌面对页与笔记分栏试用

## Status

implemented

## Problem

现有卷页以整块阅读区域为一张纸。桌面横屏留有空间，但正文与动效都没有左右对页的结构。展开笔记时，需要兼顾正文可读性与笔记列表、对话的连续性。

## Scope

用户批准本地试用：宽横屏默认对页，卷页从中缝翻动单张纸；结合窗口宽度、比例及正文实际剩余宽度安排正文、笔记列表、对话。保留连续滚动、窄窗口、移动端和单幅 CBZ 的行为。

## Architecture Impact

present

- Design purpose: 同一正文按一列或两列分组导航，Locator 仍引用同一份正文。
- Drivers / quality scenarios: 宽横屏对页，展开侧栏保留阅读位置；超长章末页与减少动态仍可读。
- Modules / Interfaces / Seams / Adapters: 工作区共用尺寸规则；内核按实际 frame 选择步长。翻页图层仅读取当前安全 DOM。
- Candidate and tradeoffs: 不复制正文数据，不改 Locator、IPC 或书籍信任边界；对页卷页使用有正反面的单张纸，单页保留现有渲染器。
- Evidence / ADR / review trigger: 本地试读；复核中缝、双面内容和分栏往返后再决定正式提审。

## Acceptance Criteria

- 宽横屏同时显示相邻两页、有可见中缝。选「卷页」时，从中缝翻一张，正反面对应相邻页，支持正反向和取消。
- 宽窗口容纳正文、列表和对话；宽度不足时正文回到单页或与对话分阶段显示。关闭笔记返回原阅读位置。
- 保持短章、奇数末页、长章、窄屏、连续滚动和减少动态的导航行为。

## Files And Steps

1. 共用窗口分档，按正文实际宽度决定正文排版。
2. 以一份正文连续排版，实现对页步进与中缝两面翻书。
3. 检查实际渲染和阅读、笔记往返。

## Checks

- 对页卷页与工作区 Chromium 回归、Svelte 静态检查及构建。
- Linux WebKitGTK 的当前场景、文档门和局部 Node 回归。

## Iteration Control

- Mode: `iterative`
- Maximum repair iterations: `3`
- Unchanged-delta limit: `1`
- Human stop: 需要更改存储、信任边界、操作真机或对外发布时停止。
- Per-iteration receipt: 写入 Result，截图和输出留在本地忽略目录。

## Approval

用户批准实现桌面端试用。发布、提审和合入不在本轮。

## Rollback

回退本分支变更即可恢复单页与旧分栏，既有书籍偏好及 Locator 格式不变。

## Result

- 正文可用宽度至少 1160 CSS px、窗口高至少 560 CSS px 且宽高比至少 1.3 时，桌面分页默认对页。横屏阅读区不够宽时回到单页。连续滚动、CBZ、Android 保持既有布局。
- 宽至少 1360 CSS px 且宽高比至少 1.4 时，笔记保留正文、列表、对话三列；较紧凑的横屏分阶段显示正文与列表、列表与对话。窄屏或竖屏仍用底部面板。笔记列表换位时只移动视觉图层，两次排版之间仍用原 Locator。
- 按「排版与主题设置 → 本书覆盖 → 阅读方式 → 卷页」试双面翻书。动效偏好不被默认对页改写。
- Chromium 对页翻书、奇数末页、超长章、减少动态和完整阅读工作区通过。1400 px 同宽的 900 / 1100 px 高度分别验证三列／分阶段布局；关闭笔记后可见原文与 Locator 保留。相关 Node 79 项及 Svelte 静态检查通过。
- Linux WebKitGTK 实际窗口通过响应式布局、原生打开和笔记往返；五个实测窗口都保留完整 Locator 和页码。第一轮原生测试在列表仍移动时点导出，复验改为等面板落定后点击，通过。

## Review

- 检查了真实左右正文、中缝与卷起正面、三列笔记的截图。双面图层在拖动取消、跳页、重排与卸载时清理；不增加书籍资源读取路径。

## Evidence And Residual Risks

- 试读截图与本机输出：`artifacts/local/page-turn/`。
- Linux 收据 `linux.json` 使用 0 次预热、2 次采样，通过功能链路与 22 次内核运动测量，不含正式 P95 性能门。对页翻书图层在 Chromium 实测，Windows WebView2 尚待试读。

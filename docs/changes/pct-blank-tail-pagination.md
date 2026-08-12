---
description: 修复 PCT-AL10 公式占位文本扩张多栏分页并产生空白尾页的问题。
---

# PCT 空白尾页计数修复

## Status

implemented

## Problem

PCT-AL10 上的问题章节在第 10 页结束，但阅读器曾显示 29 页；从第 11 页起没有任何可见内容。导入结果、阅读状态和正文 DOM 均完整，故障位于华为 WebView 114 的多栏排版链路。

首版候选把整章 `Range` 改成末节点 fragment 量测，本地门通过，但原位安装后同一节变为 38 页且第 36 页仍为空白。该候选证明最初的 Range 假设错误，不能作为真机完成证据。

调试包通过 WebView CDP 读取不含正文的几何后确认：`content` 为延迟加载 SVG 公式而移除 `src`，保留了 `alt`、显式宽高和隐藏状态；WebView 114 仍把无 `src` 图片的长 `alt` 回退文本按正文行高分进 CSS 多栏。186 个公式因此把真实结束于第 10 列的章节扩张到第 35 列。把公式的字体和行高归零后，图片宽高不变，最后内容列立即回到第 10 列。

公式首次显现后，WebView 114 还会改变最终分页几何。成功加载的固定盒图片按原设计不报告逐页布局变化，因此初次恢复可能暂时保留 11 页总数；初次可见资源完成后需要统一再计一次分页。

## Scope

- 保留现有 CSS 多栏分页器、整章 `Range` 计页和公式渐进加载；
- 对待加载和已加载公式统一归零文字行盒，阻止无 `src` 图片的 `alt` 回退文本参与分栏；
- 初次可见资源完成后统一重排一次，不把每次公式显现改成逐页重排；
- 删除首版候选的末节点 fragment 计页和 Range monkeypatch 回归，改用公式占位样式探针；
- 保留诊断宽表按视口产生有效横向行程的修正；
- 完成本地、Linux GUI、公式压力、Android 包结构和 PCT-AL10 原位更新验收。

## Non-Goals

- 不引入 Readest 分页器、额外 WebView 或新依赖；
- 不预热整章公式，不改变三槽并发、Locator、手势仲裁、滚动模式或原生长章节阈值；
- 不清理阅读数据，不保存或输出私密书籍的标题、路径、正文或内容身份。

## Acceptance Criteria

- [x] 问题章节首次恢复即显示 10 页，第 10 页有正文且不存在可到达的第 11 页；
- [x] 第 6 页至第 10 页逐次翻页均只前进一页；
- [x] 从第 10/10 页继续前进进入下一节第 1 页，回翻准确返回第 10/10 页；
- [x] 公式继续渐进加载时，最后内容列和 10 页总数保持稳定；
- [x] Linux GUI 阅读器门、公式压力冒烟、前端检查和 Android arm64 正式包门通过；
- [x] PCT-AL10 原位更新保留应用数据、首次安装时间和阅读进度。

## Architecture Impact

none

本次不改变 Module、Interface、数据语义、信任边界、依赖或运行拓扑，只修正公式占位的 CSS 几何，并复用现有初次分页稳定点重新计页。

## Files And Steps

1. 用调试包和 CDP 对比待加载公式、已加载公式、正文 fragment、列数与 `scrollWidth`；
2. 在公式共享样式中归零 `font-size` 与 `line-height`；
3. 初次可见资源加载后复用 `relayoutAtOffset()` 再计页，保留逐页成功显现不重排；
4. 恢复原有整章 Range 计页，增加公式占位样式诊断并运行正式门；
5. 构建、签名并原位安装正式 arm64 包，在同一章节验证末页和跨节边界。

## Checks

- `node --check`、Node 13 项测试和 `git diff --check`；
- `mise exec -- pnpm --dir reader/app check` 与 production build；
- `bash scripts/check-reader-linux.sh`；
- 既有私密 sidecar 驱动的公式压力冒烟，不输出书籍身份；
- `bash scripts/check-pct-reader.sh build`、`verify` 与获批后的 `install`；
- `autocorrect`、docs gate 与独立 review。

## Approval

用户明确回复“开始修复。”批准分页修复，并进一步说明手机安装与真机验证无需重复批准。首版候选复测失败后，继续定位并修正同一根因仍在该批准范围内。

## Result

`reader/atha-reader.css` 只为 `.math-inline` 与 `.math-display` 增加零字体和零行高；行间公式的上下间距改由当前阅读字号计算，显式图片宽高、公式倍率、可访问名称、视觉间距与渐进加载保持不变。`pagination.renderFromStart()` 在初次可见资源实际加载后复用现有稳定重排入口再计页；后续逐页公式成功显现仍不触发重排，避免 Locator 恢复造成跳页。

首版候选的末节点 fragment 计页已撤回，生产逻辑恢复原有整章 Range。诊断不再篡改平台 `Range`，而是直接验证长 `alt` 的待加载公式不会形成文字行盒。

PCT-AL10 调试包最终记录 186 个公式，其中 149 个已加载、37 个待加载；正文和公式的最后几何均位于第 10 列。正式包首次恢复为第 10/10 页，自动前翻进入下一节第 1 页，自动回翻返回第 10/10 页。手机最终保留在问题章节第 10/10 页。

最终 Android arm64 正式包 SHA-256 为 `5b6f985509e425a8d2754f10f3cb21a1e968334fd88b0cf7f48f870fb438c88e`，签名与既有安装一致，16 KiB ZIP 与 ELF 对齐通过。原位安装证据位于 `artifacts/local/audits/pct-reader-install-20260812T170212Z-131006`，正式包边界证据位于 `artifacts/local/audits/pct-blank-tail-final-release-20260812T1703`。

## Review

以故障修复前的 `37d572b` 为基线完成范围、标准和回归审阅。审阅发现零字体会让原有 `0.9em` 行间公式间距归零，已改为由当前阅读字号写入等价 CSS 变量，并重新运行 Linux GUI、公式压力、正式包和真机边界门。最终没有 blocking finding，未新增依赖、接口或内容日志。

## Evidence And Residual Risks

- 静态与本地：reader module 语法、Svelte check、Vite production build、Node 13 项测试和 diff 检查通过；
- Linux GUI：公开书完整 13 场景、每场 5 次预热和 20 次测量通过，220 个计时样本的最大帧间隔为 17ms，日志隐私检查通过；
- 公式压力：1332 个公式全部稳定，独立空尾 oracle 与生产页数同为 65；低采样 13 场景手势冒烟和日志隐私检查通过；
- Android 正式包：arm64、v2 / v3 签名、16 KiB ZIP 与 ELF 对齐、证书一致性和保数据原位安装通过；
- 真实目标：PCT-AL10 上的自动逐页、末页、跨节和返回链路通过。ADB swipe 不是自然手指触摸证据，实体触摸手感仍由用户日常阅读确认。

---
description: 修正 PCT-AL10 首次打开公式表格时的正文收缩，并记录真机验收边界。
---

# PCT 首开排版稳定

## Status

implemented

## Problem

PCT-AL10 首次打开包含 SVG 公式的书页时，延迟公式从无 `src` 占位切换为已加载图片后才取得内在宽高比。Huawei WebView 114 因此重新计算表格行高，视频中表格下缘和后续正文上移约 82 像素。

## Scope

- 对具有合法显式宽高的延迟 SVG 公式显式保留宽高比，使加载前后使用同一几何盒；
- 保持当前页优先加载、SVG 安全校验、失败占位和渐进加载策略不变；
- 用现有内容自检、Linux 正式门和 PCT-AL10 同包更新复测首开书页。

## Non-goals

- 不等待整页、整节或全书图片加载后再揭示正文；
- 不改变表格排版、公式大小、分页算法、书源样式或阅读偏好。

## Architecture Impact

none

## Acceptance

- `LAYOUT-FIRST-01`：合法宽高的延迟 SVG 公式在设置 `src` 前具有相同的显式宽高比，加载成功不改变表格与正文几何；
- `LAYOUT-FIRST-02`：原视频对应章节首次打开后，下方正文不再随公式显现向上收缩；
- `LAYOUT-REGRESSION-01`：内容安全自检、Linux 阅读器正式门和 PCT 安装后启动检查通过。

## Files And Steps

1. 在共享内容渲染路径复用已有尺寸边界，为延迟公式补入浏览器可直接使用的 `aspect-ratio`。
2. 扩充现有内容自检，锁定占位宽高比及书源样式切换后的保留行为。
3. 更新阅读核心与代码地图事实，构建并在 PCT-AL10 复测原章节。

## Checks

- `node --check reader/web/content.mjs`；
- `bash scripts/check-reader-linux.sh`，并以私密压力章节补充公式路径；
- `bash scripts/check-pct-reader.sh build`；
- `bash scripts/check-pct-reader.sh install --device 5ENDU19917001679`；
- 原视频章节的 PCT-AL10 首开连续截图与逐帧几何检查；
- 仅对本次中文 Markdown 运行 `autocorrect --fix` 与 `autocorrect --lint`。

## Result

共享内容渲染在应用书源样式后，从公式合法 `width` / `height` 写入明确的 `aspect-ratio`。无 `src` 占位与加载后的 SVG 因而使用同一内在比例；没有增加整页等待、重排或新的加载状态。

## Review

首轮独立 Standards 与 Spec review 指出小数显式尺寸资格被缩窄、自检没有经过真实 `setStyles()` 路径、作用域超出待加载公式，以及缺少 `Architecture Impact`。修正为保留有限正数语义、只处理待加载公式、覆盖书源样式开关并补入 `Architecture Impact: none` 后，两轮独立复审均为零发现。

## Evidence And Residual Risks

- 静态 / 本地：`node --check`、`svelte-check` 和无私密样本的完整 Linux Tauri GUI 门通过，后者包含 220 次手势测量与 AppLog 隐私检查；短目标章节加载 154 / 154 个公式并保持 6 页内容分页。
- 本地补充：私密压力章节通过公式形状、稳定加载、边界和手势阶段；该轮随后在未触碰的阅读记忆检查遇到 `recent=2`、脚本固定期待 `1`，因此不把该整轮记为完整绿灯。
- 真实目标：候选 APK SHA-256 为 `beb07537da35d5cb715775280769cf8da725a787ca47372baba30e304d092a6c`，已同包更新到 PCT-AL10；`artifacts/local/audits/pct-reader-install-20260817T105620Z-2343272/result.txt` 确认签名与版本匹配、首次安装时间保留、未请求清数据、进程存活且主界面聚焦。
- 真实目标：同一章节冷启动后首个完整正文截图到第 36 帧，表格下缘均为 1765 像素，上移量从原视频的 82 像素降为 0；截图序列位于 `artifacts/local/audits/pct-first-open-layout-stability-20260817/sequence-20260817T105650Z/`。
- 剩余边界：ADB 连续截图是真机视觉与几何证据，不等于自然手指体验；最终观感仍由用户在当前已安装版本上确认。

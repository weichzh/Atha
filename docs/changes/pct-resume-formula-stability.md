---
description: 修复 PCT-AL10 恢复阅读位置时的分页跳动和公式错误闪现。
---

# 恢复阅读位置时的分页与公式稳定性

## Status

implemented

## Problem

PCT-AL10 从书架续读时，正文出现后仍自行跳页，间歇显示浏览器的破图图标与公式替代文字。同一公式之后可以正常显示；Linux 未报告该现象。真机录屏在约 20 帧/秒下捕获到没有用户输入的多次变化。

## Scope

- 修复从书架继续阅读时的首屏定位及公式从占位到显示的稳定性。
- 保留书籍资源安全校验、真实加载失败提示与渐进加载，不等待整章资源加载完成。
- 本次单独走修复分支与 PR，不改 SVG 阅读主题兼容策略。

## Architecture Impact

none

## Acceptance Criteria

- [x] 有声明尺寸的延迟公式在等待和解码期间保留布局占位，不展示浏览器临时破图；可解码的公式会显示，真实失败仍有提示。
- [x] 加载期间抓取公式的引用时，历史快照只保存已校验书源，不保存加载标记或占位地址。
- [x] PCT-AL10 上从书架重复冷、热续读：首个可读画面落在保存的位置，之后不自行跳页、不闪现破图，再次打开仍回到该位置。
- [x] Linux 阅读原生回归及受影响的浏览器回归通过。

## Files And Steps

1. 以实际阅读器样式复现延迟公式的占位缺口，记录加载前、成功及失败的状态。
2. 修复占位与布局，再依据真机前后对照决定是否还需修改定位或调度。
3. 用高频录屏复验冷、热续读，对照原生 Linux 阅读检查。

## Checks

- 延迟 SVG 公式的浏览器回归及既有 SVG 配色回归。
- `bash scripts/check-reader-linux.sh`。
- PCT-AL10 同包候选的续读录像；从点书前开始记录直到页面稳定。

## Iteration Control

- Mode: `iterative`
- Maximum repair iterations: `3`
- Unchanged-delta limit: `1`
- Human stop: 需要用户决定、外部批准或真实目标操作超出已批准范围时停止。
- Per-iteration receipt: 在 Result 与 Evidence And Residual Risks 记录改动、剩余差距与证据级别。

## Rollback

回退本次独立修复提交。不改动书库、书源或阅读进度格式。

## Approval

用户确认独立 PR、共用当前工作目录，并明确开始修复。

## Result

- Iteration: 1
- Review findings: 实际 Tauri 样式未隐藏待加载公式。只恢复隐藏规则后，无 `src` 的图片仍以方形计算自动尺寸；接收真实 SVG 后，相邻正文和 Locator 所在列移动。
- Repair: 使用同源透明资源为待加载公式保留原生图片盒，沿用声明宽高比和书源 `em` 尺寸；终态只接受已校验书源的加载，真实失败保留原提示。没有改动 Locator、分页算法或配色策略。
- Validation cases: 产品样式下的内联公式比例、独立公式比例、超过 50ms 的网络响应、真正失败占位；SVG 主题兼容回归；完整 Linux 阅读门；PCT-AL10 的 2 次冷开与 2 次热开。
- Remaining delta: 复核独立审查。
- Evidence level: Chromium 浏览器、原生 Linux 产品、Android 真机。
- Stop reason: 首轮真机复验通过，转独立审查。

- Iteration: 2
- Review findings: 两个尺寸均为 `auto` 时仍缺少固有尺寸；等待状态会进入历史快照；Tauri 真实失败占位没有可见标志。
- Repair: 延迟阶段增加声明的固有尺寸提示；在快照副本校验并写回书源、移除所有图片加载标记；真实失败显示警告标识，不暴露浏览器破图。
- Validation cases: 内联相对尺寸、双 `auto`、真实请求在 50ms 后成功或失败、校验拒绝、设置书源前及解码等待中捕获并回放原图、SVG 配色回归、Linux 正式门和真机续读复验。
- Remaining delta: 无阻塞项。
- Evidence level: Chromium 浏览器、原生 Linux 产品、Android 真机。
- Stop reason: 约定的回归及原生验证全部通过。

## Review

- Blocking: 已按第二轮修复双 `auto`、快照加载状态与真失败显示，原候选分别有尺寸、回放和可见提示缺口。
- Non-blocking: 就绪缓存命中的跨节主题变体呈现沿用原逻辑；只读 Windows 宿主仍走原占位路径。

## Evidence And Residual Risks

- 基线与候选均为 PCT-AL10 上的 Atha 0.1.1（1001），使用 Huawei WebView 114。基线从首屏出现到打开通知前，出现多次无输入的分页与正文位移，并闪现浏览器破图。候选以同签名覆盖安装，保留书库与进度。
- 最终候选真机录像共 1,596 帧，平均约 19.1 帧/秒，从点按前开始。两次冷开、两次热开的首个可读画面与最终正文均在同一保存位置，节内页数固定，未见自行翻页或破图。逐帧差分与录像留在本机忽略目录。
- 最终候选 `svelte-check` 无错误与警告，两个受影响的浏览器回归和完整 Linux 原生阅读门均通过，后者含 220 次手势样本。

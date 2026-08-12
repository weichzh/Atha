---
description: 修复 PCT-AL10 多栏章节把内容范围误算成连续空白尾页的问题。
---

# PCT 空白尾页计数修复

## Status

implemented

## Problem

PCT-AL10 上的当前章节在第 10 页结束，但阅读器显示为 29 页；第 11、12 页没有任何可见内容，返回第 10 页后正文恢复。真机 UI 树仍保留正文节点，已安装 APK 也与当前候选一致，因此问题不在导入、缓存或内容丢失，而在分页总数。

`pagination.contentPageCount()` 当前创建从书根开头到最后一个有意义节点的整段 `Range`，再以其 `getBoundingClientRect().right` 推算页数。多栏布局中的 Range 外框会合并选中片段和完整元素盒；PCT WebView 114 因此把实际结束于第 10 列的内容范围放大到第 29 列，Navigation 随后允许用户进入不存在的空白列。

## Scope

- 继续由 `pagination` 唯一拥有分页总数，只替换错误的整段 Range 外框量测；
- 以最后一个有意义文本或媒体节点的实际 fragment 几何确定最后内容列，保留书源内部有内容的留白和尾部空盒过滤；
- 在现有 diagnostics 跨章边界探针中加入 Range 外框被放大的确定性回归，不增加测试框架或依赖；
- 让现有宽表手势夹具按视口保留足够的横向溢出，避免 DPR 2 下固定宽度使正式 Linux 门失去有效行程；
- 运行阅读器语法、Linux GUI、公式压力、前端构建和 Android 候选构建；真机安装仍等待用户对具体候选的单独批准。

## Non-Goals

- 不修改导入、缓存、Locator、手势仲裁、图片或公式加载；
- 不改变滚动模式、DPR 模型、20,000px 原生分页阈值或阅读数据；
- 不保存或输出私密书籍的标题、路径、正文或内容身份。

## Acceptance Criteria

- [x] Range 整段外框即使被人为放大，分页总数仍与独立的可见 fragment oracle 一致；
- [x] 尾部强制分栏空盒继续不计入可到达页数，跨章向前仍落在上一节最后真实内容页；
- [x] 当前问题章节的最后真实内容页不再产生第 11 至 29 页的可到达空白列；
- [x] Linux GUI 阅读器门、公式压力冒烟、前端检查与 Android arm64 候选构建通过；
- [x] PCT-AL10 原位更新后的自动翻页复测通过，或明确保留为尚未获批的真实目标验收边界。

## Architecture Impact

none

本次不改变 Module、Interface、数据语义、信任边界、依赖或运行拓扑，只修正 `pagination` 已有职责内的内容边界量测，并复用现有 diagnostics oracle 验证。

## Files And Steps

1. 在现有尾部空列探针中模拟 Range 外框膨胀，证明当前生产计页会偏离独立 oracle；
2. 把生产计页收窄到最后有意义节点的实际 fragment，不扫描整段 Range 外框；
3. 跑定向语法与 Linux GUI 门，再跑公式压力、前端和 Android 构建；
4. 完成独立 review、事实所有者和证据记录，提交候选；获得批准后才安装到 PCT-AL10 复测。

## Checks

- `node --check reader/web/pagination.mjs` 与 `node --check reader/web/diagnostics.mjs`；
- `bash scripts/check-reader-linux.sh`；
- 既有私密 sidecar 驱动的公式压力冒烟，不输出书籍身份；
- `mise exec -- pnpm --dir reader/app check` 与 production build；
- Android arm64 APK 构建、包结构与签名检查；
- `autocorrect`、docs gate、`git diff --check` 与独立 review。

## Approval

用户在看到真机根因结论后明确回复“开始修复。”，批准上述分页修复范围；该批准不包含向手机安装候选。

## Result

`pagination.contentPageCount()` 仍只在排版稳定点运行，但不再建立从书根到最后内容的整段 Range。它先保留有意义文本与媒体节点的 DOM 顺序，再从末尾找到第一个具有真实 fragment 的节点，以该节点的 `getClientRects()` 或元素 rect 确定最后内容列；尾部隐藏且无几何的节点安全回退到前一个真实内容，空书仍保持一页。Navigation、Locator、滚动模式和原生长章节分支没有改动。

现有尾部空列探针会在重排期间把 `Range.getBoundingClientRect()` 人为放大三个视口；生产页数仍必须等于独立逐 fragment oracle。Linux DPR 2 同时证明固定 1200px 的宽表夹具只剩不足一次手势的有效行程，因此夹具改为至少比当前列宽多 480px，不改变产品手势逻辑。

与真机问题相同的本地章节在最终源码下得到 `pages=10`、`contentPages=10`、`scrollPages=10`；公式压力章节得到 1332 个已稳定公式、65 个生产页和 65 个独立内容页，继续进入原生长章节分支。最终 Android arm64 候选 SHA-256 为 `353d01345ae97a8232aad7b867387f3fa92d5b8793d678b14e9a441f0a2a0827`，签名与手机现有安装一致，但尚未安装。

## Review

以 `37d572b` 为基线完成单独的范围、标准和回归审阅；没有 blocking finding。实现只修改共享计页入口及既有诊断，未新增依赖、接口或书籍内容日志。保留的风险是新候选尚未在 PCT-AL10 上执行真实目标复测。

## Evidence And Residual Risks

- 静态与本地：reader module 语法、Svelte check、Vite production build 和 Node 13 项测试通过；
- Linux GUI：公开书完整 13 场景、每场 5 次预热和 20 次测量通过，220 个计时样本的最大帧间隔 17ms，日志隐私检查通过；
- Linux 精确内容：问题章节为 10 / 10 / 10 个生产页、独立内容页与滚动页；公式压力冒烟为 1332 个公式、65 / 65 个生产页与内容页，公开和压力书的空尾、Range 外框膨胀及跨章边界均通过；
- Android 候选：arm64-only、v2 / v3 签名、16 KiB ZIP 与 ELF 对齐通过，候选哈希见 Result；候选证书与现有安装证书一致；
- 真实目标边界：手机仍运行旧 APK；未经单独批准没有安装、清数据或卸载。新候选的页数、自动翻页和自然手指体验仍需在 PCT-AL10 上验收。

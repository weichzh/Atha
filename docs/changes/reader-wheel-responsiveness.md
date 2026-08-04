---
description: 量化并修复图片悬停滚轮失效与连续滚轮翻页不跟手。
---

# 阅读器滚轮响应优化

## Status

implemented

## Problem

图片获得预览按钮语义后也进入了通用交互保护范围，鼠标停在图片上时滚轮不能翻页。滚轮检测器又把空闲间隔小于 240ms 的全部输入锁成一次手势；当前快速探针中，4 次间隔 100ms 的标准滚轮输入只接受 1 次，无法跟随用户连续浏览。

现有 `page_turn` benchmark 直接调用分页，P95 约 6.7ms，只证明分页本身足够快，没有覆盖“滚轮事件到稳定页面”的输入链路。

## Scope

- 新增数秒内完成的真实浏览器滚轮探针，分别记录图片目标是否接受滚轮、连续标准滚轮输入接受率和事件到稳定页面的 P95；
- 图片和公式保留点击预览、键盘焦点与链接语义，但滚轮继续交给阅读器翻页；表单、对话框、表格、代码和编辑区的原生交互保护不变；
- 标准离散滚轮输入逐次翻页；小幅高频输入继续累计阈值，并在同一精密手势中抑制惯性尾流；
- 复用现有 Interaction、Navigation、Pagination 和诊断入口，不增加依赖、动画、预测翻页或第二套导航队列。

## Non-Goals

- 不调整触摸滑动、点击翻页、键盘、分页布局、图片解码或绘制；
- 不把本机数据表述为跨设备性能承诺；
- 不优化尚未被新探针证明为瓶颈的 Navigation 或 Pagination。

## Acceptance Criteria

- [x] 普通图片、公式及链接包裹图片上的滚轮可以翻页，点击和键盘预览行为不回退；
- [x] 4 次 `deltaY=100`、间隔 100ms 的离散滚轮输入接受 4 次，每次只产生一次翻页；
- [x] 小幅连续输入仍需累计达到阈值，同一精密手势的尾流不会重复翻页；
- [x] 新探针记录滚轮输入到 Navigation 稳定的 nearest-rank P95，并在当前基线下不超过既有 50ms 翻页门槛；
- [x] 正式样书、Svelte/Tauri 检查、持久化交互和独立 review 无 blocking 回退。

## Files And Steps

1. 在 Diagnostics 增加只在验证模式暴露的滚轮测量，使用真实书内图片和现有 Navigation idle 边界；
2. 增加快速 Agent Browser 脚本，让当前图片与连续输入问题先稳定失败；
3. 在 Interaction 中只拆分图片目标与标准离散滚轮识别，保留其他保护规则和精密手势抑制；
4. 运行快速探针、正式阅读器与 Tauri 检查，更新阅读内核和代码地图后独立 review。

## Checks

- `pwsh -NoProfile -File scripts/check-reader-wheel.ps1`
- `pwsh -NoProfile -File scripts/check-reader-samples.ps1`
- `pwsh -NoProfile -File scripts/check-tauri-reader.ps1`
- `pnpm --dir reader/app check`
- `pnpm --dir reader/app build`
- `pwsh -NoProfile -File scripts/Invoke-Atha.ps1 check docs -Activity validation -Scope project-workflow`
- `autocorrect --fix` 与 `autocorrect --lint` 仅处理本次中文 Markdown
- `git diff --check`

## Rollback

按本次提交整体回退；没有持久化 schema、书籍数据或依赖迁移。

## Approval

2026-08-04：用户要求先量化图片悬停滚轮失效和滚轮浏览不跟手，再进行常规性能优化。

## Result

- 图片不再因预览按钮语义吞掉滚轮；普通图片、公式和链接包裹图片均沿既有 Navigation 路径翻页，点击与键盘预览规则未改变；
- 标准离散滚轮每个事件翻一页，小幅精密滚轮仍累计阈值并抑制同一手势尾流；
- 新增快速真实浏览器探针，固定检查媒体目标、4 次连续输入、单步翻页和输入到稳定页面的 P95，不增加依赖或第二套导航队列。

## Review

- Blocking：两轮独立复核最终均无 blocking；过程中发现的链接图片零覆盖、只比较页键而未严格断言单步、正式样书类型条件过严及无关 MAP 改动均已修正。
- Non-blocking：链接图片路径因当前 fixture 没有真实样本，使用挂接到书页 DOM 的 `a[href] > img` 合成目标覆盖精确事件路径。
- Out-of-scope：未调整图片解码、绘制、触摸与点击翻页，也未把本机结果外推为跨设备承诺。

## Evidence And Residual Risks

- 修复前快速探针：普通图片目标不接受滚轮，4 次标准输入仅接受 1 次；证明问题位于输入策略，不在约 6.7ms 的 Pagination 本身。
- 修复后 `scripts/check-reader-wheel.ps1`：真实鼠标停在普通图片上可翻页；普通图片、公式和链接图片均 accepted、defaultPrevented 且 singleStep；连续输入 4/4，nearest-rank P95 为 1.4ms，低于 50ms 门槛。
- `scripts/check-tauri-reader.ps1`：Svelte 检查和构建、Rust 测试及 benchmark 通过；run `1785848000505-27260` 的 page-turn P95 为 6.7ms，cold-start 584.571ms，first-stable 156.5ms，hot-open 21.1ms，font-reflow 41.6ms，均在门槛内。
- 正式样书入口曾完整通过；最终运行中前三本样书的浅色、深色及滚轮探针通过，数学样书在调用新增 `wheelProbe()` 前的既有真实 `Ctrl+C` 验证后由 Agent Browser daemon 返回 EOF。独立 namespace、关闭扩展和数学样书单跑均复现该工具链故障；同一数学样书的隔离滚轮探针当前通过。
- 最高证据等级为本机真实 Chrome/Agent Browser 与本机 Tauri benchmark；尚未覆盖物理精密触控板、不同滚轮驱动和其他电脑。

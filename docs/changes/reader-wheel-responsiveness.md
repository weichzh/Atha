---
description: 量化并修复图片悬停滚轮失效与连续滚轮翻页不跟手。
---

# 阅读器滚轮响应优化

## Status

accepted

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

- [ ] 普通图片、公式及链接包裹图片上的滚轮可以翻页，点击和键盘预览行为不回退；
- [ ] 4 次 `deltaY=100`、间隔 100ms 的离散滚轮输入接受 4 次，每次只产生一次翻页；
- [ ] 小幅连续输入仍需累计达到阈值，同一精密手势的尾流不会重复翻页；
- [ ] 新探针记录滚轮输入到 Navigation 稳定的 nearest-rank P95，并在当前基线下不超过既有 50ms 翻页门槛；
- [ ] 正式样书、Svelte/Tauri 检查、持久化交互和独立 review 无 blocking 回退。

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

待实施。

## Review

- Blocking：待实施。
- Non-blocking：待实施。
- Out-of-scope：待实施。

## Evidence And Residual Risks

待实施。

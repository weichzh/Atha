# R4D 表格与代码预览

## Status

implemented

## Problem

R4C 已闭合图片与公式预览，但宽表格和长代码仍受固定分页宽度限制。直接缩放书源节点或把 `outerHTML` 复制到应用壳，会引入书源样式、链接、图片和未知属性，并破坏表格、代码原有的选择语义。

Readest 证明独立表格查看器有使用价值，但其 `dangerouslySetInnerHTML`、缩放、拖拽和多层覆盖不适合 Atha 当前的不可信内容边界。Atha 只复用现有原生 dialog：表格从 caption、行、单元格纯文本和有限跨度重建；代码只投影 `textContent`。

## Scope

- 表格与块级代码保持原生语义和文本选择，获得键盘焦点、可见焦点和安全操作标签；
- 双击表格或代码，或在其自身焦点上按 Enter、Space，打开现有原生 dialog；
- 表格预览只重建 caption、`tr`、`th`、`td` 及受限 `rowspan`、`colspan`；代码预览只设置 `textContent`；
- 预览区可在 dialog 内横向和纵向滚动，关闭后焦点返回触发内容；
- 表格、代码及其内部链接、选择操作不会触发背景翻页；链接仍优先进入 R4B 策略；
- 打开和关闭不改变正文 DOM、section、页码、Locator、资源许可或网络策略。

## Non-Goals

- 不实现缩放、平移、拖拽、复制按钮、下载、语法高亮切换、行号或代码执行；
- 不克隆或注入书源 HTML、CSS、图片、链接和事件属性；
- 不实现表格编辑、排序、筛选、冻结行列、CSV 导出或复杂数学表格重排；
- 不改变正文内联 `code` 的行为；
- 不在 R4D 建设最终沉浸式控制层。

## Acceptance Criteria

- [x] 表格与块级代码可由双击、Enter 或 Space 打开预览，Escape 或关闭按钮退出并恢复焦点；
- [x] 表格 caption、行列文本、表头和合法跨度可读，宽内容在 dialog 内滚动；代码空白和换行保持；
- [x] 预览 DOM 只含应用创建的安全节点和纯文本，不含书源链接、图片、样式或活动 HTML；
- [x] 正文选择、内部链接和单击行为保留，表格/代码交互不翻页、不改变 Locator 或 section；
- [x] 包含表格与代码的现有样本在实际 host、明暗浏览器、Rust 检查和 benchmark 中保持通过；
- [x] 独立规格与标准 review 无 blocking，事实所有者和 `ACTIVE` 与最终实现一致。

## Files And Steps

1. 在内容校验后为 `table` 和 `pre` 补焦点与预览标记，不改变其原生 role；
2. 增加独立结构化内容动作并扩展现有 dialog，按纯文本安全投影表格与代码；
3. 扩展交互保护与真实诊断，覆盖鼠标、键盘、焦点、滚动、链接优先和位置不变；
4. 运行实际 host、四样本、benchmark、文档 gate 与独立 review。

## Checks

- 所有页面 JavaScript module 的 `node --check`；
- `cargo fmt --all --check`、`cargo clippy --workspace --all-targets --locked -- -D warnings`、`cargo test --workspace --all-targets --locked`；
- `pwsh -NoProfile -File scripts/check-reader-samples.ps1`；
- `pwsh -NoProfile -File scripts/check-reader-slice.ps1`；
- workflow `docs` gate；
- 本次中文 Markdown 的 `autocorrect --fix` 与 `autocorrect --lint`；
- `git diff --check`。

## Rollback

回滚本 change 的提交即可恢复 R4C 行为；不涉及耐久数据、资源迁移或外部写入。

## Approval

用户明确授权依据当前路线图继续实现到 M2 结束，并要求缺少规格时补规格。本 change 只完成 R4 的表格与代码纵向切片。

## Result

复用 R4B/R4C 的原生 `content-dialog` 完成表格与代码预览。内容校验为非链接 `table` 和 `pre` 补键盘焦点与安全操作标签，不改变原生 role；`Interaction` 把这些区域视为内容控件，保留选择、链接和单击而不触发翻页。双击及自身焦点上的 Enter、Space 进入预览，关闭后恢复焦点。

表格只从源 caption、行、`th`、`td`、安全文本与受限跨度创建应用节点；图片公式转为最多 160 字符的 `alt` 文本。代码只设置 `textContent`。预览 DOM 不复制书源 HTML、样式、链接或图片，明暗模式均在可聚焦的独立滚动区内显示。结构化动作和验证已从原链接、脚注与图片动作中拆出，Readest 的 `dangerouslySetInnerHTML`、缩放、拖拽和覆盖层没有引入。

## Review

- Spec：最终复审无 blocking；首轮提出的文本投影断言、图片替代文本限长和块级文本分隔均已落实；
- Standards：最终复审无 blocking；首轮提出的自身焦点、滚动区可访问性、替代文本限长、双击选择保护与模块职责问题均已修正。

## Evidence And Residual Risks

- 最高证据等级：Windows 真实目标证据；
- `scripts/check-reader-samples.ps1 -BasePort 19800` 通过四样本实际 WebView2 host 与 Agent Browser 明暗验收；逻辑样本验证 1 个表格，范畴样本验证 8 个代码块与代码内链接优先；
- 正式 Agent Browser 使用真实 Enter、Space 与 Escape 验证表格/代码打开、关闭和焦点返回，并保存明暗预览截图；closed Shadow DOM 下的双击由模块诊断覆盖，未为工具限制增加产品代理节点；
- `scripts/check-reader-slice.ps1` 通过 Rust fmt、clippy、test、实际 host 自检与 10 样本 benchmark；run `1785697478560-31108` 的中位数为冷启动 820.121ms、首个稳定页面 166.500ms、热打开 20.800ms、翻页 6.200ms、字号重排 27.750ms；
- 定向双样本首次串行运行中，范畴样本的 Agent Browser 等待曾超时一次；同一入口单独读取状态为 `pass`，随后单样本与最终四样本均通过，未再复现；
- 最终复审保留三项非阻断证据边界：复杂混合内容的块分隔尚无独立 oracle，滚动仅由 CSS、焦点与截图验证，双击仍是模块合成事件；
- 未执行旧提交的同时间对照，性能数据只证明当前正式门槛保持通过；
- 缩放、拖拽、复制按钮、导出、代码执行、语法切换、表格编辑和排序仍按 Non-Goals 保留。

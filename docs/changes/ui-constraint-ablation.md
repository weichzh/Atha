---
description: 独立 UI 约束消融原型的范围、验收和证据边界。
---

# UI 约束消融原型

## Status

implemented

## Problem

当前 UI/UX 约束需要先做 Stage 0 视觉校准：用户给定的微信读书和 Telegram 截图只作为界面几何、层级、token、排版与交互骨架的参照；随后在不带这些标签和截图的条件下运行 Atha 单变量消融。若把参照复刻、生产阅读器和实验变体混在一起，视觉差异会被误当成约束因果，且无法形成严格的单变量对照。

## Scope

- **Stage 0：视觉校准。** 在独立的参照视图中，分别校准用户给定的微信读书截图和 Telegram 截图；“忠实”仅指界面几何、层级、token、排版与交互骨架。Telegram 竖屏视图的大媒体块必须使用唯一允许的实际位图 `reader/app/src/ui-ablation/media-preview.png`，不得用 CSS 绘制或伪造图片替代；该资产只能表示合成匿名媒体，不复制截图中的人物或内容。所有可见文案使用合成文字，头像只使用合法匿名占位状态；不复制截图中的个人头像、会话内容或长篇受版权文本。每个来源有自己的根节点、样式和状态，不进入 `C0`/`A1` 至 `A7`，不进入盲评材料。参照图只用于校准，不用于证明 Atha 约束或产品优劣。
- **Stage 1：Atha 消融。** 只新增独立的 `/ui-ablation.html` 原型入口，在本地 Vite 离线服务器中运行；它使用本地合成夹具，不接入生产 reader、MessageStore、真实书籍、真实账号、网络或持久化写入。在同一 DOM、同一内容夹具、同一视口状态和同一文字参考中提供控制组 `C0` 与协议定义的 `A1` 至 `A7` 七个单变量消融组；每组只关闭一个预注册约束开关。
- Stage 0 参照视图与 Stage 1 实验视图必须通过互斥根节点、独立状态重置和命名空间样式选择器分离；不强求另一个 HTML 文件或组件。Stage 1 捕获和盲评时不得保留参照图片、竞品名称、来源标签或当前 Atha 实现截图。交付给评估者的截图/任务包、可见 UI 和捕获 DOM 不得暴露语义组名；评估者不获得源码，源码内映射不声称密码学隐藏。
- Stage 1 固定 `360 x 760`、`1000 x 760`、`1600 x 900` 状态，复刻 `A/B/C`、消息线程、作用域和响应式任务所需的最小视觉结构；宽度不小于 `1440px` 时列表状态为 `A+B`，打开 thread 后为 `A+B+C`；提供“专注对话”切换为 `B+C` 并隐藏 `A`，返回列表时隐藏 `C`；`800-1439px` 仍保持 `A+B`/`B+C`。竖屏 note window 必须可关闭回正文，并能从正文重新打开，`B↔C` 切换保持两页；不为某个变体单独调整文案、颜色、间距、断点或内容。
- 生成 Stage 0 校准记录，以及 Stage 1 的静态截图、DOM/几何断言、交互任务记录、盲评映射和停止/不可声称记录，保存到 `artifacts/local/experiments/ui-ablation-20260904/`。
- 版本化变更的允许清单只有本 change、作为本实验指针的 `docs/ACTIVE.md`、四个原型路径和一个媒体资产：`reader/app/ui-ablation.html`、`reader/app/src/ui-ablation/main.ts`、`reader/app/src/ui-ablation/UiAblation.svelte`、`reader/app/src/ui-ablation/ui-ablation.css`、`reader/app/src/ui-ablation/media-preview.png`。夹具内联在 `UiAblation.svelte`，不新增第五个原型文件或其它媒体资产。`media-preview.png` 只用于合成匿名 Telegram 媒体，不进入 Stage 1 盲评。不得修改既有生产 reader 文件。Stage 1 遵守 `artifacts/local/experiments/ui-ablation-20260904/research/ablation-protocol.md` 的 G0/G1 淘汰门、V1-V9 词典序判定、盲评和不变量协议；该协议不包含 Stage 0 参照。

## Non-goals

- 不修改既有生产 reader 文件、`backend/`、`scripts/`、数据模型、MessageStore、生产路由、生产 CSS、依赖、设置 schema 或事实所有者文档；允许的版本化 diff 只包括本 change、仅指向本实验的 `docs/ACTIVE.md` 指针、四个原型路径 `reader/app/ui-ablation.html`、`reader/app/src/ui-ablation/main.ts`、`reader/app/src/ui-ablation/UiAblation.svelte`、`reader/app/src/ui-ablation/ui-ablation.css`，以及唯一媒体资产 `reader/app/src/ui-ablation/media-preview.png`。夹具内联在 Svelte 组件内，除该资产外不新增媒体文件。
- 不修改 `docs/product/OVERVIEW.md` 中现有的 G0/G1、V1-V9 价值排序、构图约束或作用域约束；本 change 只消费它们作为文字参考。
- 不修改或重写前一候选及其验收记录，尤其是 `docs/changes/conversation-visual-language.md`、`docs/changes/notes-responsive-panes.md` 和 `docs/changes/reader-layout-baseline.md`。
- 不把 Stage 0 参照校准当作 Atha 设计、竞品优劣、生产 reader、Windows WebView2、Android、自然触摸、真实输入法、屏幕阅读器、长期疲劳或真实用户价值验收，也不把其中的个人头像、会话内容或长篇受版权文本带入原型。
- 不把 Stage 0 的竞品差异带入 Stage 1；Stage 1 不展示竞品标签/截图或当前实现截图，不新增作者/AI 角色、同步、社交功能或新的价值维度/加权总分。盲评保证只覆盖交付给评估者的截图/任务包、可见 UI 和捕获 DOM：其中不出现 `G0`、`G1`、`V1` 至 `V9`、`A1` 至 `A7` 或其它语义组名；评估者不获得源码，源码映射不声称密码学隐藏。

## Architecture Impact

none

原型可在同一个本地 Vite 入口内承载两阶段，但 Stage 0 与 Stage 1 不共享生产状态、组件、数据写入或布局状态。两阶段使用互斥根节点，切换阶段或实验组时独立重置状态，样式选择器使用各自命名空间；不要求另一个 HTML 文件或组件。新增文件不改变 Atha 的公开 interface、事实所有者或生产构图。

## Acceptance Criteria

- `ABLATION-REF-00`：Stage 0 为每个用户给定截图提供独立的校准视图，忠实范围仅为界面几何、层级、token、排版与交互骨架；Telegram 竖屏视图的大媒体块必须使用 `reader/app/src/ui-ablation/media-preview.png` 这一实际位图，不得以 CSS 绘制或伪造图片替代，且该资产只表示合成匿名媒体，不复制人物或内容。可见文案为合成文字、头像为合法匿名占位状态，不复制个人头像、会话内容或长篇受版权文本。参照视图不出现在 Stage 1 的 DOM、截图索引、盲评包或结果表中。
- `ABLATION-HTML-01`：独立 `/ui-ablation.html` 能在本地 Vite 离线服务器中、无生产 reader 路由和无网络条件下加载；生产 `vite build` 仅用于证明主入口和生产资产未被污染，不要求生成 `dist/ui-ablation.html`。Stage 0 只读本地给定参照，Stage 1 只使用本地合成夹具。
- `ABLATION-HTML-02`：Stage 0 参照根节点与 Stage 1 实验根节点互斥，阶段和组切换有独立状态重置，CSS 选择器有命名空间；不强求另一个 HTML 文件或组件。Stage 1 的 `C0` 与 `A1` 至 `A7` 使用同一 DOM 结构、内容身份、消息关系、主题、字体、初始状态和固定实现，每个变体只有一个 `X_i` 从 `on` 变为 `off`。
- `ABLATION-HTML-03`：Stage 1 所有组都覆盖 `360 x 760`、`1000 x 760`、`1600 x 900` 及协议规定的列表/线程状态；宽度不小于 `1440px` 时列表为 `A+B`、打开 thread 为 `A+B+C`；“专注对话”切换为 `B+C` 并隐藏 `A`，返回列表隐藏 `C`；`800-1439px` 仍为 `A+B`/`B+C`。竖屏 note window 必须可关闭回正文并从正文重新打开，`B↔C` 切换保持两页；不得因变体结果临时更换视口或任务。
- `ABLATION-HTML-04`：Stage 0 校准证据、Stage 1 静态视觉证据、交互任务证据和不可判定项分开记录；Stage 1 截图不替代保存、返回、Locator、焦点、输入器、删除确认或作用域任务。
- `ABLATION-HTML-05`：Stage 1 的 G0/G1 失败立即淘汰变体；硬门不与 V1-V9 相抵消；硬门通过后按 `V1` 至 `V9` 词典序判断，不加权、不平均。
- `ABLATION-HTML-06`：盲评顺序、匿名组映射、不变量校验、停止条件和不可声称结论均保存到 `artifacts/local/experiments/ui-ablation-20260904/`；盲评交付给评估者的截图/任务包、可见 UI 和捕获 DOM 不暴露语义组名，评估者不获得源码；源码内映射不声称密码学隐藏。Stage 1 盲评包不含竞品标签/截图、当前实现截图、真实内容或真实账号。
- `ABLATION-HTML-07`：版本化 diff 仅包含本 change、仅把 `docs/ACTIVE.md` 指向本实验的指针、四个原型路径 `reader/app/ui-ablation.html`、`reader/app/src/ui-ablation/main.ts`、`reader/app/src/ui-ablation/UiAblation.svelte`、`reader/app/src/ui-ablation/ui-ablation.css`，以及唯一媒体资产 `reader/app/src/ui-ablation/media-preview.png`；夹具必须内联在 `UiAblation.svelte`，不得新增第五个原型文件或其它媒体资产。媒体资产只用于合成匿名 Telegram 媒体，不进入 Stage 1 盲评。不得修改生产 reader、价值排序事实所有者或前一候选文档。实施阶段只允许接管并更新该指针。

## Iteration Control

- Mode: `iterative`
- Maximum repair iterations: `2`
- Unchanged-delta limit: `1`
- Human stop: 只有实验不变量、G0/G1 证据或禁止的视觉先验边界需要超出本 change 范围时停止；不得以新增生产功能继续实验。
- Per-iteration receipt: 在本地实验目录记录匿名结果、剩余差异和证据等级；不复制完整日志、正文或截图来源之外的私密材料。

## Files And Steps

1. 在四个允许的原型路径 `reader/app/ui-ablation.html`、`reader/app/src/ui-ablation/main.ts`、`reader/app/src/ui-ablation/UiAblation.svelte`、`reader/app/src/ui-ablation/ui-ablation.css` 新增独立原型入口及本地资源边界，并仅使用唯一媒体资产 `reader/app/src/ui-ablation/media-preview.png`；将夹具内联在 `UiAblation.svelte`，不导入生产 reader 模块，不新增第五个原型文件或其它媒体资产。阶段可共用入口，但必须使用互斥根节点、独立状态重置和命名空间样式，不要求另一个 HTML 文件或组件。
2. 先完成 Stage 0：为用户给定的微信读书、Telegram 截图各建立独立校准视图，忠实范围只限界面几何、层级、token、排版与交互骨架；Telegram 竖屏大媒体块必须使用 `media-preview.png` 实际位图，不得用 CSS 绘制或伪造图片替代；只使用合成文字和合法匿名头像状态，记录校准结果后从 Stage 1 的 DOM、截图索引和盲评包排除。
3. 再完成 Stage 1：按协议固定 `C0` 和 `A1` 至 `A7` 的唯一开关、视口、状态和文字参考；先校验 DOM/内容/几何不变量，并验证宽度不小于 `1440px` 的 `A+B` 列表、`A+B+C` thread、隐藏 `A` 的“专注对话” `B+C` 和返回列表隐藏 `C`；验证 `800-1439px` 的 `A+B`/`B+C` 状态。
4. 运行 Stage 1 静态捕获和最小交互任务，分别记录可视觉判定、需交互判定和本实验不可判定的结果；交互任务必须覆盖竖屏 note window 关闭回正文、从正文重新打开，以及 `B↔C` 保持两页；不把 Stage 0 差异当作 Stage 1 结果。
5. 使用匿名 ID 和平衡盲评顺序写入截图索引、断言摘要、评估表、停止原因和不可声称项；交付给评估者的截图/任务包、可见 UI 和捕获 DOM 不暴露语义组名，评估者不获得源码，源码映射不声称密码学隐藏。
6. 由本 change 接管并仅把 `docs/ACTIVE.md` 指向本实验；揭盲后只报告本夹具、视口和实现下的观察，不回写既有生产 reader、前一候选或其它事实所有者。

## Checks

- Stage 0 的两个参照视图可在本地 Vite 离线入口中分别加载，并能以界面几何、层级、token、排版与交互骨架为范围校准；Telegram 竖屏大媒体块使用唯一的 `media-preview.png` 实际位图，不使用 CSS 绘制或伪造图片；它们只显示合成文字和合法匿名头像状态，不共享 Stage 1 的实验 DOM、夹具、开关或结果表。
- Stage 1 的 `/ui-ablation.html` 在本地 Vite 离线入口中可加载，且无生产状态读写、网络请求或真实数据路径；生产 `vite build` 只证明主入口和生产资产未被污染，不要求 `dist/ui-ablation.html`；版本化 diff 仅包含本 change、`docs/ACTIVE.md` 的本实验指针、四个允许的原型路径和唯一媒体资产 `reader/app/src/ui-ablation/media-preview.png`，不得出现其它生产 reader 文件或媒体资产。
- Stage 1 每组的 DOM/内容/视口/主题/字体/初始状态校验一致，且只有一个 `X_i` 变化；固定视口截图和几何断言覆盖 `360 x 760`、`1000 x 760`、`1600 x 900`，交互任务单独记录状态变化，并验证 `1440px` 以上的 `A+B`/`A+B+C`、`800-1439px` 的 `A+B`/`B+C`、专注对话隐藏 `A`、返回列表隐藏 `C`、note window 关闭/重开及 `B↔C` 两页保持。
- Stage 1 盲评映射在所有记录收齐前不可见；交付给评估者的截图/任务包、可见 UI 和捕获 DOM 不暴露语义组名，评估者不获得源码，源码映射不声称密码学隐藏；盲评包不含 Stage 0 参照或竞品标签/截图；揭盲后按 G0/G1、`V1` 至 `V9` 汇总。
- `git diff` 验证本 change 自身只修改 `docs/ACTIVE.md` 的本实验指针、四个允许的原型路径和唯一媒体资产 `reader/app/src/ui-ablation/media-preview.png`；其它既有生产 reader、前一候选和价值排序事实所有者的工作树改动不属于本 change，不得被回退或计入交付。
- 本文件的 `autocorrect --fix`、`autocorrect --lint` 和 `git diff --check` 通过。

## Result

### Iteration 1

- `iteration`：`1`；
- `review findings`：初版 Stage 0 没有按源视口铺满，Telegram 媒体退化成文件行且列表密度不足；Stage 1 的 C0 仍是同向时间线，不能表达原文与“我”的消息方向；A3 宽屏规则被后续状态规则覆盖；
- `repair`：校准参照根尺寸和列表密度，改用唯一合成位图，补齐原文左侧卡、读者右侧消息、引用块与 composer，并调整 A3 选择器次序；
- `validation cases`：Svelte check、Vite build、Round 2/3 视觉截图和协议探针通过相应修复项；
- `remaining_delta`：宽屏专注对话仍保留空列，Telegram desktop 仍偏稀疏，portrait 后续消息被 composer 截断，A3 在必需宽屏状态仍未形成稳定反事实；
- `evidence level`：静态 / 构建 + Linux 本地 Chromium 截图与 CSSOM 几何；
- `stop reason`：仍有本 change 范围内可修复的 P1，进入第二轮。

### Iteration 2

- `iteration`：`2`；
- `review findings`：Round 3 确认 focus 的 B+C 没有填满宽屏、Telegram 横竖屏密度和裁切仍不稳定、A3 宽屏差异无效；
- `repair`：让 focus 的 B+C 填满视口，补足 desktop 消息密度，portrait 只保留完整媒体与下一条消息，并让 A3 在 list/thread/focus 都形成可见反事实；
- `validation cases`：最终定点视觉复核无 P0/P1；A3/C0 的 1440px 三状态几何、A4 的 360px 预期 hard-fail、C0 的 360/1000/1600 状态与删除取消门禁均已记录；生成 64 张匿名状态图和 8 张 contact sheet，由两名不知道映射且不能读取源码/竞品参照的子 agent 独立评阅后再揭盲；
- `remaining_delta`：A1/A2/A4 的硬门失败是预注册实验结果，不是待修原型缺陷；A6 渐进披露和 A7 消息语法仍因盲评分歧保持“未定”；真实 Tauri/WebKitGTK、Windows、Android、自然触控、IME 和辅助技术未覆盖；
- `evidence level`：静态 / 构建 + Linux 本地 Chromium 固定视口、运行时交互和双盲静态评阅；
- `stop reason`：第二轮已清空原型视觉 P0/P1 并达到修复上限，剩余项属于实验结论或明确证据边界，停止 Repair。

完整报告位于 `artifacts/local/experiments/ui-ablation-20260904/report.md`。本 change 没有修改生产 reader、价值排序事实所有者或前一候选，也不批准发布或生产改造。

## Review

- Standards：独立协议复核确认 Stage 0/1 使用互斥根节点和命名空间样式；原型只导入 Svelte、既有 Lucide 与本地合成位图，不访问生产 reader、MessageStore、真实数据或外部网络。
- Spec：最终定点复核确认 C0 的 1440px `A+B`、`A+B+C`、隐藏 A 的 `B+C`，1000px 的 `A+B`/`B+C`，以及竖屏 B→C→B、关闭和重开；A3 反事实可见，A4 保留预注册 hard-fail。
- Visual：最终目标视口未发现 P0/P1；微信读书和 Telegram 校准图只比较结构、比例、密度和遮挡，不把合成内容差异写成产品结论。
- Blind evaluation：Alpha 与 Beta 的材料不含映射、源码、竞品参照或语义组名；两份记录完成后才揭盲。A3/A5 的共同观察、A6/A7 的分歧和 A1/A2/A4 的交互门已按协议保存。
- Residual：最终报告复核后无 P0/P1；Markdown 未做真实阅读器排版截图，图片中的分面标签只由周围正文补充；这些非阻断项不升级证据等级。

## Evidence And Residual Risks

- Stage 0 的证据只回答“界面几何、层级、token、排版与交互骨架是否完成校准”；Stage 1 的证据来自静态原型、离线交互和盲评记录，不自动升级为真实目标或生产等价证据。
- Stage 0 与 Stage 1 使用互斥根节点，盲评包不含竞品标签、参照截图、当前实现截图、源码或映射。映射只保存在实验目录的 `private/`，盲评完成后才揭示。
- 合成夹具无法代表大量真实书籍、消息规模、跨平台 WebView、自然触摸、真实输入法、屏幕阅读器、长期疲劳或长期价值。
- Stage 0 的复刻不能证明 Atha 应该复制任何竞品交互；Stage 1 中观察到的差异不能回溯归因于微信读书或 Telegram 的独有视觉特征。
- G0/G1 的失败仅在不可持久化夹具中模拟；它们证明协议可以区分同图不同事实，不证明真实生产数据或辅助技术行为。
- `V1-V9` 的词典序在本轮被固定为判定协议，没有与其它价值排序随机对照；A6、A7 的分歧说明部分结果会随评价前提变化。

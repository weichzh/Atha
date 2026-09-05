---
description: 方案二所定义的笔记索引与当前线程视觉语言、范围和验收记录。
---

# 笔记对话视觉语言

## Status

implemented

用户于 2026-09-03 从三份同状态视觉方案中明确选择方案二，本文件将该选择收敛为本轮唯一实施范围。

## Problem

现有 `A / B / C` 响应式结构已经成立，但 `B` 仍像带行内编辑、删除按钮的标注管理器，`C` 仍像同向排列的文档卡片列表。它们没有把产品最重要的“读者围绕一段原文持续回应”呈现为可扫读的对话索引与稳定的消息流。

## Scope

- `B` 以原文作为每段对话的身份，显示最近一条未删除的读者回应与最近活动时间；纯标注没有回应时使用克制空态；
- `B` 移除常驻类型标签和行内编辑 / 删除按钮，当前线程使用细窄强调线和轻背景，不改变搜索、章节筛选、选择、滚动与返回；
- `C` 在当前线程中把原文作为流内起点，不伪装成作者消息；根笔记和后续回复均作为右侧读者消息；
- 回复根消息时不重复展示已在流内可见的原文，回复其他消息时标明“回复”，额外引用标明“引用”；
- 复用现有消息菜单、编辑器、MessageStore、`A / B / C` 响应式状态和竖屏 `B → C → B` 页面关系；
- 删除从低频消息菜单进入，并使用平台确认框阻止误触；正文选区删除沿用同一确认语义。

## Non-goals

- 不实现作者 / AI 身份、头像、参与者、未读、置顶、转发、社交状态或网络同步；
- 不修改 Message、Conversation、SourceAnchor 或 SourceSnapshot 数据模型；
- 不重写消息编辑器、词典、CSS 模块、设置页或既有响应式分栏；
- 不增加依赖、路由、布局设置或第二份对话状态。

## Architecture Impact

present

- 设计目的：让既有 Message 事实在界面上形成“原文 → 读者回应”的稳定语法，而不是 CRUD 管理列表；
- 驱动因素：对话身份可辨、最近活动可扫读、线程关系可理解、危险操作不常驻；
- 受影响 Module：后端 Roots 查询补充最近回应投影，Message client / store 透传该只读字段，Annotations 渲染索引，Conversations 渲染线程，CSS 负责视觉层级；
- Interface / Seam：只扩充 `RootMessageView` 的只读投影，不改变持久化 schema、写入命令或消息关系；
- 替代方案：打开笔记面板时调用已有全书 conversations 查询。该接口当前逐对话加载，列表越大开销越高，因此不采用；
- 证据计划：Node / Rust 投影测试、Svelte check / build、Linux Tauri 360 / 1000 / 1600 px 交互门，以及方案二与实现截图同图 Design QA；
- ADR：不新增。只有参与者角色或跨书实时线程进入产品后才重审消息身份模型。

## Acceptance Criteria

- `VISUAL-B-01`：`B` 的每行按“原文、最近回应、时间”排序呈现；没有“笔记 / 标注”前缀和常驻管理按钮，纯标注显示“还没有回应”；
- `VISUAL-B-02`：当前行的选择线、背景、焦点和文字对比清楚，320 px 宽度内无裁切或水平滚动；
- `VISUAL-C-03`：当前线程首先显示带“原文”标签的流内引用，再按既有顺序显示右侧读者消息；纯标注不生成重复消息气泡；
- `VISUAL-C-04`：回复根消息不重复原文预览；回复非根消息显示“回复”，额外引用显示“引用”，均能跳到当前线程中的目标消息；
- `VISUAL-C-05`：回复、编辑、删除、历史引用、引用关系、跳回原文和固定输入器仍可用；删除取消时不写入，确认后才删除；
- `VISUAL-LAYOUT-06`：360 px 保持同一悬浮窗口中的 `B → C → B`；1000 px 保持 `A+B / B+C`；1600 px 保持从左到右 `A+B+C`；
- `VISUAL-REGRESSION-07`：搜索、章节筛选、对话范围 / 排序、Locator、输入可见性和 AppLog 隐私的既有 Linux 门通过；
- `VISUAL-EVIDENCE-08`：与所选方案二同状态比较后，`design-qa.md` 没有未关闭 P0 / P1 / P2。

## Iteration Control

- Mode: `iterative`
- Maximum repair iterations: `3`
- Unchanged-delta limit: `1`
- Human stop: 只能通过新增参与者数据、真实输入法、自然触摸或真实辅助技术判断才能判定。
- Per-iteration receipt: 本文件的 `Result` 与 `Evidence And Residual Risks`；完整命令和输出由 `project-workflow` 工站保存。

## Files And Steps

1. 在 Roots 只读投影中复用现有消息修订，返回最近未删除回应，不引入额外列表查询。
2. 用现有 Annotations DOM 将 `B` 收敛为连续索引，保留现有交互状态。
3. 用现有 Conversations DOM 将当前原文移入消息流，区分原文、回复和引用，保留消息菜单与编辑器。
4. 用 CSS 匹配方案二的密度、方向、间距和选择态，不改变既有响应式几何。
5. 扩充最小逻辑测试和 Linux 真壳断言，完成同图 Design QA、事实同步和独立 review。

## Checks

- `cargo test --locked -p atha-backend message_reading`；
- `mise exec -- node --test reader/web/conversations.test.mjs`；
- `mise exec -- pnpm --dir reader/app check`；
- `mise exec -- pnpm --dir reader/app build`；
- `bash scripts/check-reader-linux.sh --gesture-warmups 0 --gesture-measurements 1` 作为迭代门；
- `bash scripts/check-reader-linux.sh` 作为最终 Linux 真壳门；
- 360、1000 和 1600 px 当前截图、DOM 几何与方案二同图 Design QA；
- 本次中文 Markdown 的 AutoCorrect、required docs gate 与 `git diff --check`。

## Result

实现只扩充既有 Roots 只读投影和 `B / C` 渲染，没有增加依赖、schema、路由或第二份对话状态：

- Roots 查询从 Conversation 中投影最近一条未删除消息的 `plain_text`；`B` 以原文、最近回应和活动时间形成连续索引，纯标注显示“还没有回应”，类型前缀与行内管理按钮已经移除；
- `C` 以可跳回的流内原文为起点，根笔记与后续回复均作为右侧读者消息；回复根消息不重复原文，回复其他消息显示“回复”，额外引用显示“引用”；
- 现有消息菜单、编辑器、范围 / 排序、历史、关系和跳回原文保持不变；列表选择删除与消息菜单删除都先请求平台确认；
- 竖屏继续使用同一悬浮窗口中的 `B → C → B`，中等横屏继续在 `A+B / B+C` 间切换，1600 px 继续从左到右显示 `A+B+C`。

### Iteration 1

- `review findings`：原文块在短视口中参与纵向收缩而裁切；原文移入消息流后，旧四行 overlay 网格又让当前范围的内容区占到自动行，输入器停在窗口中部；首份截图没有覆盖非根消息回复预览。
- `repair`：原文块和消息卡退出纵向收缩；内容区固定进入 overlay 最后一行，旧原文摘要行在当前范围折叠、在聚合范围保留；视觉夹具建立嵌套回复。
- `validation cases`：360 px 悬浮线程、1000 / 1280 px `B+C`、1600 px `A+B+C` 的原文、消息、回复预览、输入器和无裁切检查通过。
- `remaining_delta`：需要把所选方案与最终真壳截图置于同一输入复核层级和密度。
- `evidence level`：Linux Tauri / WebKitGTK 真壳自动化与 GUI 截图。
- `stop reason`：仍有可在当前范围内完成的视觉对照，继续第二轮。

### Iteration 2

- `review findings`：方案二与实现的正文顶栏、分栏比例和筛选位置不同，但这些属于既有阅读壳和已明确保留的响应式几何，不是本轮 `B / C` 视觉语言缺陷；当前范围没有新的 P0、P1 或 P2。
- `repair`：不为追逐静态示意图改写既有功能或分栏，只记录范围边界；保留通过交互门的当前实现。
- `validation cases`：1600 px 方案 / 实现同图与 360 px Telegram 参照 / 实现同图复核通过；完整 Linux 门覆盖 360、600、1000、1280、1600 px 和 220 次指针测量。
- `remaining_delta`：本轮范围内无剩余差异；设置界面另立范围。
- `evidence level`：静态 / 构建 + Linux Tauri / WebKitGTK 真壳 + 同图 Design QA。
- `stop reason`：全部验收条件通过，未达到第三轮即停止。

## Review

以工作流基线 `ae7287d08c8f0d92ec4430cef9e3b1ea3230c217` 对完整 diff 执行 Standards 与 Spec 两轴复核。

- `Standards`：无 blocking 发现。实现复用 MessageStore、Annotations、Conversations、原生确认框和现有语义 token；Roots 的相关子查询使用既有 `(conversation_id, ordinal)` 唯一索引，没有增加查询循环、依赖或持久化事实。
- `Spec`：无 blocking 发现。`VISUAL-B-01 / 02`、`VISUAL-C-03 / 04 / 05`、`VISUAL-LAYOUT-06`、回归和同图证据条件均有自动化或截图证据；复核发现的聚合范围网格风险已通过固定内容行和正式断言修复。
- `non-blocking`：截图夹具是合成长列表，能证明密度和关系层级，不能代替大量真实笔记的长期扫读观察。
- `out-of-scope`：没有实现作者 / AI 身份、头像、主题选择、自由列宽或设置页重组。

## Evidence And Residual Risks

- 静态 / 构建：Message Node 测试通过；Rust `message_reading` 为 22 passed / 1 fixture ignored；Svelte check 为 0 errors / 0 warnings；Vite production build、Rust fmt、Bash 语法、AutoCorrect、文档门和 `git diff --check` 通过。
- Linux 真壳：完整 `bash scripts/check-reader-linux.sh` 通过，WebDriver 报告 WebKitGTK capability `0.55.1`。`B` 最近回应、流内原文、右侧消息、回复标签、删除取消、聚合范围布局、输入器贴底、竖屏两页、横屏两 / 三栏、Locator、焦点、输入可见性和 AppLog 隐私均进入正式断言。
- 指针测量：13 个场景、220 次测量通过；input-to-first-visual p95 约 32 ms，tap p95 约 9 ms，最大连续帧间隔 18 ms，release-to-stable p95 约 352 ms。这是 WebKit 主线程鼠标 Pointer Actions，不是自然触摸或 compositor FPS。
- GUI / Design QA：`artifacts/local/audits/conversation-visual-language-20260904/` 保存最终 Linux 截图、`uiux-evidence.json`、横屏与竖屏同图对照及 `design-qa.md`；没有未关闭的 P0、P1 或 P2。
- 残余风险：未执行 Windows WebView2、PCT-AL10 自然手指触摸、真实输入法或屏幕阅读器验收；Linux 截图和鼠标自动化不替代这些证据。

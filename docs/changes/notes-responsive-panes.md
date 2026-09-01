---
description: 笔记列表、消息线程与正文的响应式分栏和视觉验收记录。
---

# 笔记响应式分栏

## Status

implemented

## Problem

Atha 已经把笔记存为可回复的 Message，但当前视觉结构仍把“笔记列表”、“阅读正文”和“当前消息线程”当成三个互相替代的全屏界面。它有三个用户可见问题：

- 横屏已能并排正文与笔记列表，但进入某条笔记后线程又覆盖整个窗口，列表与当前选择丢失；
- 竖屏笔记列表是全屏 panel，当前线程是另一个半屏 sheet，两者不像同一笔记窗口中的前后页；
- 线程内容仍是带分割线的文档列表，消息块、原文上下文、回复操作和输入器没有组成稳定的即时通讯阅读顺序。

用户明确的目标是常规的响应式 master-detail，而不是为每个尺寸发明一套界面：

- `A` 是阅读正文；
- `B` 是笔记 / 对话列表；
- `C` 是当前消息线程。

竖屏在同一悬浮笔记窗口中以两页完成 `B → C → B`；横屏按可用宽度和当前任务投影为 `A+B`、`B+C` 或 `A+B+C`。字母同时定义从左到右的空间顺序，不能把正文夹在笔记列表与当前线程之间。

## Scope

- 将笔记布局能力改为“横向且至少 800 CSS px”，不再只用 1100 px 桌面宽度猜测设备类型；
- 竖屏和不足 800 px 的窄横屏把笔记显示为书页上的悬浮窗口；列表和线程共用同一几何，打开线程后返回只回到列表页，不先关闭笔记窗口；
- 800–1439 px 横屏在未选线程时从左到右显示 `A+B`，进入线程后改为 `B+C`；
- 至少 1440 px 的横屏在进入线程后从左到右显示 `A+B+C`，关闭线程回到 `A+B`；
- 复用现有 notes `details`、Conversation overlay、MessageStore、MessageComposer 和 Navigation；只保留当前选中线程的视觉状态，不增加路由、持久化布局设置或另一份消息状态；
- 把笔记列表收敛为可扫读的对话索引，当前行使用既有主题 token 高亮；把线程消息收敛为无嵌套卡片的紧凑气泡流，保留用户已确认的“引用在正文上方，回复和更多在消息下方”；
- 扩充 Linux Tauri 真壳门与 GUI 对照，验收竖屏两页、中等横屏两栏切换、宽横屏三栏、选中状态、返回、输入和 Locator。

## Non-goals

- 不实现 AI 作者、参与者身份、头像、群组、未读数、置顶、转发、共享或网络实时状态；
- 不伪造“作者来信”：当前数据没有发送者角色，原文引用与用户消息仍按真实数据呈现；
- 不改 Message / SourceAnchor / SourceSnapshot schema、MessageStore、富文本编辑器、词典或 CSS 模块；
- 不在本切片统一“笔记”、“标记”、“阅读对话”等用户术语；
- 不把 Telegram 的颜色、品牌、频道、群组或社交功能复制到 Atha，只采用已经成熟的列表 / 线程层级、消息密度和返回关系。

## Architecture Impact

present

- 设计目的：让“与书对话”的 Message 模型在界面中呈现为稳定的索引 / 线程主循环，同时保留正文上下文；
- 驱动因素：横竖屏信息密度、列表位置记忆、当前选择可见性、键盘 / 输入法遮挡和 Locator 稳定；
- 受影响 Module：App 拥有 viewport 工作区投影，Annotations 拥有列表与当前选择投影，Conversations 拥有线程打开 / 关闭与消息 DOM，CSS 仅按既有状态组合分栏；
- Interface / Seam：复用 `details[open]`、overlay `hidden`、CSS media query / `:has()`、当前 reader workspace 和 `annotations.selectConversation()` 视觉投影；不改 IPC 或数据 interface；
- 替代方案：新建一个 Svelte 三栏页并复制列表 / 线程状态。它会制造第二份 Message 主循环和跨 DOM 同步，因此不采用；
- 证据计划：使用用户提供的 Telegram 竖屏列表 / 线程截图与当前 Atha 截图作为源图，在 360、1000、1280 和 1600 px Linux Tauri 真壳中复核几何、导航、选中、输入和无溢出；
- ADR：不新增。若以后引入多角色、多书线程或可持久布局编辑，再重新研究状态所有权。

## Acceptance Criteria

- `NOTES-PORTRAIT-01`：360 × 760 中笔记是书页上有可见外边的悬浮窗口；列表页与线程页共用边界，打开线程后返回回到原列表滚动位置，再返回才关闭笔记。
- `NOTES-LANDSCAPE-02`：1000 × 760 打开笔记时从左到右是 `A+B`；进入线程后从左到右是 `B+C`，两栏填满可见区、边界无重叠且当前列表行可见。
- `NOTES-WIDE-03`：1600 × 900 进入线程后从左到右是 `A+B+C`；`B` 为稳定 320 px，`C` 在 440–520 px 之间，`A` 使用剩余宽度且仍有完整正文与 Locator。
- `NOTES-THREAD-01`：线程保留原文上下文、消息正文、直接引用、时间、回复 / 更多和固定输入器；消息气泡不嵌套卡片，360 与 1600 px 不裁切正文、菜单或发送按钮。
- `NOTES-STATE-01`：进入、切换和关闭线程时只有一个列表行为当前项；列表滚动位置不因页面切换重置。
- `NOTES-INPUT-01`：线程与笔记窗口的可见底部跟随 VisualViewport；现有紧凑输入、展开编辑、Markdown / 可见模式和发送不改数据语义。
- `NOTES-LOCATOR-01`：打开 / 关闭笔记、进入 / 退出线程及在两栏 / 三栏间重排后，章节和可解析 Locator 不变，没有终止错误。
- `NOTES-REGRESSION-01`：搜索、目录、标注、对话范围 / 排序、回复、编辑、删除、历史引用、跳回原文、导出、键盘和 AppLog 隐私的既有 Linux 门通过。
- `NOTES-EVIDENCE-01`：将参考图与相同状态的最新 Linux 截图并置评审，`design-qa.md` 没有未关闭 P0 / P1 / P2；截图不代替交互门。

## Iteration Control

- Mode: `iterative`
- Maximum repair iterations: `3`
- Unchanged-delta limit: `1`
- Human stop: 只能通过新消息角色 / 数据 schema、第二套线程状态、真实输入法或平台自然触摸才能判定。
- Per-iteration receipt: change 的 `Result` 与 `Evidence And Residual Risks`；完整命令与输出由 `project-workflow` 工站保存。

## Files And Steps

1. 用现有 viewport media 与 workspace 状态定义 `A / B / C` 几何，不新建页面容器。
2. 打开线程时保留已打开的 notes `details`，让关闭线程恢复列表；投影当前对话行而不重建列表。
3. 用 CSS 重组悬浮窗口、两栏 / 三栏和消息气泡，保留现有主题、图标与输入器。
4. 扩充 Linux 门的竖屏页面返回、横屏 pane 几何、选中和 Locator 断言，采集对应截图。
5. 更新事实所有者，以参考 / 实现同图完成 Design QA 和独立 review。

## Checks

- `mise exec -- pnpm --dir reader/app check`；
- `mise exec -- pnpm --dir reader/app build`；
- `mise exec -- node --test reader/web/conversations.test.mjs`；
- `bash scripts/check-reader-linux.sh --gesture-warmups 0 --gesture-measurements 1` 作为迭代门；
- `bash scripts/check-reader-linux.sh` 作为最终 Linux 真壳门；
- 360、1000、1280 和 1600 px 的当前 Linux GUI 截图与 DOM 几何；
- 参考图与实现图同状态 Design QA；
- 本次中文 Markdown 的 `autocorrect --fix` 与 `autocorrect --lint`；
- `project_workflow.py station notes-responsive-panes-20260901 --activity verification --gate docs`；
- `git diff --check`。

## Result

实现复用了既有 notes `details`、Conversation overlay、MessageStore、MessageComposer、Navigation 和 CSS media query，没有增加路由、依赖、持久化字段或第二份消息状态。当前结果为：

- 竖屏列表 `B` 与线程 `C` 共用 12 px 外边距的悬浮窗口，以 `B → C → B` 切页；返回保留当前行、焦点和列表滚动位置；
- 800–1439 px 横屏在从左到右的 `A+B` 与 `B+C` 间切换；至少 1440 px 的线程态从左到右显示 `A+B+C`；
- 宽屏 `B` 固定 320 px，`C` 使用 440–520 px，正文 `A` 使用剩余宽度；窗口变化和线程开关通过既有 resize / Navigation 队列保持 Locator；
- 列表投影当前线程行，行内低频操作使用既有 Fluent 图标；线程使用紧凑消息气泡、原文上下文、固定输入器和既有回复 / 更多关系。

### Iteration 1

- `review findings`：初版竖屏列表与线程几何已经统一，但正式 Linux 门沿用了不保留 session 的 resize helper，并把 Tauri 窗口装饰后的 `innerHeight` 当成启动高度，导致验收脚本误报。
- `repair`：在同一 WebDriver session 中直接设置窗口 rect，几何断言读取真实 `innerWidth / innerHeight`；不放宽产品条件。
- `validation cases`：360 px `B → C → B`、1000 / 1280 px 两栏、选中、焦点、滚动和 Locator 进入门禁。
- `remaining_delta`：宽屏三栏的 reader frame 仍需视觉复核。
- `evidence level`：Linux Tauri / WebKitGTK 真壳自动化。
- `stop reason`：存在当前范围内可修复的宽屏差异，继续第二轮。

### Iteration 2

- `review findings`：1600 px 时 reader frame 仍受根 grid 居中，正文可能伸入线程区域；竖屏返回焦点只框住行内主按钮，列表操作仍用文字。
- `repair`：让 frame 在分配列内从起点布局，焦点环覆盖完整列表行，并复用既有 Fluent 编辑 / 删除图标。
- `validation cases`：1600 px 三栏边界、正文 Locator、360 px 返回焦点和参考 / 实现同图均通过。
- `remaining_delta`：三栏顺序尚需按用户给出的 `A / B / C` 语义复核。
- `evidence level`：Linux 真壳几何 + GUI 截图 + 同图 Design QA。
- `stop reason`：用户评审发现空间顺序仍有 blocking 差异，继续第三轮。

### Iteration 3

- `review findings`：首轮把通用聊天应用的“列表在左”惯例套到 Atha，得到 `B+A+C`；用户明确指出正文不应位于中间。根因是评审偏向通用 master-detail 范式，低估了用户已规定的字母顺序。
- `repair`：横屏严格按从左到右 `A+B`、`B+C`、`A+B+C` 排列；同步修正正式门、截图门和事实所有者。
- `validation cases`：完整 Linux 门通过 360 / 600 / 1000 / 1280 / 1600 五个宽度；1600 px 实测为 `A 0–800 / B 800–1120 / C 1120–1600`，13 个手势场景共 220 次测量通过。
- `remaining_delta`：本切片范围内无 blocking 差异；用户术语留给独立 change。
- `evidence level`：静态 / 构建 + Linux Tauri / WebKitGTK 真壳 + 当前 GUI 截图和 DOM 几何。
- `stop reason`：验收条件通过，达到预设三轮上限且没有 blocking `remaining_delta`。

## Review

以基线 `760ba881a2425257db2201e8579d78773deccb08` 对完整 diff 执行 Standards 与 Spec 两轴复核。

- `Standards`：无 blocking 发现。实现保留单 WebView、现有 `details` / overlay、MessageStore 与 Navigation 所有权；CSS 负责响应式几何，JS 只保留当前选择和触发既有 resize 队列；没有增加依赖、schema、设置或第二套状态。
- `Spec`：无 blocking 发现。`NOTES-PORTRAIT-01`、`LANDSCAPE-02`、`WIDE-03`、`THREAD-01`、`STATE-01`、`INPUT-01`、`LOCATOR-01`、回归和证据条件都有执行证据；用户发现的 `B+A+C` 顺序错误已经修复并加入正式断言。
- `non-blocking`：当前夹具只有一个根笔记；滚动位置保持有自动化证据，但大量真实线程的摘要密度仍需长期书库观察。
- `out-of-scope`：没有伪造作者、头像、左右角色、AI、未读、拖拽列宽或布局设置；用户术语未在本切片统一。

## Evidence And Residual Risks

- 静态 / 构建：`pnpm check` 为 0 errors / 0 warnings，Vite production build、`cargo build --locked -p atha-reader-app`、`node --test reader/web/conversations.test.mjs`、Bash 语法、文档门、AutoCorrect 和 `git diff --check` 通过。
- Linux 真壳：完整 `bash scripts/check-reader-linux.sh` 通过；WebDriver 报告 WebKitGTK capability `0.55.1`，系统包取证为 WebKitGTK `2.52.6`。竖屏两页同边界，1000 / 1280 px 的两栏切换、1600 px 的 `A+B+C`、当前选择、焦点、Locator、输入可见性和 AppLog 隐私均通过。
- 性能回归：13 个既有手势场景、220 次测量通过；最大连续帧间隔 17 ms，release-to-stable p95 为 352 ms。它是 WebKit 主线程自动化指针证据，不是实体触摸或 compositor FPS。
- GUI / Design QA：`artifacts/local/audits/atha-notes-responsive-panes-20260901/` 保存当前截图和 `uiux-evidence.json`；参考列表 / 线程与实现相同状态已在同一视觉输入中比较，`design-qa.md` 没有未关闭 P0 / P1 / P2。
- 残余风险：未执行 Windows WebView2、PCT-AL10 自然手指触摸、真实输入法、屏幕阅读器或大量真实笔记长期使用；Linux 截图和鼠标 Pointer Actions 不替代这些证据。

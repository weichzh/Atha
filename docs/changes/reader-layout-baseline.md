---
description: 默认阅读版面的 DPR 几何、正文尺度、工具退场与验收记录。
---

# 默认阅读版面基线

## Status

implemented

## Problem

用户已把 UI/UX 优化设为当前阶段目标，并明确要求以微信读书式安静阅读作为阅读层参照。当前 Linux Tauri 真壳在 `devicePixelRatio = 2` 时暴露三个相互关联的默认版面问题：

- `standard = 32` 的左右边距直接写入内部设备像素，显示后只有 16 CSS px；
- 正文宽度跟随 reader frame 无限增长，1024 px 窗口约 52 个汉字一行，1600 px 窗口在 320 px 目录旁仍约 66 个汉字一行；
- 视口达到 1100 px 后自动打开目录工作区并永久显示工具，窗口宽度擅自改变了阅读模式。

这些不是缺少更多设置，而是默认构图没有承担普通读者的阅读质量。本切片只修复共同根因，不同时重做字面、对话、词典、CSS 或消息编辑器。

## Scope

- 把本书三档左右边距解释为稳定的视觉 CSS px，再换算到阅读内核使用的设备像素；
- 在分页和滚动两种模式中，把默认单栏正文限制为约 760 CSS px 并在当前 reader frame 内居中；窄视口继续由可用宽度和所选边距自然收缩；
- 保留 1100 px 桌面工作区能力，但宽屏初始只显示正文；目录、搜索和书中对话由用户明确打开，关闭后保持关闭，跨宽度变化不自动重开；
- 工具出现、关闭、工作区切换和窗口重排继续通过既有 Navigation / Locator 恢复正文位置；
- 扩充 Linux Tauri 正式门，记录 DPR、可见边距、正文宽度、默认专注状态、显式工作区与窄屏回归；
- 更新产品、路线图、阅读内核和代码地图中的当前事实。

## Non-goals

- 不新增正文宽度、桌面模式或工具自动隐藏设置；
- 不实现双页、纸张卡片、拟真翻页、字体下载或语言猜测；
- 不改变书源样式开关、段首缩进、段距、字号、行距或 CSS 模块 schema；
- 不改对话消息流、用户术语、`MessageComposer`、词典面板或 MessageStore；
- 不把 Linux WebKitGTK 结果称为 Windows、Android、自然触摸或真实输入法验收。

## Architecture Impact

present

本切片不改变后端、数据、信任边界、依赖或公开 interface，但会改变阅读布局长期质量门。

- 设计目的：让单栏阅读尺度和边距由阅读需求而不是窗口剩余宽度或 DPR 偶然决定，并让宽屏工作区成为显式任务模式；
- 驱动因素：长时阅读舒适、跨 DPR 视觉一致性、Locator 稳定和普通读者的沉浸默认；
- 受影响 Module：`Preferences` 负责视觉边距到设备像素的换算，`Pagination` 负责 reader frame 与正文最大尺度，`App` 负责工作区显隐，CSS 只投影这些状态；
- Interface / Seam：沿用现有 CSS custom properties、Navigation resize 队列和 `details` panel；不增加 IPC、数据字段或第二套状态；
- 替代方案：限制整个 reader frame 或新增“正文宽度”设置。前者会把书页做成浮动卡片并改变工具/手势几何，后者把默认责任推回用户，均不采用；
- 证据计划：Linux Tauri 固定视口中的 DOM 几何、显式工作区交互、Locator 前后值、console/AppLog 隐私和当前正式门；
- ADR：不新增。若以后引入双页、多栏或持久桌面模式，再重新研究布局模型并决定是否需要 ADR。

## Acceptance Criteria

- `LAYOUT-DPR-01`：在 DPR 2 的 Linux 真壳中，`standard` 左右边距显示为 `32 ± 1` CSS px；字号仍按既有逻辑 CSS px 解释。
- `LAYOUT-MEASURE-01`：工作区关闭时，1000、1280 和 1600 px 视口的正文可见宽度不超过 760 CSS px，并在当前 frame 内误差 1 px 地居中；360 px 视口无横向溢出且正文宽度至少 280 CSS px。
- `LAYOUT-FOCUS-01`：1280 和 1600 px 启动时具备桌面工作区能力，但没有活动 workspace panel，reader frame 占满窗口且工具不可见；中心点击可显示工具，显式打开目录后才出现 320 px 工作区，关闭或 Escape 后保持专注阅读。
- `LAYOUT-LOCATOR-01`：记录章节与当前 Locator 后，依次打开/关闭目录、切换搜索与书中对话、在 1280 和 1600 px 间重排；章节不变、Locator 仍可解析且正文无终止错误。
- `LAYOUT-NARROW-01`：360 和 600 px 继续使用覆盖式工具面板；正文、目录、搜索、书中对话、设置和中心点击没有遮挡、横向溢出或状态分叉。
- `LAYOUT-REGRESSION-01`：分页/滚动、书源样式、CSS 模块、目录、搜索、消息入口、内容查看器、手势和 AppLog 隐私的既有 Linux 门通过。
- `LAYOUT-EVIDENCE-01`：保存至少 360、1000、1280 和 1600 px 的当前 Linux GUI 截图与 DOM 几何；截图只证明当前真壳呈现，不替代交互链路或其他平台验收。

## Iteration Control

- Mode: `iterative`
- Maximum repair iterations: `2`
- Unchanged-delta limit: `1`
- Human stop: 正文尺度或 Locator 需要修改数据模型、第二套布局状态，或只能由 Windows / Android 自然输入判断。
- Per-iteration receipt: change 的 `Result` 与 `Evidence And Residual Risks`，完整命令结果由 `project-workflow` 工站保存。

## Files And Steps

1. 在 Preferences 写入左右边距前按当前 DPR 换算，不改变持久偏好枚举。
2. 在 Pagination 同步 viewport 时写入 760 CSS px 对应的内部最大正文宽度，由 CSS 同时投影分页和滚动布局。
3. 让桌面工作区默认关闭，并允许中心点击、显式 panel、关闭与 Escape 形成可逆状态；不增加持久化。
4. 更新 Linux 真壳门的默认专注状态、几何、显式工作区、Locator 和窄屏断言。
5. 更新事实所有者并采集前后证据。

## Checks

- `mise exec -- pnpm --dir reader/app check`；
- `mise exec -- pnpm --dir reader/app build`；
- `bash scripts/check-reader-linux.sh --gesture-warmups 0 --gesture-measurements 1` 作为迭代门；
- `bash scripts/check-reader-linux.sh` 作为最终 Linux 真壳门；
- 当前 UI/UX 专项截图与 DOM 几何复核；
- 本次中文 Markdown 的 `autocorrect --fix`、`autocorrect --lint`；
- `project_workflow.py station reader-layout-baseline-20260901 --activity verification --gate docs`；
- `git diff --check`。

## Result

实现沿用了既有 Preferences、Pagination、Navigation、`details` panel 和 CSS custom properties，没有新增依赖、持久化字段或第二套布局状态。当前结果为：

- 24 / 32 / 48 页边距按视觉 CSS px 解释，由 Preferences 乘当前 DPR 写入内部画布，并在 resize 时重新同步；
- 分页和滚动模式共用最多 760 视觉 CSS px 的居中单栏，360 px 窄视口依边距收缩为 296 px；
- 1100 px 以上仅声明桌面工作区能力，启动不再自动打开目录；中心点击显示工具，用户显式打开目录、搜索或笔记后才分出 320 px 工作区，Escape 恢复专注阅读。

### Iteration 1

- `review findings`：首次迭代门使用了未在 Linux 固定视口白名单中的 `360x800`；实现本身没有越界，但验收入口与现有真壳契约不一致。
- `repair`：改用既有 `360x760`，不扩展测试专用尺寸契约。
- `validation cases`：迭代 Linux Tauri 门通过；1280 / 1600 默认专注、显式工作区、1000 居中正文、360 / 600 窄屏、Locator 和键盘路径均通过。
- `remaining_delta`：需要把滚动模式和 DPR 变化后的边距重同步纳入同一质量门。
- `evidence level`：Linux Tauri / WebKitGTK 真壳自动化。
- `stop reason`：存在可在当前范围内修复的剩余差异，继续第二轮。

### Iteration 2

- `review findings`：滚动模式尚未直接断言单栏尺度；页边距只在 Preferences `apply()` 时换算，监视器 / DPR 改变后可能保留旧值；change 初版的 `Architecture Impact` 和 `Iteration Control` 元数据也未满足 doc guard 的固定格式。
- `repair`：增加共用 `syncPageMargins()` 并复用 resize 事件；在 Linux 门显式切换到滚动模式复核 760 px 居中单栏；按现有 change 协议修正文档元数据。
- `validation cases`：`pnpm check` 为 0 errors / 0 warnings，Vite build 通过；迭代门与完整 `scripts/check-reader-linux.sh` 均通过，后者完成 13 个手势场景、220 次测量；文档门、AutoCorrect 和 `git diff --check` 通过。
- `remaining_delta`：默认阅读版面范围内无 blocking 差异；笔记的响应式视觉结构属于下一个 change。
- `evidence level`：静态 / 构建 + Linux Tauri / WebKitGTK 真壳 + 当前 Linux GUI 几何截图。
- `stop reason`：验收条件通过，达到预设两轮上限且 `remaining_delta` 中无 blocking 项。

## Review

以基线 `ebbc020d91e35ba2f0bd8bbe442f3c18cc15fcb4` 对完整 diff 执行 Standards 与 Spec 两轴独立复核。

- `Standards`：无 blocking 发现。改动保留单 WebView、单 Navigation 队列、现有 Preferences / Pagination 所有权和内容安全边界；使用 CSS `min()` / custom properties、原生 `matchMedia`、resize 事件和现有 `details`，未增加推测性抽象、设置或依赖。
- `Spec`：无 blocking 发现。`LAYOUT-DPR-01`、`MEASURE-01`、`FOCUS-01`、`LOCATOR-01`、`NARROW-01` 和回归门均有执行证据；视觉证据保留 360、1000、1280 和 1600 px 状态。
- `non-blocking`：正式 WebDriver 门为避免 WebKitGTK 运行时 resize 不生效，使用固定尺寸真壳 session 对比恢复后 Locator；专项截图脚本另有实时窗口缩放的视觉证据，但尚不是同一自动化链路中的连续 Locator 证明。
- `out-of-scope`：笔记窗口和消息线程仍是现有外观；已在 roadmap 登记为后续独立切片，不在本次扩展。

## Evidence And Residual Risks

- 静态 / 构建：`mise exec -- pnpm --dir reader/app check`、`mise exec -- pnpm --dir reader/app build`、`bash -n scripts/check-reader-linux.sh`、文档门、AutoCorrect 与 `git diff --check` 通过。
- Linux 真壳：完整 `bash scripts/check-reader-linux.sh` 通过；WebDriver 报告 `WebKitGTK 0.55.1`，DPR 2 下可见标准边距 32 px，正文宽度为 1280:760、1600:760、1000:760、360:296，分页与滚动尺度断言通过，Locator、键盘、窄屏和 AppLog 隐私无回归。
- GUI 几何：`artifacts/local/audits/atha-reader-layout-baseline-20260901/uiux-evidence.json` 与同目录截图记录系统 WebKitGTK 2.52.6；1280 / 1000 / 1600 正文分别居中于 `left = 260 / 120 / 420`，移动正文 `left = 32`。
- 残余风险：未执行 Windows WebView2、Android / PCT-AL10 自然触摸、真实输入法和屏幕阅读器验收；Linux 鼠标 Pointer Actions 与截图不代替这些证据。截图只证明当前 Linux 真壳呈现，不证明长期阅读偏好。

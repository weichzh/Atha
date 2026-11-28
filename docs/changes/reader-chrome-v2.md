# 阅读控件与沉浸状态 V2

## Status

implemented

## Problem

顶底控件悬浮在首末行上方。隐藏控件后没有章节和阅读状态；极简控件、上滑笔记、跟手书签和复制结果尚未从本地 V2 工作区交付。

## Scope

从用户已批准的 V2 和移动体验改动中拆出阅读 UI 和「阅读与翻页」全局设置入口。初始基底 `3044c93` 含 Android 桥，最终基底为 #19 合入后的 `a0a140c`；关联 [Issue #1](https://github.com/weichzh/Atha/issues/1)。不改书架、既有设置项的导航、正文净化、会话/Locator 持久化或翻页渲染。不操作设备。

## Architecture Impact

present

沿用既有输入所有权判断，分页触摸的垂直动作才交给阅读控件，不在阅读 UI 上并行识别横拖、选区或长按。稳定的顶底保留区不因控件显隐重分页。电量只在 Android 受限同源主框架消息桥读取，不使用 JavaScript interface 或修改 CSP；无真实读数不显示。相比移植整份 V2 ReaderView/engine，这保留后续合入的脚注、连续滚动、分页、笔记和安全区实现。

## Acceptance Criteria

- 经典模式保留显式目录、笔记、书签、样式和前后页；极简模式提供返回、目录、三点选项、章节切换和全书进度。不强制修改单书阅读方式。
- 隐藏控件时保留章节、时间、受支持设备的实际电量、页码和进度；点章节开目录。窄屏目录与样式在安全区内贴底，顶底控件不遮挡正文。
- 分页下拉书签跟手、阈值预览亮灭、正常释放提交、反向撤回及取消不提交。极简分页上滑笔记有预览反馈，同一手势不触发翻页；滚动和选区保留其原有输入意义。
- 复制成功与失败均轻提示，失败可重试；开书、回书架、笔记焦点、脚注、目录和返回优先级保留。
- 应用设置可搜索并调整阅读控件、下拉书签、极简分页上滑笔记和分页动效；连续滚动仍由本书设置控制。

## Files And Steps

1. `ReaderView.svelte` 和阅读设置、选区浮层：控件与保留区、状态、复制反馈及窄屏布局。
2. `interaction.mjs` 和 ReaderEngine：复用现有输入仲裁，将垂直手势回调和明确切章接给阅读 UI。
3. Android 端及 `system-bars.ts`：在 #17 的权限约束内加只读电量请求。
4. `AppSettingsView.svelte`：在既有分组中增加「阅读与翻页」，复用现有 UI 偏好字段。
5. 浏览器、交互单测、静态门及可用工具链中的 Kotlin 编译。

## Iteration Control

- Mode: `iterative`
- Maximum repair iterations: `3`
- Unchanged-delta limit: `1`
- Human stop: 测试需要安装或操作设备时停止，保留实际证据和缺口。

## Checks

- `mise exec -- pnpm --dir reader/app check`、`build` 和 `mise exec -- node scripts/test-node.mjs` 通过；160 项 Node 单测含 52 项输入仲裁。
- Chromium 390 × 844（DPR 2、触摸）及 1280 × 820：`reader-chrome.browser.mjs` 覆盖稳定保留区、章节入口与等宽贴底浮层、模式切换、三点菜单退出顺序、切章、书签拉绳跟手、撤回/取消、极简上滑、连续滚动、选区复制与失败保留选区。已查看实际截图。
- `reader-app.browser.mjs`、`page-turn.browser.mjs` 和 `android-reader-insets.browser.mjs` 通过，覆盖开书到笔记与原文回跳、目录与脚注、WebGL/Canvas2D/DOM 翻页、Locator 恢复和 Android 桥模拟。模拟端口覆盖可用、无读数和超范围电量，不把模拟值视为 Android 实测。
- 在 #19 合入基底上，Svelte、Vite 和上述四项 Chromium 套件通过。390px 设置页实际搜索并打开「阅读与翻页」，四项选择刷新后保持；从书架重新打开，正文显示极简控件并使用全局覆盖翻页。旧 `scroll` 值在全局动效中显示滑动，不成为全书滚动开关。
- 本地 Android SDK 36 / Temurin 21，Gradle 离线 `:app:compileX86_64DebugKotlin :app:compileX86_64ReleaseKotlin` 通过。
- `bash scripts/check-docs.sh`、`bash scripts/check-workflow.sh` 和 `git diff --check` 通过。Markdown 走全局提交钩子。

## Result

- 经典保留按钮与全书进度；极简将书签、笔记和样式归入三点菜单，底栏可跨章。选择阅读控件不写本书阅读模式，连续滚动仍可通过菜单打开笔记。
- 正文在顶底保留区之间分页或滚动；两种控件和沉浸态不移动正文。打开目录/样式用相同的安全区内贴底浮层，桌面居中。章节首个目录锚点在当前读位之后时，天头显示当节第一个目录标题，不移动读位。
- 垂直动作由原有触摸仲裁分派，取消、撤回和抢占不落书签。Android 仅在沉浸状态轮询同源桥的 `battery:read`；原生查询返回百分比或空值，桌面和无读数时省略该项。
- 应用设置使用现有的 `readerChromeStyle`、`gesturePullBookmark`、`gestureSwipeUpNotes` 和 `pageTurnStyle`。后者只提供三项分页动效；已设为连续滚动的书仍保持滚动。
- 与原工作区差异：未采用极简强制分页、整套经典五按钮底栏/独立进度浮层、正文外围的并行 pointer 处理、Web Battery API 或无源约束的原生接口。保留最新选区安全区定位、笔记工作区、脚注、目录和卷页实现。

## Review

本地复核了 390 px 页边与标题、顶底栏、拉灯、三点菜单、贴底浮层和桌面复制。首轮完整回归发现退场时的旧布局任务在读区已卸载、文本已清空后仍恢复偏移（偏移 5、文本长度 0）；原基底同一回归通过。在 resize 的异步边界取消已卸载任务，重跑全链路通过；仍保留活跃阅读器的精确 Locator 和正常报错。Android 端继续只接收指定 WebView、应用来源、主框架的列举命令，CSP 未改。独立审查记录在 PR。

## Evidence And Residual Risks

日志与截图在 `artifacts/local/audits/reader-chrome-v2/`。未在 Android 设备或模拟器验证手势、实际电量或原生手柄，未跑 Linux WebKitGTK / Windows WebView2 原生壳。可单独回退本批提交，已存的经典/极简和手势偏好仍为同名 UI 字段。

---
description: 固定 Readest 0.11.20 与 foliate-js 源码，复核 Android 分页手势、内容仲裁、动画热路径和 Atha 真机滑动帧缺口。
---

# Readest Android 翻页手势与动画源码复核

## 后续状态

本文的 Readest 结论固定在所列提交；其中 Atha 对照只代表 2026-08-09 的源码快照。2026-08-10 的实现已经同步末位移、对超长章节采用 20,000px 原生 `scrollLeft` 回退、改为 300ms 收束，并让诊断读取内联 transform / 真实 `scrollLeft` 及全部连续 rAF 间隔。当前 Atha 行为与数字以 `docs/changes/reader-gesture-performance.md` 和 `docs/codebase/MAP.md` 为准；下文 `[A]` / `[G]` 不再作为当前事实。

## 结论

截至 2026-08-09，GitHub Releases 标记的最新正式版仍是 [Readest v0.11.20](https://github.com/readest/readest/releases/tag/v0.11.20)，release commit 为 [`1df1505fc5033fc949463c9908f2d53bd0fbdfa6`](https://github.com/readest/readest/tree/1df1505fc5033fc949463c9908f2d53bd0fbdfa6)，其 `foliate-js` gitlink 为 [`dd71f2be356563c16a23272686189fcfb45d0b82`](https://github.com/readest/foliate-js/tree/dd71f2be356563c16a23272686189fcfb45d0b82)。本文只以这两个固定提交作源码结论，不把 Readest `main`、README 或历史版本混入。

这次深入后，前一轮理解需要修正三个关键点：

1. **Readest Android 的默认翻页不是截图、Canvas 或 WebGL。** 移动端默认合并为 `animated: true`、`pageTurnStyle: push`，实际由 foliate paginator 直接修改原生 `scrollLeft` 跟手，松手后再执行 300ms 收束。截图覆盖层只用于用户选择 `curl`，或旧引擎下的 `slide`，不是默认性能基线。
2. **Atha 的“拖动帧”指标不是 Android 已呈现帧。** 当前诊断只在 `pointerup` 之前逐个 rAF 读取 `getComputedStyle(book).transform`，既排除了 150ms 松手动画，也不能证明 SurfaceFlinger 已经提交该帧。现有 Linux 门还人为发送 10 个、每个 16ms 的 move，天然给 rAF 十次运行机会，不能覆盖真机的稀疏快速 flick。
3. **Atha 确有一个 Readest 没有的末样本丢失窗口。** `previewSwipe()` 将最新位移放进待执行 rAF；松手后 navigation 的 Promise microtask 会在下一次渲染前进入 `showPage()`，而 `showPage()` 先取消该 rAF、清空 `swipeDelta`。因此最后一个或全部尚未绘制的 move 可以被丢掉。它能解释稀疏手势只有极少拖动视觉样本，但仅凭静态源码还不能断言它就是 PCT-AL10 卡顿的唯一根因。

最小优先级因此不是复制 Readest 的卷页，而是：

- P0：在 release 路径保留并应用最新位移，再从该位置收束；同时补一个 1 个 move 和一个无等待 burst 的可信手势用例。
- P0：把诊断拆成输入事件、JS 视觉采样、松手动画采样和 Android 实际呈现帧，停止把 rAF 样本叫“呈现帧”。
- P1：记录 `book.scrollWidth`、DPR 和合成耗时；若卡顿只在长章出现，再引入 Readest 的“大 section 不合成整层”策略，而不是永久添加 `translateZ(0)`。
- P2：补最近速度用于短 flick，但保留 Atha 已有 48px 产品阈值，不机械改成 Readest 的半页阈值。

## 证据边界

下文使用以下标记：

- **[R]** 固定 Readest / foliate-js 源码事实；
- **[A]** 当前 Atha 工作树源码事实；
- **[I]** 由两边调用顺序推出、仍需真机 trace 验证的解释；
- **[G]** 当前自动化或实机证据缺口。

本文是静态源码复核，没有操作 PCT-AL10，也没有把 Linux WebKitGTK、Android 模拟器或 Readest 测试结果冒充真机验收。

## Readest 的三条翻页管线

### 1. 默认 Android `push`

**[R] 默认设置。** `getDefaultViewSettings()` 先合并 `DEFAULT_VIEW_CONFIG`，再合并移动端设置，因此 Android 默认从 `animated: false, pageTurnStyle: push` 变为 `animated: true, pageTurnStyle: push`。[`settingsService.ts` L38-L54](https://github.com/readest/readest/blob/1df1505fc5033fc949463c9908f2d53bd0fbdfa6/apps/readest-app/src/services/settingsService.ts#L38-L54)、[`constants.ts` L286-L315](https://github.com/readest/readest/blob/1df1505fc5033fc949463c9908f2d53bd0fbdfa6/apps/readest-app/src/services/constants.ts#L286-L315)、[`L354-L360`](https://github.com/readest/readest/blob/1df1505fc5033fc949463c9908f2d53bd0fbdfa6/apps/readest-app/src/services/constants.ts#L354-L360)、[`L383-L407`](https://github.com/readest/readest/blob/1df1505fc5033fc949463c9908f2d53bd0fbdfa6/apps/readest-app/src/services/constants.ts#L383-L407)。

**[R] 事件绑定。** 每个 iframe 文档加载后，Readest 同时安装两组监听：应用层把 `touchstart/move/end/cancel` 序列化到父窗口；foliate paginator 直接在宿主和已加载 iframe document 上注册非 passive Touch Events。[`FoliateViewer.tsx` L419-L438](https://github.com/readest/readest/blob/1df1505fc5033fc949463c9908f2d53bd0fbdfa6/apps/readest-app/src/app/reader/components/FoliateViewer.tsx#L419-L438)、[`iframeEventHandlers.ts` L384-L467](https://github.com/readest/readest/blob/1df1505fc5033fc949463c9908f2d53bd0fbdfa6/apps/readest-app/src/app/reader/utils/iframeEventHandlers.ts#L384-L467)、[`paginator.js` L1446-L1455](https://github.com/readest/foliate-js/blob/dd71f2be356563c16a23272686189fcfb45d0b82/paginator.js#L1446-L1455)。默认 `push` 没有 captured interceptor，真正移动页面的是 foliate 的直接监听。

**[R] 拖动。** `touchstart` 一次性保存位置、时间和背景绘制上下文；`touchmove` 拒绝多点、现有选区、scroll lock、`no-swipe` 和滚动模式，随后累计 `dx/dy`、记录最后一段 `vx/vy`。普通横排分页只要累计横向距离不小于纵向距离，就调用 `scrollBy(dx, 0)`，直接改变容器原生 scroll position，而不是先排一个可被 release 取消的 transform rAF。[`paginator.js` L2137-L2176](https://github.com/readest/foliate-js/blob/dd71f2be356563c16a23272686189fcfb45d0b82/paginator.js#L2137-L2176)、[`L2177-L2238`](https://github.com/readest/foliate-js/blob/dd71f2be356563c16a23272686189fcfb45d0b82/paginator.js#L2177-L2238)。

**[R] 松手。** 超过最后一个 move 80ms 才抬手会清零 flick 速度；普通 push 在下一个 rAF 中以最后速度、整段位移和当前 scroll position 调用 `snap()`。`snap()` 区分跟手路径与非跟手路径，并用整个手势的方向避免末端抖动误翻。[`paginator.js` L2035-L2109](https://github.com/readest/foliate-js/blob/dd71f2be356563c16a23272686189fcfb45d0b82/paginator.js#L2035-L2109)、[`L2476-L2538`](https://github.com/readest/foliate-js/blob/dd71f2be356563c16a23272686189fcfb45d0b82/paginator.js#L2476-L2538)。

**[R] 收束动画。** 默认 push 对已加载 view 临时设置 `will-change/transform/transition`，强制一次起始样式落定，以 300ms `cubic-bezier(0.25, 0.46, 0.45, 0.94)` 动画，结束后清理 inline 样式并提交最终原生 scroll position。[`paginator.js` L17-L81](https://github.com/readest/foliate-js/blob/dd71f2be356563c16a23272686189fcfb45d0b82/paginator.js#L17-L81)。

### 2. 新 WebView 的 layered `slide/curl`

**[R] 启用条件。** 只有用户选择非 `push`，而且引擎同时支持 `startViewTransition` 与 `view-transition-group: nearest` 时，renderer 才获得 `turn-style`。Readest 明确把该能力界定为 Chrome / WebView 140+，并用它避开 iOS 18 WebKit 的 layered snapshot 崩溃。[`viewTransition.ts` L1-L22](https://github.com/readest/readest/blob/1df1505fc5033fc949463c9908f2d53bd0fbdfa6/apps/readest-app/src/utils/viewTransition.ts#L1-L22)、[`useCapturedTurn.ts` L34-L72](https://github.com/readest/readest/blob/1df1505fc5033fc949463c9908f2d53bd0fbdfa6/apps/readest-app/src/app/reader/hooks/useCapturedTurn.ts#L34-L72)。

**[R] 认领与跟手。** 累计横移至少 24px、且达到纵移 1.5 倍才建立 View Transition；回调中先把 live page 跳到目标页，再暂停 `::view-transition-*` 动画，以手指位移映射 `currentTime`。[`paginator.js` L2284-L2367](https://github.com/readest/foliate-js/blob/dd71f2be356563c16a23272686189fcfb45d0b82/paginator.js#L2284-L2367)。

**[R] 提交。** 整段手势必须横向占优；沿翻页方向的速度超过 `0.3px/ms` 时提交，否则进度超过 50% 才提交，反向 flick 会取消。取消路径恢复 live page 后给两次 rAF 再移除 snapshot。[`paginator.js` L2374-L2435](https://github.com/readest/foliate-js/blob/dd71f2be356563c16a23272686189fcfb45d0b82/paginator.js#L2374-L2435)、[`L2501-L2527`](https://github.com/readest/foliate-js/blob/dd71f2be356563c16a23272686189fcfb45d0b82/paginator.js#L2501-L2527)。slide 是 300ms 同一缓动；curl 是 450ms `cubic-bezier(0.3, 0.1, 0.4, 1)` 的 snapshot mask 动画。[`paginator.js` L151-L253](https://github.com/readest/foliate-js/blob/dd71f2be356563c16a23272686189fcfb45d0b82/paginator.js#L151-L253)。

### 3. Tauri captured `slide/curl`

**[R] 启用条件。** Tauri 中用户选择 curl 时始终尝试 captured turn；选择 slide 但引擎没有完整 View Transition group 时也走 captured turn。此时 paginator 被设为 `no-swipe`，应用层 touch interceptor 取得所有权。[`useCapturedTurn.ts` L34-L72](https://github.com/readest/readest/blob/1df1505fc5033fc949463c9908f2d53bd0fbdfa6/apps/readest-app/src/app/reader/hooks/useCapturedTurn.ts#L34-L72)。

**[R] 事件与认领。** iframe 会把 `changedTouches` 和真实 release 坐标一同发给父窗口；interceptor 按优先级调用，第一个返回 true 的 owner 消费手势。captured turn 以 15px 横向阈值认领，选区和 scroll lock 会锁住整个序列。[`iframeEventHandlers.ts` L384-L467](https://github.com/readest/readest/blob/1df1505fc5033fc949463c9908f2d53bd0fbdfa6/apps/readest-app/src/app/reader/utils/iframeEventHandlers.ts#L384-L467)、[`useIframeEvents.ts` L321-L381](https://github.com/readest/readest/blob/1df1505fc5033fc949463c9908f2d53bd0fbdfa6/apps/readest-app/src/app/reader/hooks/useIframeEvents.ts#L321-L381)、[`useTouchInterceptor.ts` L3-L64](https://github.com/readest/readest/blob/1df1505fc5033fc949463c9908f2d53bd0fbdfa6/apps/readest-app/src/app/reader/hooks/useTouchInterceptor.ts#L3-L64)、[`useCapturedTurn.ts` L289-L395](https://github.com/readest/readest/blob/1df1505fc5033fc949463c9908f2d53bd0fbdfa6/apps/readest-app/src/app/reader/hooks/useCapturedTurn.ts#L289-L395)。

**[R] 不丢末样本。** capture 尚未完成时，`moveDrag()` 仍更新独立 session；`endDrag()` 与 `beginDrag()` 串行，并在 release 前再次应用 changedTouches 的进度。因此快速 release 不会从 progress 0 开始收束。[`capturedTurn.ts` L154-L265](https://github.com/readest/readest/blob/1df1505fc5033fc949463c9908f2d53bd0fbdfa6/apps/readest-app/src/app/reader/utils/capturedTurn.ts#L154-L265)。这正是 Atha 当前 rAF release 顺序缺少的保护，但应借用“末样本不能丢”的原则，不应复制整套截图管线。

**[R] 截图成本。** Android 先以 PixelCopy 截取 WebView window surface，把目标压到最多 2 倍 CSS 像素，再在后台线程编码 JPEG 90。源码记录 Xiaomi 13 的 3 倍 PNG 约 1.5 秒，2 倍 JPEG 编码仍约 100ms。[`NativeBridgePlugin.kt` L1537-L1605](https://github.com/readest/readest/blob/1df1505fc5033fc949463c9908f2d53bd0fbdfa6/apps/readest-app/src-tauri/plugins/tauri-plugin-native-bridge/android/src/main/java/NativeBridgePlugin.kt#L1537-L1605)。之后还要 `createImageBitmap`、挂载 Canvas/WebGL overlay、覆盖 live page、无动画跳页，才开始跟手或收束。[`capturedTurn.ts` L292-L409](https://github.com/readest/readest/blob/1df1505fc5033fc949463c9908f2d53bd0fbdfa6/apps/readest-app/src/app/reader/utils/capturedTurn.ts#L292-L409)。

**[R] 渲染。** captured slide 是整屏 DPR Canvas 的 `translateX`；curl 是 64×64 WebGL 网格，每帧更新 uniforms 后 `drawElements`，默认以 450ms `easeInOutQuad` 播放剩余进度。[`pageSlide.ts` L14-L61](https://github.com/readest/readest/blob/1df1505fc5033fc949463c9908f2d53bd0fbdfa6/apps/readest-app/src/utils/pageSlide.ts#L14-L61)、[`pageCurl.ts` L76-L118](https://github.com/readest/readest/blob/1df1505fc5033fc949463c9908f2d53bd0fbdfa6/apps/readest-app/src/utils/pageCurl.ts#L76-L118)、[`L248-L297`](https://github.com/readest/readest/blob/1df1505fc5033fc949463c9908f2d53bd0fbdfa6/apps/readest-app/src/utils/pageCurl.ts#L248-L297)、[`capturedTurn.ts` L411-L433](https://github.com/readest/readest/blob/1df1505fc5033fc949463c9908f2d53bd0fbdfa6/apps/readest-app/src/app/reader/utils/capturedTurn.ts#L411-L433)。

**[I] 对 Atha 的意义。** 该管线牺牲约百毫秒 capture 启动成本换取 viewport 大小的独立 moving layer，适合卷页效果，不适合修复 Atha 的首反馈与快速 push。直接复制会增加原生插件、bitmap 内存、Canvas/WebGL 生命周期和失败回退，超出当前问题所需。

## 内容为什么不会吞掉翻页

### 图片、公式、表格与链接

**[R] 图片和普通链接不在 swipe 黑名单。** paginator 在 iframe document 根接收 Touch Events，不按 target 排除 `img` 或 `a`。应用层只在 clean single click 阶段区分图片 / SVG image / table、链接、音视频和脚注；滑动造成的兼容 click 通过起终点距离抑制，甚至覆盖 `touchstart -> touchend` 之间没有 `touchmove` 的快速 flick。[`iframeEventHandlers.ts` L226-L245](https://github.com/readest/readest/blob/1df1505fc5033fc949463c9908f2d53bd0fbdfa6/apps/readest-app/src/app/reader/utils/iframeEventHandlers.ts#L226-L245)、[`L247-L382`](https://github.com/readest/readest/blob/1df1505fc5033fc949463c9908f2d53bd0fbdfa6/apps/readest-app/src/app/reader/utils/iframeEventHandlers.ts#L247-L382)、[`L405-L438`](https://github.com/readest/readest/blob/1df1505fc5033fc949463c9908f2d53bd0fbdfa6/apps/readest-app/src/app/reader/utils/iframeEventHandlers.ts#L405-L438)。注意：这里保证的是不误开链接 / 预览；默认 push 若完全没有 move，foliate 本身也不会凭 release 位移翻页。

**[R] display math 与 table 只在真实 overflow 时另有 owner。** Readest 给 table 和 display MathML 注入 `.scroll-wrapper`，fit 容器改为 `overflow: visible`，不参与拦截；确实溢出的 wrapper 使用 `overflow: auto` 和原生 touch scrolling。inline math 不包装，`pre/code` 也不进入这套 capture-phase owner。[`scrollable.ts` L1-L41](https://github.com/readest/readest/blob/1df1505fc5033fc949463c9908f2d53bd0fbdfa6/apps/readest-app/src/utils/scrollable.ts#L1-L41)、[`L133-L179`](https://github.com/readest/readest/blob/1df1505fc5033fc949463c9908f2d53bd0fbdfa6/apps/readest-app/src/utils/scrollable.ts#L133-L179)、[`style.ts` L402-L427](https://github.com/readest/readest/blob/1df1505fc5033fc949463c9908f2d53bd0fbdfa6/apps/readest-app/src/utils/style.ts#L402-L427)。

**[R] Readest 不做边界移交。** wrapper 一旦在某轴存在 overflow，该轴整次手势都由 wrapper 消费；判定不读取当前 `scrollLeft/scrollTop`，所以到左右 / 上下边界也不会把同一次或下一次手势交给翻页。源码注释和测试都把“边界仍消费”写成目标行为。[`scrollable.ts` L43-L70](https://github.com/readest/readest/blob/1df1505fc5033fc949463c9908f2d53bd0fbdfa6/apps/readest-app/src/utils/scrollable.ts#L43-L70)、[`L72-L131`](https://github.com/readest/readest/blob/1df1505fc5033fc949463c9908f2d53bd0fbdfa6/apps/readest-app/src/utils/scrollable.ts#L72-L131)、[`scrollable.test.ts` L161-L267](https://github.com/readest/readest/blob/1df1505fc5033fc949463c9908f2d53bd0fbdfa6/apps/readest-app/src/__tests__/utils/scrollable.test.ts#L161-L267)。

**[A] Atha 的方向边界 owner 更符合当前验收。** Atha 在 pointerdown 缓存 overflow 起点和上限，首次明确横向意图时只在起手方向还有空间才选 `overflow`，否则选 `page`；owner 在序列内不再变化。[`interaction.mjs` L128-L156](../../reader/web/interaction.mjs#L128-L156)、[`L236-L309`](../../reader/web/interaction.mjs#L236-L309)。不应为了“像 Readest”退回边界永久吞手势。

### 选区和控件

**[R] Readest 在 move 时检查非折叠 selection，captured turn 还把“起手已有选区 / 曾被 scroll lock 阻止”锁存到整个序列，避免选区短暂消失后突然翻页。[`paginator.js` L2177-L2200](https://github.com/readest/foliate-js/blob/dd71f2be356563c16a23272686189fcfb45d0b82/paginator.js#L2177-L2200)、[`useCapturedTurn.ts` L83-L113](https://github.com/readest/readest/blob/1df1505fc5033fc949463c9908f2d53bd0fbdfa6/apps/readest-app/src/app/reader/hooks/useCapturedTurn.ts#L83-L113)、[`L297-L339`](https://github.com/readest/readest/blob/1df1505fc5033fc949463c9908f2d53bd0fbdfa6/apps/readest-app/src/app/reader/hooks/useCapturedTurn.ts#L297-L339)。

**[A] Atha 在 down 和 up 都检查 selection，表单、dialog、contenteditable 和链接则从 pointerdown 起完全拒绝该序列。[`interaction.mjs` L73-L96](../../reader/web/interaction.mjs#L73-L96)、[`L236-L285`](../../reader/web/interaction.mjs#L236-L285)、[`L311-L358`](../../reader/web/interaction.mjs#L311-L358)。

**[I] 后续产品改进。** Readest 证明“链接保持原生点击”不要求“从链接起手的横向 drag 完全失效”。Atha 可在独立语义测试后把 `a` 从 hard-protected drag 中移出，只在未形成 page owner 时保留 click；这与当前帧问题无直接关系，不应混入 P0 修复。

## 动画和重内容热路径

### Readest 的 Android 保护

**[R] 大 section 逃生路径。** 默认 push 的 CSS transform 会把所有已加载 view 合成为大层。foliate 记录 Android 高 DPR 下超过 GPU texture limit 会让 Blink 在动画前阻塞约 1 秒，因此当累计 rendered view size 超过 20,000 CSS px 时，不再 transform 整层，改用 300ms `requestAnimationFrame` 更新原生 scroll offset；竖排无法这样替代时直接跳页。[`paginator.js` L255-L285](https://github.com/readest/foliate-js/blob/dd71f2be356563c16a23272686189fcfb45d0b82/paginator.js#L255-L285)、[`L2617-L2747`](https://github.com/readest/foliate-js/blob/dd71f2be356563c16a23272686189fcfb45d0b82/paginator.js#L2617-L2747)。

**[R] Android 不使用持久 promotion hint。** `gpu-composite` 只在 iOS 设置；Android paginated renderer 不保留 `translateZ(0)`。源码明确说明该提示曾导致 Android 高 DPR Blink freeze。[`FoliateViewer.tsx` L749-L770](https://github.com/readest/readest/blob/1df1505fc5033fc949463c9908f2d53bd0fbdfa6/apps/readest-app/src/app/reader/components/FoliateViewer.tsx#L749-L770)、[`paginator.js` L1291-L1317](https://github.com/readest/foliate-js/blob/dd71f2be356563c16a23272686189fcfb45d0b82/paginator.js#L1291-L1317)。touchstart 虽给 primary view 临时写 `willChange`，touchend 的直接清理代码已被注释，最终通常由 settle / animation cleanup 回收；这不是值得 Atha 照抄的持久层策略。[`paginator.js` L2137-L2151](https://github.com/readest/foliate-js/blob/dd71f2be356563c16a23272686189fcfb45d0b82/paginator.js#L2137-L2151)、[`L2450-L2484`](https://github.com/readest/foliate-js/blob/dd71f2be356563c16a23272686189fcfb45d0b82/paginator.js#L2450-L2484)。

**[R] 拖动时不做加载和重复 layout read。** 相邻 section 的 columnize / expand 会在主线程丢帧，所以 finger drag 期间暂停 preload；背景所需 computed style 和几何在手势开始一次性快照，逐帧路径只写 DOM。[`paginator.js` L1367-L1415](https://github.com/readest/foliate-js/blob/dd71f2be356563c16a23272686189fcfb45d0b82/paginator.js#L1367-L1415)、[`L1660-L1758`](https://github.com/readest/foliate-js/blob/dd71f2be356563c16a23272686189fcfb45d0b82/paginator.js#L1660-L1758)。Readest 还把 relocate 状态写入合并到 rAF；源码记录旧 `requestIdleCallback` 在 Android 压力下积成 2 秒以上的松手后任务。[`FoliateViewer.tsx` L181-L249](https://github.com/readest/readest/blob/1df1505fc5033fc949463c9908f2d53bd0fbdfa6/apps/readest-app/src/app/reader/components/FoliateViewer.tsx#L181-L249)。

### Atha 当前路径

**[A] 已做对的部分。** `pointerdown` 一次缓存 reader rect、layout scale 和 overflow 几何；`pointermove` 的 page owner 只写内存并由单个 rAF 更新 transform；公式与后续 `onPageShown()` 在新页 transform 写入和一个 `nextFrame()` 之后执行。[`interaction.mjs` L236-L309](../../reader/web/interaction.mjs#L236-L309)、[`pagination.mjs` L515-L528](../../reader/web/pagination.mjs#L515-L528)、[`L600-L619`](../../reader/web/pagination.mjs#L600-L619)。这已经符合 Readest 的“热路径不做布局读”原则。

**[A] 末样本窗口。** `previewSwipe()` 只保留一个 pending rAF。`showPage()` 会取消它、清空 delta，再移除 dragging 并写目标页 transform。[`pagination.mjs` L265-L287](../../reader/web/pagination.mjs#L265-L287)、[`L600-L619`](../../reader/web/pagination.mjs#L600-L619)。navigation 的 `run()` 通过 resolved Promise 的 `.then(action)` 排队，HTML 事件任务结束后的 microtask 会先于下一次 rendering opportunity 执行。[`navigation.mjs` L61-L71](../../reader/web/navigation.mjs#L61-L71)、[`L196-L219`](../../reader/web/navigation.mjs#L196-L219)、[`L371-L380`](../../reader/web/navigation.mjs#L371-L380)。

**[I] 结果。** 若最后一个 move 与 pointerup 落在同一 vsync 之前，navigation microtask 会先进入 `showPage()`，待执行预览就消失。前面已有 rAF 的慢拖仍顺滑，所以 Linux 的 10×16ms 用例稳定通过，真机短 flick 却可能只留下 0 到 2 个拖动样本。

**[A] 整章 layer 风险。** Atha 的 `.book` 是整个 section 的多栏内容，所有页都通过同一个 `transform: translateX(...)` 定位；拖动时对整个 `.book` 设置 `will-change`，没有 Readest 的 20,000px 保护。[`atha-reader.css` L960-L996](../../reader/atha-reader.css#L960-L996)。长章、高 DPR 和重图片会让该 layer 远大于 viewport。

**[I] 风险排序。** 末样本丢失能直接解释“拖动视觉采样少”；整章 layer 更能解释“已经有 150ms transition 但 Android 实际仍只呈现极少帧”。两者可同时存在，必须用同一次真机 trace 中的 `book.scrollWidth`、输入时间、transform write、FrameTimeline 和 SurfaceFlinger frame 对齐后再判主因。

## 为什么现有 benchmark 没挡住

### Atha

**[A] 输入被刻意平滑。** Linux gate 为每次 drag 固定发送 10 个 `pointerMove`，每个 duration 16ms；随后要求 `pointerMoves >= 10`、page drag 至少 6 个不同 transform，以及视觉更新不少于 `max(3, ceil(pointerMoves/2))`。[`check-fb2-source.ps1` L126-L172](../../scripts/check-fb2-source.ps1#L126-L172)、[`L247-L314`](../../scripts/check-fb2-source.ps1#L247-L314)。这验证持续拖动，不验证稀疏 Android flick。

**[A] 指标只覆盖抬手前。** diagnostics 的 `visualFrames` 从 `pointerdown` 后的 rAF 样本筛选，但又以 `frame.at <= pointerUpAt` 截断；150ms settling 和目标页呈现均不计入。采样内容是 computed transform / scrollLeft，不是硬件 frame。[`diagnostics.mjs` L1206-L1274](../../reader/web/diagnostics.mjs#L1206-L1274)、[`L1285-L1375`](../../reader/web/diagnostics.mjs#L1285-L1375)。因此 `visualUpdateSamples=2` 最多证明抬手前观察到两个不同 CSS 状态，不能单独证明十次操作总共只显示了两帧。

**[A] 平台边界。** 已记录的正式门虽为可信 W3C input，但 WebKitGTK 实际报告 `pointerType=mouse`；它不覆盖 Android Touch Events / Pointer Events cadence、Blink compositor 或 ARM GPU。

### Readest

**[R] 浏览器测试覆盖语义与热路径，不覆盖 Android FPS。** `paginator-turn-styles.browser.test.ts` 覆盖 layered 跟手、提交、取消与方向；`captured-turn.browser.test.ts` 覆盖 capture race、快速 release 和末样本 buffer；`paginator-background-anim-perf.browser.test.ts` 驱动真实动画，但只断言一次 turn 的 `getComputedStyle` 不超过 3 次、逐拖动帧不超过 1 次，不测呈现帧率。[`paginator-background-anim-perf.browser.test.ts` L1-L21](https://github.com/readest/readest/blob/1df1505fc5033fc949463c9908f2d53bd0fbdfa6/apps/readest-app/src/__tests__/document/paginator-background-anim-perf.browser.test.ts#L1-L21)、[`L114-L225`](https://github.com/readest/readest/blob/1df1505fc5033fc949463c9908f2d53bd0fbdfa6/apps/readest-app/src/__tests__/document/paginator-background-anim-perf.browser.test.ts#L114-L225)。

**[R] Android lane 也没有 swipe perf gate。** Vitest Android lane 通过 WebView CDP 和 adb input 驱动已安装 app，串行、120 秒、失败重试一次；当前 Android 测试集中在 selection 和 double click。GitHub workflow 是 nightly / 手动 / 加标签运行且不阻断普通 PR。[`vitest.android.config.mts` L1-L21](https://github.com/readest/readest/blob/1df1505fc5033fc949463c9908f2d53bd0fbdfa6/apps/readest-app/vitest.android.config.mts#L1-L21)、[`test-android.sh` L1-L29](https://github.com/readest/readest/blob/1df1505fc5033fc949463c9908f2d53bd0fbdfa6/apps/readest-app/scripts/test-android.sh#L1-L29)、[`android-e2e.yml` L1-L26](https://github.com/readest/readest/blob/1df1505fc5033fc949463c9908f2d53bd0fbdfa6/.github/workflows/android-e2e.yml#L1-L26)。`pnpm bench` 是手动、拒绝 CI 的 Node benchmark，现有项目只有 vector retrieval，没有 reader gesture benchmark。[`package.json` L26-L40](https://github.com/readest/readest/blob/1df1505fc5033fc949463c9908f2d53bd0fbdfa6/apps/readest-app/package.json#L26-L40)、[`bench/README.md` L1-L54](https://github.com/readest/readest/blob/1df1505fc5033fc949463c9908f2d53bd0fbdfa6/apps/readest-app/bench/README.md#L1-L54)。

所以 Readest 源码能提供设计和已知回归线索，不能拿它的测试套件声称“Readest Android 已通过固定 FPS 门”。

## 逐项对照

| 维度 | Readest v0.11.20 | Atha 当前实现 | 判断 |
| --- | --- | --- | --- |
| 默认 Android 翻页 | animated push，原生 scroll 跟手 | 整章 transform 跟手 | 不能用 Readest captured curl 代表其默认性能 |
| 输入 API | iframe Touch Events + 父窗口消息 | 同文档 Pointer Events | Atha 结构更短，但要覆盖真实 Pointer cadence |
| owner 阈值 | push 以累计横向占优；layered 24px/1.5；captured 15px/1 | 8px/1.5 后锁存 owner | Atha 认领更早，不是明显慢因 |
| release | 最近速度、整段方向、位移 / 当前 scroll；captured 用 changedTouches | 固定 48px 和总方向 | 可加最近速度，不应删除 Atha 的 48px 门 |
| pending sample | 原生 scroll 已写；captured 显式 buffer 并在 end 前应用 | pending rAF 可被 `showPage()` 取消 | P0 源码缺口 |
| 普通 settle | 300ms | 150ms | Atha 已更短，不要为模仿而加长 |
| 大 section | >20,000px 改原生 scroll rAF / 瞬移 | 始终 transform 整章 | P1 真机 trace 重点 |
| Android promotion | 不设置持久 `gpu-composite` | drag 时 transient `will-change` | 不要直接加永久 `translateZ(0)` |
| 链接 / 图片 | swipe 不按 target 黑名单；click 单独仲裁 | 图片可 swipe，链接 hard protect | 链接可后续放宽，不属性能 P0 |
| 溢出表格 | 存在 overflow 就永久拥有该轴，边界不移交 | 起手方向到边界则交给 page | 保留 Atha 策略 |
| 自动化 | 丰富浏览器语义测试，Android 无 swipe FPS | 可信 Linux 10×16ms 时序门 | 两边都缺真实 Android 稀疏 flick + 呈现帧 gate |

## 最小修复顺序

### P0：保留 release 最新位移

在 `pagination` 内增加单一 release 收口，而不是让 `interaction` 知道 transform 细节：

1. pointerup 用真实 release 坐标更新最终 `deltaX`；
2. 若 `swipeFrame` 仍 pending，取消它但立即把最新 transform 写入；
3. 在 release 冷路径让起始样式生效，再设置 `data-swipe-settling` 和目标 transform；可选方案是一次 layout flush，或等待一次 rAF，二者只选一个并用真机首反馈比较；
4. commit 和 cancel 共用同一收口，避免无效短拖回弹也从过期样本开始；
5. 添加“release 与最后 move 同一 rendering opportunity”的回归测试。

这借的是 Readest `moveDrag(latest) -> endDrag()` 和 CSS animation 起始样式落定的原则，不引入 screenshot、Canvas、WebGL 或新依赖。

### P0：修正诊断语义

至少分开输出：

- `pointerMoveSamples`：浏览器实际收到多少 move；
- `dragStyleSamples`：pointerup 前观察到多少不同 CSS 值；
- `settleStyleSamples`：pointerup 后到 stable 的不同 CSS 值；
- `firstInputToStyleWriteMs`：输入到 JS 写样式；
- `releaseToStableMs`：保留现有稳定耗时；
- `androidPresentedFrames` / jank：只由项目级 Android FrameTimeline、gfx framestats 或 Perfetto 入口产生，拿不到就写 `null`，不能由 rAF 推断。

可信输入增加两类 cadence：现有 10×16ms 持续拖动继续保留；另加 1 个 move 的短 flick，以及多个 0ms move 后立即 pointerup 的 burst。后两类只要求单步语义、末样本收束和松手帧，不要求抬手前必须出现 6 个 rAF transform。

### P1：只在证据命中后处理大 layer

先把 `book.scrollWidth`、page count、DPR、图片像素量和触摸期间 Long Task 写入匿名诊断。若 PCT-AL10 的卡顿与长章尺寸高度相关：

1. 短期可在超大章关闭 drag / settle layer promotion，并比较直接跳页是否消除约 1 秒冻结；
2. 正式方向优先把页面位置改为 viewport 内原生 horizontal scroll，像 Readest oversized fallback 一样逐帧改 scroll offset；
3. 若仍需复杂卷页，再研究 viewport snapshot；不要 transform 整章，也不要先上持久 `translateZ(0)`。

20,000px 是 Readest 针对其多 view 架构的经验阈值，不是可直接复制到 Atha 的常数。Atha 应以 PCT-AL10 A/B 找到自己的边界。

### P2：补 flick，不替换产品距离

记录最近一个 move 的 x / time；若该样本距 release 不超过 80ms，沿目标方向速度超过 `0.3px/ms` 可提交，否则仍按 Atha 的 48px 与整段方向判断。这样只增加短快 flick，不把慢拖门槛提高到半页。反向末端 flick 和整段纵向占优必须取消。

## 不应复制

- 不复制 Readest captured PixelCopy + JPEG + Canvas / WebGL 作为默认 push；其启动成本和复杂度与当前问题相反。
- 不复制 Readest overflow wrapper 的“到边界仍永久吞该轴”；Atha 已有的下一次手势边界移交更符合验收。
- 不把 Readest 的 300ms / 450ms 动画当作“更快”；Atha 当前 150ms 不是帧数少的直接证据。
- 不在 Android 给整章永久加 `translateZ(0)` 或 `will-change`；Readest 正是因 Android 高 DPR 大层冻结而取消这条路。
- 不把 Readest 浏览器测试、Atha Linux rAF 样本或截图当 Android 硬件呈现验收。

## 待真机回答的四个问题

1. 稀疏 flick 中，Atha 实际收到多少 `pointermove`，最后 move 到 pointerup 间隔是多少？
2. pending preview 被 flush 后，150ms settling 是否产生正常 FrameTimeline，还是仍只提交头尾帧？
3. 卡顿是否随 `book.scrollWidth × DPR` 增长，并在短章节消失？
4. 同一 PCT-AL10 上 Readest 默认 `push` 与 Atha 的实际 presented / jank 数据各是多少？必须确认 Readest 设置仍为 `push`，不能拿 curl 与 Atha push 比。

回答完这四项，才能决定 P1 是原生 scroll、viewport 分片，还是仅修 release 与 benchmark。源码本身已经足够支持 P0，但不足以声称完整真机性能问题已关闭。

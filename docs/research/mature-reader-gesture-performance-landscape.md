---
description: 对照 Readium、Foliate、KOReader、CREngine 与 epub.js 的手势、分页和重内容性能机制，给出 Atha 的差异矩阵与取舍。
---

# 成熟阅读器手势、分页与重内容性能研究

## 后续状态

上游项目比较仍固定在表中提交；Atha 对照固定在 `f07ecb2`，不是当前工作树。此后 Atha 已改为 300ms 收束、超过 20,000px 的章节使用原生 `scrollLeft`，并完成新的 rAF 与 PCT SurfaceFlinger 基准。下文 150ms、整章始终 transform、212 / 216ms 等 Atha 数字只用于解释该历史候选；当前事实以 `docs/changes/reader-gesture-performance.md` 和 `docs/codebase/MAP.md` 为准。

## 结论先行

本轮固定 Atha 候选并不是“少学了一个成熟库”那么简单。对固定源码逐段比较后，结论是：

1. **Atha 的 DOM 命中仲裁已经比 Foliate、epub.js 和旧 Readium JS 更细。** Atha 以一次手势一个 owner 区分页、纵向意图和横向溢出容器，并在表格边界把新手势交给翻页；其他三个 Web 阅读器的核心分页器大多从整个文档接管 touch，或只在松手时看总位移，未提供等价的 `table/pre` 边界所有权。
2. **值得继续吸收的是“阈值、资源边界和重排生命周期”，不是重写分页器。** Readium Kotlin 使用系统 touch slop、速度与位移共同决定落页，先让当前 WebView 消费滚动，再让外层 Pager 消费资源边界；Foliate 在 `ResizeObserver`、字体完成和隐藏文档时收束布局；KOReader 把昂贵定位延迟到松手，并用渲染哈希与有界缓存控制失效。
3. **Atha 的滑动热路径目前更克制。** 起手缓存几何，move 只更新内存并由一个 rAF 写 transform；`will-change` 仅在拖动期存在，150ms CSS transition 负责收束。Foliate 与 epub.js 都在 JS 动画中逐帧写 scroll offset，可能触发 scroll 管线；KOReader/CREngine 的位图页缓存则建立在自有排版引擎上，不能在不牺牲 WebView 浏览器保真、选择和安全模型的前提下移植。
4. **最有价值的下一轮不是立刻预载更多内容，而是补齐测量。** 当前 Atha 已有直接针对可信输入的 13 场景、每场景 5 次预热 + 20 次测量门；所研究项目的当前源码都没有同等粒度的 Web 手势帧基准。应先补真实触摸、速度矩阵、长会话内存和重排定位漂移，再决定是否增加速度提交或至多一个相邻 section 的预热。

因此，本报告不建议替换 Atha 当前 `interaction.mjs` / `pagination.mjs`，也不建议引入通用手势库、多个 WebView、截图覆盖层或整页位图缓存。建议把后续改动拆成可单独证伪的候选，并以现有性能门为基线做 A/B。

## 范围、方法与固定证据

本轮只读研究产品代码，没有操作 PCT-AL10，没有使用用户级 `android-cli` skill，也没有运行这些上游项目。源码浅克隆位于项目忽略的 `.tmp/`，结论均锚定到完整提交；官方文档和发布说明用于判断公开成熟度，机制判断以固定源码为准。

| 对象 | 固定锚点 | 成熟度边界 |
| --- | --- | --- |
| Atha | `f07ecb24009c0eb4734d3dbf349217223787824e`，2026-08-09 | 当前已实现候选；Linux Tauri / WebKitGTK 可信输入门已过，PCT-AL10 真实触摸仍待用户验收 |
| Readium Kotlin Toolkit | [`f8e6f93db81570c7cc0833279b2628f4c65d8efe`](https://github.com/readium/kotlin-toolkit/tree/f8e6f93db81570c7cc0833279b2628f4c65d8efe) | 项目已发布 3.3.0；本轮重点看的 Compose/Web 新 rendition 标注 `ExperimentalReadiumApi`，属于成熟项目中的新接口，不等同稳定 API |
| Readium Navigator JS | [`01a5d14e44b2daab78ea16270b35a2fe9c36490a`](https://github.com/readium/r2-navigator-js/tree/01a5d14e44b2daab78ea16270b35a2fe9c36490a)，`v1.25.7` | 正式 tag；适合观察长期桌面 WebView 兼容代码，但其 touch 分页较旧，不代表 Readium Kotlin 新方案 |
| foliate-js | [`78914aef4466eb960965702401634c2cb348e9b1`](https://github.com/johnfactotum/foliate-js/tree/78914aef4466eb960965702401634c2cb348e9b1) | 无独立 release tag；是 Foliate 的实际渲染子项目。Foliate 3.2.1 发布说明公开确认 1:1 触摸滑动、分页动画、按 section 降低内存等方向 |
| KOReader | [`e9c0a6e3999726eec20413e2b367021d7130809e`](https://github.com/koreader/koreader/tree/e9c0a6e3999726eec20413e2b367021d7130809e) | 长期发布、多设备应用；官方仓库已有 270 个 tag 和 `v2026.07.2`，本轮 snapshot 提交日期为 2026-08-09 |
| CREngine | [`98d6d6f7ee1d4e6a175e4c6a3d8e81f7a0adb4f8`](https://github.com/koreader/crengine/tree/98d6d6f7ee1d4e6a175e4c6a3d8e81f7a0adb4f8) | KOReader/CoolReader 系自有排版核心；架构与 WebView 根本不同，两槽页缓存代码在该固定源码中默认禁用，只作为设计边界反例 |
| epub.js | [`eee359d0790002115a1156a9833c54f4bcd44c1d`](https://github.com/futurepress/epub.js/tree/eee359d0790002115a1156a9833c54f4bcd44c1d) | 老牌浏览器库；该 snapshot 的 `package.json` 仍为 0.3.93，仓库另有 0.5 alpha tag，因此不能把当前 `master` 的每一行都当稳定发布行为 |

公开成熟度交叉证据：Readium 3.2.0 的[官方发布说明](https://blog.readium.org/release-note-kotlin-toolkit-version-3-2-0/)明确新增 EPUB 动画翻页；Readium 的[输入适配 API](https://readium.org/kotlin-toolkit/latest/api/readium/readium-navigator/org.readium.r2.navigator.util/-directional-navigation-adapter/index.html)公开边缘点击、阅读方向和动画开关；Foliate 的[发布页](https://github.com/johnfactotum/foliate/releases)记录 section 化降低启动内存、触摸跟手分页及更快 resize；KOReader 的[正式发布页](https://github.com/koreader/koreader/releases)和[用户指南](https://koreader.rocks/user_guide/)证明其多设备长期使用边界；epub.js 的[官方仓库](https://github.com/futurepress/epub.js)只证明它是跨设备浏览器渲染库，不替代当前源码审计。

## 1. 事件仲裁不是同一个问题

### Atha：DOM 内容级 owner

Atha 起手先保护链接、表单、弹窗、可编辑内容和已有选区；图片/公式、表格、代码不再整体禁止翻页。超过 8px 漂移后，以 1.5 倍轴优势一次性认领 `vertical`、`overflow` 或 `page`。宽表只有在起手方向仍有可滚空间时拿到 owner；对应边界的新手势由页接管。认领后不在同一序列反复换 owner，避免边界抖动。[`interaction.mjs` L73-L92](../../reader/web/interaction.mjs#L73-L92)、[`L236-L309`](../../reader/web/interaction.mjs#L236-L309)。

这不是普通“ignore selector”。它同时回答四个问题：事件是否可导航、横纵轴意图、内部滚动是否还有余量、该序列由谁完成。页 owner 完成后还抑制兼容 `click` / `dblclick`，避免拖图片后误开查看器。[`interaction.mjs` L311-L372](../../reader/web/interaction.mjs#L311-L372)。

### Readium Kotlin：平台层 child-first，但不替 Atha 做 DOM 仲裁

Readium 新 rendition 把每个 publication resource 放在一个 WebView 中，再由 Compose Pager 连接资源。自定义 `RenditionScrollState` 依次让当前 WebView、外层 Pager、相邻 WebView 消费位移；即“资源内部先走，到了边界再跨资源”。[`RenditionScrollState.kt` L51-L135](https://github.com/readium/kotlin-toolkit/blob/f8e6f93db81570c7cc0833279b2628f4c65d8efe/readium/navigators/web/internals/src/main/kotlin/org/readium/navigator/web/internals/pager/RenditionScrollState.kt#L51-L135)。

Pager 自己关闭 Compose 内建手势和 fling，统一走 2D scroll state，并在 nested-scroll prescroll 前消费子层传入的链路，防止两套滚动器竞争。[`RenditionPager.kt` L38-L82](https://github.com/readium/kotlin-toolkit/blob/f8e6f93db81570c7cc0833279b2628f4c65d8efe/readium/navigators/web/internals/src/main/kotlin/org/readium/navigator/web/internals/pager/RenditionPager.kt#L38-L82)。这是成熟的“WebView resource 与宿主 Pager”仲裁，但并不识别 DOM 内的宽表或公式。

WebView 内点击只把链接、音视频、按钮、表单、canvas、details 和 contenteditable 视为交互目标；`img`、`table`、`math` 不在通用保护表中。[`gestures.ts` L25-L76](https://github.com/readium/kotlin-toolkit/blob/f8e6f93db81570c7cc0833279b2628f4c65d8efe/readium/navigators/web/internals/scripts/src/common/gestures.ts#L25-L76)、[`L78-L115`](https://github.com/readium/kotlin-toolkit/blob/f8e6f93db81570c7cc0833279b2628f4c65d8efe/readium/navigators/web/internals/scripts/src/common/gestures.ts#L78-L115)。因此 Readium 的 native child-first 原则可借鉴，但不能直接替换 Atha 的内容级 hit testing。

### Foliate 与 epub.js：文档级接管，嵌套目标是弱项

Foliate 在外层 custom element 和 iframe document 上都注册非 passive `touchmove`；分页模式单指移动就 `preventDefault()` 并写容器 scroll，只有多指和 pinch 明确退出。源码没有 target path、选区、表格 scrollLeft 边界或 axis owner。[`paginator.js` L558-L575](https://github.com/johnfactotum/foliate-js/blob/78914aef4466eb960965702401634c2cb348e9b1/paginator.js#L558-L575)、[`L823-L864`](https://github.com/johnfactotum/foliate-js/blob/78914aef4466eb960965702401634c2cb348e9b1/paginator.js#L823-L864)。Foliate 另有跨页选择保护和 700ms debounce，这是 selection navigation，不是嵌套横滚仲裁。[`paginator.js` L586-L620](https://github.com/johnfactotum/foliate-js/blob/78914aef4466eb960965702401634c2cb348e9b1/paginator.js#L586-L620)。

epub.js 的 Snap 也从外层 scroller 和每个 iframe 接收 touch。move 时只比较**本次**纵向增量是否小于 10px，满足就直接改 `scrollLeft`；它没有累计轴锁、目标保护或内部横滚边界 owner。[`snap.js` L105-L160](https://github.com/futurepress/epub.js/blob/eee359d0790002115a1156a9833c54f4bcd44c1d/src/managers/helpers/snap.js#L105-L160)、[`L171-L205`](https://github.com/futurepress/epub.js/blob/eee359d0790002115a1156a9833c54f4bcd44c1d/src/managers/helpers/snap.js#L171-L205)。这种判断对噪声方向变化更敏感，不能反向证明 Atha 应简化。

### KOReader：完整状态机，但命中语义来自自有引擎

KOReader 的 contact 从 tap state 开始，DPI 缩放后达到 35px pan 阈值才进入 pan state；hold、double-tap、multi-touch、pan、swipe 分别是显式状态。[`gesturedetector.lua` L59-L121](https://github.com/koreader/koreader/blob/e9c0a6e3999726eec20413e2b367021d7130809e/frontend/device/gesturedetector.lua#L59-L121)、[`L660-L793`](https://github.com/koreader/koreader/blob/e9c0a6e3999726eec20413e2b367021d7130809e/frontend/device/gesturedetector.lua#L660-L793)。方向斜率用 0.577 / 1.732 分类；900ms 内结束的 pan 可转为 swipe。[`gesturedetector.lua` L315-L361](https://github.com/koreader/koreader/blob/e9c0a6e3999726eec20413e2b367021d7130809e/frontend/device/gesturedetector.lua#L315-L361)。

它的 reader module 还保留 pan 起点：若随后被更高优先级 swipe/menu 处理，就恢复原位置；已经实际滚动的短 pan 则不再重复当翻页 swipe。[`readerrolling.lua` L541-L573](https://github.com/koreader/koreader/blob/e9c0a6e3999726eec20413e2b367021d7130809e/frontend/apps/reader/modules/readerrolling.lua#L541-L573)、[`L575-L687`](https://github.com/koreader/koreader/blob/e9c0a6e3999726eec20413e2b367021d7130809e/frontend/apps/reader/modules/readerrolling.lua#L575-L687)。

可借鉴的是“阈值后固定状态、被上层接管时有明确回滚”；不可移植的是其命中数据，因为文字、图片、表格和公式已由 CREngine 排版成内部坐标，不存在 Web DOM 嵌套滚动链。

### 旧 Readium JS：发布成熟不等于手势先进

`v1.25.7` 只记录 touchstart / touchend；松手时把位移除以 DPR，要求横向至少 80、总时长不超过 500ms、斜率不超过 0.5，再发 IPC 翻页。没有 touchmove 跟手，也没有内容目标仲裁。[`preload.ts` L243-L359](https://github.com/readium/r2-navigator-js/blob/01a5d14e44b2daab78ea16270b35a2fe9c36490a/src/electron/renderer/webview/preload.ts#L243-L359)。它适合作为“长期项目也可能保留保守 fallback”的证据，不是 Atha 的优化模板。

## 2. 拖动跟手、提交阈值与动画

| 实现 | 跟手路径 | 松手决策 | 收束 | 主要代价/风险 |
| --- | --- | --- | --- | --- |
| Atha | move 仅存 delta；每帧最多一次 `transform` 写 | 固定 48px + 横轴占优 | 150ms CSS transform transition | 阈值未随屏宽/速度变化；整章 transform 是否形成超大 layer 需 trace |
| Readium Kotlin | Compose 2D scroll 把位移分配给 WebView/Pager | 低速看 50% page 与最小 56dp；400dp/s 以上按速度方向 | decay + medium-low spring | 体系成熟但新 rendition 实验性；Compose/WebView 多层，不能原样移植 |
| Foliate | 每次 touchmove 写原生 scroll offset | 最近速度投影到 page 中点 | 300ms JS rAF `easeOutQuad` | 全文档接管；每帧 scroll 写和 scroll 事件；没有稳定轴 owner |
| epub.js | touchmove 写 `scrollLeft` | 10px 最小距离 + 0.2px/ms；否则四舍五入最近页 | 80ms JS rAF cubic | 判断依赖最后增量；直接 scroll 管线；无内容 target 仲裁 |
| KOReader | classic scroll 跟手，昂贵 xpointer 延迟到 release | 状态机内按时间将 pan 转 swipe；滚动过则不再翻页 | 30Hz 惯性，摩擦递减，可触摸中断 | 为 e-ink/位图绘制调度，帧模型不同 |
| Readium JS | 无跟手 | 80px、500ms、slope 0.5 | 直接页切换 | 手感反馈最弱，不能作为优化目标 |

Atha 的具体热路径在起手缓存 reader rect、DPR layout scale、表格 `scrollLeft` / maximum；move 只进入 `previewOverflow()` 或 `previewSwipe()` 的单 rAF 写。[`interaction.mjs` L128-L156](../../reader/web/interaction.mjs#L128-L156)、[`L261-L309`](../../reader/web/interaction.mjs#L261-L309)。`pagination.previewSwipe()` 不读 geometry，`showPage()` 才在 release/cancel 后恢复最终 transform。[`pagination.mjs` L265-L287](../../reader/web/pagination.mjs#L265-L287)、[`L600-L619`](../../reader/web/pagination.mjs#L600-L619)。CSS 仅在 dragging 时设置 `will-change`，settling 时才设置 150ms transition；reduced-motion 会取消 transition。[`atha-reader.css` L975-L996](../../reader/atha-reader.css#L975-L996)、[`L2745-L2756`](../../reader/atha-reader.css#L2745-L2756)。

这里也存在一个需要新 benchmark 证伪的细缝：overflow owner 在松手时会调用 `finishOverflow()`，主动 flush 尚未执行的 rAF；page owner 没有对应的 `finishSwipe()`。若最后一次 pointermove 与 pointerup 落在同一帧，后续 `showPage()` 会先取消待执行 rAF，再直接进入最终页 transition，最后一段跟手位移可能从未呈现。[`interaction.mjs` L152-L156](../../reader/web/interaction.mjs#L152-L156)、[`pagination.mjs` L265-L278](../../reader/web/pagination.mjs#L265-L278)。当前 GUI 门固定生成 10 个、每个 16ms 的 move，并要求至少 6 次 rAF transform，因此很擅长测稳定拖动，却可能掩盖稀疏快速 flick。[`check-fb2-source.ps1` L130-L161](../../scripts/check-fb2-source.ps1#L130-L161)、[`L288-L313`](../../scripts/check-fb2-source.ps1#L288-L313)。这不是立刻改实现的结论，但应在速度候选前先增加 1-3 move + 立即 release 的测试。

Readium 的可借鉴阈值更完整：先用平台 `ViewConfiguration` touch slop，mouse slop 仅为 touch 比例的 0.125dp / 18dp；手势被别的 detector consume 时取消，而不是继续抢占。[`DragGestureDetector.kt` L81-L138](https://github.com/readium/kotlin-toolkit/blob/f8e6f93db81570c7cc0833279b2628f4c65d8efe/readium/navigators/web/internals/src/main/kotlin/org/readium/navigator/web/internals/gestures/DragGestureDetector.kt#L81-L138)、[`L240-L285`](https://github.com/readium/kotlin-toolkit/blob/f8e6f93db81570c7cc0833279b2628f4c65d8efe/readium/navigators/web/internals/src/main/kotlin/org/readium/navigator/web/internals/gestures/DragGestureDetector.kt#L240-L285)、[`L362-L378`](https://github.com/readium/kotlin-toolkit/blob/f8e6f93db81570c7cc0833279b2628f4c65d8efe/readium/navigators/web/internals/src/main/kotlin/org/readium/navigator/web/internals/gestures/DragGestureDetector.kt#L362-L378)。低速落页看 0.5 page 和最多 56dp 的位置阈值，高速从 400dp/s 起强制按速度方向落相邻页。[`PagingFlingBehavior.kt` L52-L67](https://github.com/readium/kotlin-toolkit/blob/f8e6f93db81570c7cc0833279b2628f4c65d8efe/readium/navigators/web/internals/src/main/kotlin/org/readium/navigator/web/internals/pager/PagingFlingBehavior.kt#L52-L67)、[`L232-L316`](https://github.com/readium/kotlin-toolkit/blob/f8e6f93db81570c7cc0833279b2628f4c65d8efe/readium/navigators/web/internals/src/main/kotlin/org/readium/navigator/web/internals/pager/PagingFlingBehavior.kt#L232-L316)、[`L332-L340`](https://github.com/readium/kotlin-toolkit/blob/f8e6f93db81570c7cc0833279b2628f4c65d8efe/readium/navigators/web/internals/src/main/kotlin/org/readium/navigator/web/internals/pager/PagingFlingBehavior.kt#L332-L340)。

Foliate 的速度投影比“仅距离”更接近快速短 flick 的直觉：它用最后速度乘 page size，和当前 viewport 中点一起选择 page。[`paginator.js` L787-L822](https://github.com/johnfactotum/foliate-js/blob/78914aef4466eb960965702401634c2cb348e9b1/paginator.js#L787-L822)。但其最后一个 move 的速度容易受采样抖动影响；Atha 若引入速度，应保留一个短时间窗口或加权样本，不复制单样本算法。

Foliate 还有一个低成本生命周期细节：文档隐藏时动画直接写终值，不继续等待 rAF。[`paginator.js` L17-L37](https://github.com/johnfactotum/foliate-js/blob/78914aef4466eb960965702401634c2cb348e9b1/paginator.js#L17-L37)。Atha 当前 visibilitychange 只收束滚动位置，不会主动清除分页 transition timer。[`pagination.mjs` L645-L669](../../reader/web/pagination.mjs#L645-L669)。可把“隐藏立即落终值”列为小候选，但只有生命周期日志证明后台/恢复残留 settling 时才改。

## 3. 分页、双模式与重排控制

### CSS columns 是共同基础，资源模型不同

Atha 与 Foliate 都在单个当前 section 内用 CSS columns。Atha 计算 `pageStep = width + gap`，总页数来自 `scrollWidth`，并用整章 translateX 定位。[`pagination.mjs` L289-L312](../../reader/web/pagination.mjs#L289-L312)。Foliate 把 iframe 轴长度补齐到 page 整数倍，并在页面两端留虚拟空间；Readium 则为多列 spread 追加空的 virtual column，确保 resource width 是 viewport 整数倍。[`paginator.js` L362-L389](https://github.com/johnfactotum/foliate-js/blob/78914aef4466eb960965702401634c2cb348e9b1/paginator.js#L362-L389)、[`columns.ts` L1-L53](https://github.com/readium/kotlin-toolkit/blob/f8e6f93db81570c7cc0833279b2628f4c65d8efe/readium/navigators/web/internals/scripts/src/util/columns.ts#L1-L53)。

epub.js 也将 reflowable paginated 内容格式化为 columns，scroll flow 则保留自然尺寸；page count 由总轴长度除以 page length。[`layout.js` L22-L28](https://github.com/futurepress/epub.js/blob/eee359d0790002115a1156a9833c54f4bcd44c1d/src/layout.js#L22-L28)、[`L192-L229`](https://github.com/futurepress/epub.js/blob/eee359d0790002115a1156a9833c54f4bcd44c1d/src/layout.js#L192-L229)。它的 iframe expansion 会把测得宽高向整页上取整，再决定是否 reframe。[`iframe.js` L283-L334](https://github.com/futurepress/epub.js/blob/eee359d0790002115a1156a9833c54f4bcd44c1d/src/managers/views/iframe.js#L283-L334)。

共同点并不意味着应统一实现：Atha 是一个受控、净化后 DOM 的单 WebView；Readium 是多 resource WebView + Pager；Foliate 是单 iframe 替换；epub.js continuous manager 可同时保留多个 iframe。缓存和预载成本因此完全不同。

### 双模式

Atha 的 paged mode 使用 columns + transform，scroll mode 取消 columns，让 reader 原生 `overflow-y: auto`，并以 passive touch 只处理 section 边界；不在滚动中接管 move。[`atha-reader.css` L926-L1023](../../reader/atha-reader.css#L926-L1023)、[`interaction.mjs` L374-L402](../../reader/web/interaction.mjs#L374-L402)。

Foliate 也用 `flow === scrolled` 在同一 View 中切换自然流和 column flow，且滚动/分页都维护同一个可见 range anchor。[`paginator.js` L285-L340](https://github.com/johnfactotum/foliate-js/blob/78914aef4466eb960965702401634c2cb348e9b1/paginator.js#L285-L340)、[`L716-L752`](https://github.com/johnfactotum/foliate-js/blob/78914aef4466eb960965702401634c2cb348e9b1/paginator.js#L716-L752)。Readium 的 LayoutResolver 也分别为 scroll 计算单列最大行长，为 paged 自动或指定列数，并在竖排文字时强制 scroll。[`LayoutResolver.kt` L33-L89](https://github.com/readium/kotlin-toolkit/blob/f8e6f93db81570c7cc0833279b2628f4c65d8efe/readium/navigators/web/reflowable/src/main/kotlin/org/readium/navigator/web/reflowable/layout/LayoutResolver.kt#L33-L89)、[`L91-L188`](https://github.com/readium/kotlin-toolkit/blob/f8e6f93db81570c7cc0833279b2628f4c65d8efe/readium/navigators/web/reflowable/src/main/kotlin/org/readium/navigator/web/reflowable/layout/LayoutResolver.kt#L91-L188)。

这三者共同支持 Atha 当前决策：分页和滚动应是两个明确模式，scroll 继续交给平台原生滚动；不要在 paged owner 上叠加“可随时切换为任意方向 pan”的复杂状态。

### 重排与 Locator

Atha 字号/viewport 变化会先捕获文本 offset，等待 `document.fonts.ready`，重排后最多等 20 帧取得连续 2 帧相同 signature，再把 offset 映射回 page。[`pagination.mjs` L353-L390](../../reader/web/pagination.mjs#L353-L390)、[`L484-L503`](../../reader/web/pagination.mjs#L484-L503)。同一稳定页的 offset 按 `page:<index>` / `scroll:<top>` 缓存，避免一次翻页后重复扫描全章。[`pagination.mjs` L101-L107](../../reader/web/pagination.mjs#L101-L107)。

Readium 将 CSS properties 在 HTML 首次 layout pass 前直接注入，避免加载后再通过 JS 改样式；resource resize 时会保持当前/相邻 resource 边界并回补位移。[`ReadiumCssInjector.kt` L189-L203](https://github.com/readium/kotlin-toolkit/blob/f8e6f93db81570c7cc0833279b2628f4c65d8efe/readium/navigators/web/reflowable/src/main/kotlin/org/readium/navigator/web/reflowable/css/ReadiumCssInjector.kt#L189-L203)、[`RenditionScrollState.kt` L174-L193](https://github.com/readium/kotlin-toolkit/blob/f8e6f93db81570c7cc0833279b2628f4c65d8efe/readium/navigators/web/internals/src/main/kotlin/org/readium/navigator/web/internals/pager/RenditionScrollState.kt#L174-L193)。它还只为显式标记 preload 的字体生成 `<link rel=preload>`，不是把所有字体都预热。[`ReadiumCssInjector.kt` L278-L283](https://github.com/readium/kotlin-toolkit/blob/f8e6f93db81570c7cc0833279b2628f4c65d8efe/readium/navigators/web/reflowable/src/main/kotlin/org/readium/navigator/web/reflowable/css/ReadiumCssInjector.kt#L278-L283)。

Foliate 观察 body resize，并在字体 ready 后再次 expand；样式变化后用 rAF 更新背景并再等字体确认尺寸。[`paginator.js` L210-L283](https://github.com/johnfactotum/foliate-js/blob/78914aef4466eb960965702401634c2cb348e9b1/paginator.js#L210-L283)、[`L1100-L1116`](https://github.com/johnfactotum/foliate-js/blob/78914aef4466eb960965702401634c2cb348e9b1/paginator.js#L1100-L1116)。epub.js 也有 ResizeObserver + rAF resize check，但它的 `listeners()` 当前把 `fontLoadListeners()` 注释掉，不能简单宣称它会自动处理字体完成。[`contents.js` L378-L405](https://github.com/futurepress/epub.js/blob/eee359d0790002115a1156a9833c54f4bcd44c1d/src/contents.js#L378-L405)、[`L523-L583`](https://github.com/futurepress/epub.js/blob/eee359d0790002115a1156a9833c54f4bcd44c1d/src/contents.js#L523-L583)。

对 Atha 的结论是：首帧前注入稳定样式、显式字体 ready、有限稳定窗口都应保留。不要直接加常驻全 DOM MutationObserver；若真实书证明异步尺寸漂移仍存在，优先在“当前 section 尚有 pending resource 或设置刚变更”的有限窗口启用 ResizeObserver，并继续用 signature 去重。

## 4. 重图片、公式、表格的加载与缓存

### Atha 当前：按可见范围解码，失败才重排

Atha 先验证 SVG，固定尺寸公式延迟设置 `src`；`loadVisible()` 只查当前或下一页 bounds。成功 decode 只显现，不宣称 layout changed；失败替换才在首次 DOM 改动前捕获 Locator，并最多做 4 个可见 pass。[`content.mjs` L410-L495](../../reader/web/content.mjs#L410-L495)。当前 section 剩余资源在 idle turn 中每批 16 个预热并主动让出事件循环，没有预载相邻 section。[`content.mjs` L497-L520](../../reader/web/content.mjs#L497-L520)。

普通图片被限制在 page 可用宽高，公式使用已知源宽高按字号缩放；表格、代码和 figure 放进页内 `overflow:auto` 容器并 `break-inside: avoid`。[`pagination.mjs` L245-L262](../../reader/web/pagination.mjs#L245-L262)、[`atha-reader.css` L1091-L1149](../../reader/atha-reader.css#L1091-L1149)。这与本轮目标匹配：先保证当前重内容页跟手，不用更多 DOM/位图换取未经证明的边界提速。

### Foliate：单 section、单 iframe，低常驻内存

Foliate 每次只创建当前 View；切 section 后卸载旧 section。图像、SVG、video 按可用 page 尺寸 clamp，`object-fit: contain` 且避免列内分割。[`paginator.js` L341-L360](https://github.com/johnfactotum/foliate-js/blob/78914aef4466eb960965702401634c2cb348e9b1/paginator.js#L341-L360)、[`L971-L1020`](https://github.com/johnfactotum/foliate-js/blob/78914aef4466eb960965702401634c2cb348e9b1/paginator.js#L971-L1020)。Foliate 3.2.1 发布说明还明确记录“不再整本载入内存”和按 pagebreak 拆分大 Mobipocket section 的性能收益。

其代价是 section 边界可能等待 load，且当前 upstream paginator 没有 Atha 的 deferred formula / SVG validation 分层。可借鉴的是“限制 section 粒度和旧 view 生命周期”，不是复制 iframe 架构。

### Readium Kotlin：两侧预留资源，边界平滑但有潜在内存成本

`ReflowableWebRendition` 将 `beyondViewportPageCount` 固定为 3，即 Pager 可组合当前资源两侧各三个 resource WebView。[`ReflowableWebRendition.kt` L121-L150](https://github.com/readium/kotlin-toolkit/blob/f8e6f93db81570c7cc0833279b2628f4c65d8efe/readium/navigators/web/reflowable/src/main/kotlin/org/readium/navigator/web/reflowable/ReflowableWebRendition.kt#L121-L150)。每个 resource 都设置 hardware layer；document resized 后的位置校正由外层 scroll state 完成。[`ReflowableResource.kt` L415-L446](https://github.com/readium/kotlin-toolkit/blob/f8e6f93db81570c7cc0833279b2628f4c65d8efe/readium/navigators/web/reflowable/src/main/kotlin/org/readium/navigator/web/reflowable/resource/ReflowableResource.kt#L415-L446)、[`RenditionScrollState.kt` L174-L193](https://github.com/readium/kotlin-toolkit/blob/f8e6f93db81570c7cc0833279b2628f4c65d8efe/readium/navigators/web/internals/src/main/kotlin/org/readium/navigator/web/internals/pager/RenditionScrollState.kt#L174-L193)。

这是边界平滑优先的选择，但其潜在成本可能是至多七个已组合 WebView、多个 DOM、字体和 layer；本轮没有运行 Readium，不能把这一风险写成实测开销。Atha 当前是单 WebView/当前 section；在没有 Android `dumpsys meminfo`、边界首帧和长会话 P95 前，不能把 `3` 解释成行业最佳值。若以后证实 section 边界加载是主瓶颈，Atha 候选上限应先是**一个**已净化相邻 section，且可取消、可逐出、有内存门。

### epub.js continuous：窗口化多个 iframe

epub.js continuous manager 默认 lookahead 500px、offset delta 250px；接近首尾就 prepend / append resource。可见 iframe 才 show，滚动中把离屏 iframe 隐藏，动量停止后 trim，只保留可见区附近的一前一后。[`continuous/index.js` L7-L50](https://github.com/futurepress/epub.js/blob/eee359d0790002115a1156a9833c54f4bcd44c1d/src/managers/continuous/index.js#L7-L50)、[`L210-L285`](https://github.com/futurepress/epub.js/blob/eee359d0790002115a1156a9833c54f4bcd44c1d/src/managers/continuous/index.js#L210-L285)、[`L287-L408`](https://github.com/futurepress/epub.js/blob/eee359d0790002115a1156a9833c54f4bcd44c1d/src/managers/continuous/index.js#L287-L408)。scroll check 通过队列和 debounce 串行，trim 会等 momentum delta 降下来。[`continuous/index.js` L470-L568](https://github.com/futurepress/epub.js/blob/eee359d0790002115a1156a9833c54f4bcd44c1d/src/managers/continuous/index.js#L470-L568)。

这个窗口化思想适合连续滚动的大 publication，却也带来 iframe show/hide、队列和 scroll listener 成本。Atha 已明确只有当前 section DOM，scroll 模式交给当前 reader；除非产品要做真正连续跨章滚动，否则不应引入这套 manager。

epub.js archive 还会把每个资源生成的 Blob/Base64 URL 保存在 `urlCache`，直到显式 revoke 或整本 destroy。[`archive.js` L183-L252](https://github.com/futurepress/epub.js/blob/eee359d0790002115a1156a9833c54f4bcd44c1d/src/archive.js#L183-L252)。Atha 有受控协议和 `readySvg` 生命周期，不应复制全书无预算 URL cache。

### KOReader / CREngine：有界位图、禁用实验与后台重排

KOReader 的 page/tile cache key 包含文件修改时间、页、zoom、rotation、gamma、清理模式、render mode、颜色、reflow 字号和 saturation；cache 命中还检查有效时间。内存不足时优先只渲染请求 fragment，而不是强塞完整页。[`document.lua` L393-L471](https://github.com/koreader/koreader/blob/e9c0a6e3999726eec20413e2b367021d7130809e/frontend/document/document.lua#L393-L471)、[`L474-L529`](https://github.com/koreader/koreader/blob/e9c0a6e3999726eec20413e2b367021d7130809e/frontend/document/document.lua#L474-L529)。全局 DocCache 按 free memory 比例计算预算；少于 8MB 就退化成单 slot，磁盘预算与内存预算相同，持久化时优先当前显示页而非最近 hint 页。[`doccache.lua` L13-L58](https://github.com/koreader/koreader/blob/e9c0a6e3999726eec20413e2b367021d7130809e/frontend/document/doccache.lua#L13-L58)、[`L60-L118`](https://github.com/koreader/koreader/blob/e9c0a6e3999726eec20413e2b367021d7130809e/frontend/document/doccache.lua#L60-L118)。

CREngine 源码还保留两个 slot 的 page image cache，按 current/prev/next page 或 offset 命中，未准备好时 join background render thread；但整个实现受 `CR_ENABLE_PAGE_IMAGE_CACHE` 条件编译控制，该固定提交的 CMake 配置和 Android 默认配置都将它设为 `0`。因此它是被禁用的实验/遗留实现，不是当前成熟机制。[`crsetup.h.cmake` L72-L74](https://github.com/koreader/crengine/blob/98d6d6f7ee1d4e6a175e4c6a3d8e81f7a0adb4f8/crengine/include/crsetup.h.cmake#L72-L74)、[`lvdocview.h` L42-L48](https://github.com/koreader/crengine/blob/98d6d6f7ee1d4e6a175e4c6a3d8e81f7a0adb4f8/crengine/include/lvdocview.h#L42-L48)、[`L77-L170`](https://github.com/koreader/crengine/blob/98d6d6f7ee1d4e6a175e4c6a3d8e81f7a0adb4f8/crengine/include/lvdocview.h#L77-L170)、[`lvdocview.cpp` L630-L724](https://github.com/koreader/crengine/blob/98d6d6f7ee1d4e6a175e4c6a3d8e81f7a0adb4f8/crengine/src/lvdocview.cpp#L630-L724)。

字体或版式改变时，KOReader 可先局部重排当前 fragment，让用户继续读；用户空闲后再 fork 子进程完成 full render，最后从 cache reload。源码注释以“当前约占 120MB 的大书”为例，估算简单后台 rerender 约额外 60MB、完整 reload + render 约额外 130MB，并据此拒绝后者常态化；这不是跨设备通用实测值。[`readerrolling.lua` L1626-L1689](https://github.com/koreader/koreader/blob/e9c0a6e3999726eec20413e2b367021d7130809e/frontend/apps/reader/modules/readerrolling.lua#L1626-L1689)、[`L1720-L1766`](https://github.com/koreader/koreader/blob/e9c0a6e3999726eec20413e2b367021d7130809e/frontend/apps/reader/modules/readerrolling.lua#L1720-L1766)、[`L1900-L1976`](https://github.com/koreader/koreader/blob/e9c0a6e3999726eec20413e2b367021d7130809e/frontend/apps/reader/modules/readerrolling.lua#L1900-L1976)。

KOReader 的现行机制证明“cache 必须由完整 layout inputs 定址、有预算、可失效，后台工作要服从用户输入和内存”；CREngine 默认禁用的两槽实现则提醒我们，小缓存也不自动等于值得启用。但 Atha 若缓存 WebView 截图，会失去可选择文本、焦点、表格滚动和媒体命中；若自建 CREngine，又会改变浏览器兼容与安全信任边界。因此只借原则，不借位图实现。

## 5. Atha 差异与决策矩阵

| 机制 | 成熟实现证据 | Atha 当前 | 差异、成本与决定 |
| --- | --- | --- | --- |
| 一次序列所有权 | KOReader 显式 contact state；Readium Compose cancel consumed gesture | `vertical/overflow/page` 一次认领 | **保留。** Atha 是唯一直接覆盖 DOM 宽表边界的实现；不要退回全局 touch handler |
| 交互命中 | Readium Web click 先保护链接/控件；Foliate 依赖 selection 额外处理 | composedPath + hard target + media/structure 区域 | **保留并扩测试。** 公式是 img、表格/代码有 wrapper；未来 MathML 原生化时需重审 selector |
| 起手阈值 | Readium platform slop；KOReader DPI 35；epub.js 10px | 固定 8 CSS px + 1.5 轴优势 | **候选研究。** 真机收集 pointerType / DPR / 抖动后，再决定按 pointer 类型或物理尺寸调整 |
| 松手提交 | Readium 56dp/50% + 400dp/s；Foliate velocity projection；epub.js 10px + 0.2px/ms | 横向 48px 且横轴占优 | **优先 A/B 候选。** 增加短窗口 velocity，不删距离 guard；需覆盖快速短 flick、慢长拖和反向回拉 |
| move 热路径 | Foliate/epub.js 每 move 写 scroll；KOReader 延迟 xpointer | 单 rAF transform/scrollLeft 写，零 layout read；page release 未显式 flush 待执行帧 | **保留架构、补稀疏 flick 门。** 先确认末位移是否丢帧；只在 trace 证明超大 layer 时换策略 |
| 收束动画 | Readium spring；Foliate 300ms JS；epub.js 80ms JS；KOReader 设备相关 | 150ms CSS transition，reduced-motion 关闭 | **保留默认。** 可单独研究 hidden 时立即完成；不引入 JS tween |
| scroll/page 双模式 | Readium、Foliate、epub.js 均显式两模式 | paged columns；scroll 原生纵向 | **保留。** 不把滚动模式也塞进 pointer owner move |
| 字体/异步重排 | Readium first-pass 注入；Foliate RO + fonts；KOReader rendering hash | fonts.ready + 20 帧有限稳定 + Locator offset | **保留并观测。** 只有异步漂移实证后才加有限期 ResizeObserver |
| 当前/下一页资源 | Foliate clamp；Atha 特有 deferred formula | 可见 bounds decode，失败才 layoutChanged | **保留。** 成功固定尺寸公式不重排是直接针对重内容的优势 |
| section 预载 | Readium 两侧各 3 WebView；epub.js 500px 窗口；Foliate 当前单 section | 不预载相邻 section；当前 section idle 批 16 | **先测后做。** 边界 P95 超预算才试 1 个 sanitized section；同时设内存/取消门 |
| page bitmap cache | KOReader 现行 LRU/tile；CREngine 两 slot 源码在当前构建默认禁用 | 无 | **拒绝当前引入。** 破坏 DOM 交互且显著占内存；只能在 compositor trace 证明整章 paint 是主瓶颈后重开研究 |
| cache key/失效 | KOReader 完整 rendering hash | page/scroll offset cache、SVG validation/ready cache | **借鉴原则。** 后续任何预载都必须含 section、样式、字号、viewport、资源 generation 并有逐出 |
| resize | Readium 资源边界校正；Foliate anchor + RO；Readium JS 200ms debounce | 120ms debounce + Locator relayout | **保持。** 增加 orientation/连续 resize 漂移 benchmark，不复制旧 Readium 的纯 debounce |
| 书页末尾补齐 | Readium virtual column；Foliate view 向整页扩展 | `round((scrollWidth+gap)/pageStep)` | **观察而非立即改。** 现有 layout gate 未报告尾页 snap 错；出现多栏尾页问题再引入显式空列 |

## 6. 测试与 benchmark 成熟度

“项目有很多用户”不能替代“本机制有直接测试”。本轮固定 snapshot 的实际覆盖如下：

| 项目 | 找到的直接证据 | 缺口 |
| --- | --- | --- |
| Atha | `check-fb2-source.ps1` 13 个可信 W3C Pointer Actions 场景；普通/公式压力各 5 次预热 + 20 次测量；输入到首视觉、frame P95/max、release-to-stable 有硬门；table 中部与两侧边界均测 | Linux WebKitGTK 实际 `pointerType=mouse`；PCT-AL10 手指、代码块等价矩阵、速度分层和长会话内存未覆盖 |
| Readium Kotlin | 当前 `readium/navigators` 目录未找到 `Test.kt` / benchmark 对 `RenditionScrollState`、`PagingFlingBehavior`、drag detector 的直接覆盖 | 新 rendition 标为 experimental；源码策略强，但本 snapshot 不能证明阈值回归门 |
| Readium JS | 只找到 CFI lexer/parser/resolver 等 `.spec.ts` | 没有 touch swipe、resize/wheel 或帧性能直接 spec |
| foliate-js | `tests/` 只覆盖 EPUB CFI | paginator touch、selection 冲突、resize、动画和大 section 无自动 benchmark；`package.json` 只有 build script |
| epub.js | Karma/Mocha 覆盖 Book、CFI、Locations、Section、core URL/path | 未找到 continuous manager、Snap、ResizeObserver 或内存窗口直接测试；`lint` script 还显式 `exit 0` |
| KOReader | `gesturedetector_spec.lua` 测旋转坐标；`readerrolling_spec.lua` 测页导航、横竖屏与字号/词距重排页数；`cache_spec.lua` 测序列化/反序列化；另有通用 bench 脚本 | gesture spec 没覆盖 pan/swipe 阈值状态机；cache spec 只跑最多 1 页；没有与 Atha 同类触摸帧 P95 |
| CREngine | 当前 `tests/` 主要是字体人工/视觉 test plan 和 EPUB generator | 未找到 page image cache、partial reflow、MathML 手势/性能的直接自动 benchmark |

Atha 当前性能门的已记录结果是：普通章节最差输入首视觉 32ms、frame P95/max 17/17ms、release stable 212ms；公式压力对应 30ms、25/25ms、216ms，均在门内。[`reader-gesture-performance.md` L90-L94](../changes/reader-gesture-performance.md#L90-L94)。最高证据仍是 Linux 真实 GUI + 可信自动化指针，不是真实触摸。

### 下一轮 benchmark 应先于实现

1. **速度与距离二维矩阵。** 慢拖 20/40/60/120px、仅 1-3 个 move 后立即 release 的快速短 flick、反向回拉、边界半途停手；记录最近 80-120ms 速度窗口、最后 move 是否得到视觉帧、最终方向和是否恰好一页。先采样，不先改阈值。
2. **真实触摸噪声。** 用户在 PCT-AL10 运行相同图片、公式、窄表、宽表中部/边界、代码块矩阵；记录实际 pointerType、DPR、refresh rate、首个 owner 的距离与轴比。Linux mouse 可信事件不能校准 touch slop。
3. **长会话内存。** 普通书和公式压力各连续前后翻 100 页/跨 20 个 section；记录进程 PSS/RSS、WebView renderer、DOM node、pending/ready resource、compositor layer 数。停止条件应是稳定平台上三轮可重复增长，而非一次峰值。
4. **边界加载。** 分开记录 section 内页翻转与跨 section 的 input-to-first-content / stable；只有跨 section P95 明显越过 220ms 且当前页帧仍合格，才进入相邻 section 预载实验。
5. **重排定位漂移。** 16/19/40 字号、portrait/landscape、连续 resize、字体延迟和图片失败替换；以原文本 offset/CFI 是否仍可见为 correctness gate，同时记录 blank frame 和重排时间。
6. **超大 layer 证伪。** 在最大合法 section 上抓 WebKit/Android trace，区分 JS、style/layout、paint、composite、GPU memory；只有 transform 期间 paint 或 layer upload 占主导，才比较 scrollLeft、DOM page window 或相邻 snapshot。
7. **生命周期。** drag/settle 中切后台再恢复，检查 settling attribute、最终页、兼容 click 抑制和 Locator；用它决定是否采纳 Foliate 的 hidden-immediate settle。

## 7. 建议路线

### 现在保持不动

- 保持一次序列一个 owner、宽表方向边界移交、拖后兼容 click 抑制；这是本轮项目中最贴合 Atha DOM 的机制。
- 保持 move 热路径 rAF 合并、只写 transform/scrollLeft、drag 期临时 `will-change`、150ms CSS 收束和 reduced-motion。
- 保持 scroll 模式原生纵向滚动、分页/滚动显式双模式、navigation 最终动作串行。
- 保持 visible formula decode、失败才 `layoutChanged`、稳定页 offset cache 和当前 section idle 小批预热。
- 保持现有 5 + 20 性能门；上游缺少直接 benchmark 不是降低 Atha 门槛的理由。

### 研究后可做的低风险候选

1. **速度辅助提交。** 在现有距离/轴 owner 上增加短窗口速度，不允许单个最后 move 决定；分别固定慢长拖、快速短 flick、反向回拉和 diagonal 的 red/green oracle。此候选改变手感，需独立 change 和真机验收。
2. **按 pointer 类型校准 slop。** Readium 的 system slop 证明固定 8px 不是唯一成熟选择，但 Linux 把可信 touch 请求报告为 mouse；应先在 PCT-AL10 收集分布，再决定 touch/mouse/pen 阈值，不能照搬 18dp。
3. **隐藏时立即收束。** 若 lifecycle benchmark 出现后台 transition 残留，visibility hidden 时取消 timer、写最终 transform、清 attribute；改动小，可独立验证。
4. **有限期 ResizeObserver。** 只在 pending resource、字体或设置重排窗口观察当前 book；rAF 合并并比较 signature，无变化不触发 Locator。不要常驻 MutationObserver。
5. **单相邻 section 预载实验。** 仅在边界加载 P95 是主瓶颈时做；预载必须完成相同安全验证，带 generation cancellation、一个 section 上限、明确 memory budget 和快速逐出。

### 明确拒绝或延期

- 不复制 Readium 两侧各三个 WebView；Atha 先没有证明边界收益能抵消多 DOM、多 layer 和字体内存。
- 不复制 Foliate/epub.js 全文档 touchmove 接管；它会退化表格、图片、选择和纵向意图仲裁。
- 不复制旧 Readium JS 的 end-only 80px swipe；它会失去跟手反馈。
- 不复制 KOReader 的位图页缓存，也不启用 CREngine 默认禁用的两槽实现；后台 fork 重排或自有 MathML 排版同样会改变 Atha 的 WebView 浏览器保真、选择、焦点和信任边界。
- 不引入无预算 Blob URL/DOM/截图缓存，不永久设置 `will-change`，不把“预载更多”当默认性能优化。
- 不因 Foliate/epub.js 使用 scrollLeft 就直接替换 transform；先用 compositor trace 判断当前瓶颈。

## 最终判断

Atha 当前最强的部分是内容级手势仲裁和直接性能门；最需要继续学习的是平台化阈值、资源边界预热的预算、异步重排的有限观察窗口，以及缓存失效纪律。成熟项目没有给出一个可直接拷贝的统一答案：

- Readium Kotlin 对跨 resource scroll/fling 最系统，但新 rendition 实验性，潜在多 WebView 成本仍需实测；
- Foliate 的单 section 生命周期、resize/font 收束和速度投影值得小范围借鉴，但 touch target 仲裁明显不够；
- KOReader 的状态机、哈希和有界缓存成熟，但渲染架构不可移植；CREngine 的两槽缓存代码默认禁用，只能作为取舍反例；
- epub.js 的 iframe windowing 提供连续滚动参考，Snap 和测试覆盖却不应作为 Atha 的质量上限；
- 旧 Readium JS 只说明兼容 fallback 的保守边界，不代表现代手感。

下一步应先完成真实触摸、速度、长会话内存、跨 section 和重排漂移五组数据。只有数据指向具体瓶颈，才在“速度辅助提交”“有限期 observer”“单相邻 section 预载”中逐个做独立 A/B；不要同时改变手势语义、渲染模型和缓存策略。

## 证据限制

本报告最高证据是 Atha 当前源码与已有 Linux GUI 结果、上游固定提交的静态代码审计、官方文档/发布说明。没有运行上游应用，没有在 Android 真机复现其行为，也没有重新跑 Atha Linux GUI 性能门。上游源码中“未找到测试”只表示本次固定 checkout 的可见测试树未发现直接覆盖，不能外推为项目私有 CI 或未检出的外部仓库绝无测试。

---
description: 固定 Readest 与其 foliate-js 分支源码，复核 EPUB 恢复定位、图片加载与解码、布局稳定、损坏资源容错及表格公式进入策略。
---

# Readest EPUB 图片恢复与兼容性源码复核

## 后续状态

本文的 Readest 机制仍固定在所列提交；实施后的 Atha 已在 EPUB v5 为符合边界的无尺寸本地图片写入原生尺寸，并以有界 CSS 固有尺寸规则稳定解码前几何。下文对 Atha pipeline 的未决问题只代表研究时快照，当前行为与残余边界以 `docs/changes/reader-gesture-performance.md` 和 `docs/architecture/READER-CORE.md` 为准。

## 结论

截至 2026-08-09，对 Readest 当前 `main`、最新正式版和其 foliate-js 分支逐段复核后，可以明确回答最关键的问题：

1. **Readest 不会在恢复位置前统一等待“当前可见图片全部 `decode()` 完成”。** 它先把 XHTML/CSS 引用的本地资源从 ZIP 解出并改写为 Object URL，再等待目标 iframe 的 `load`，随后排版、解析 CFI anchor 并滚到目标位置。普通 `<img>` 没有显式 `decode()` 门，迟到布局由 `ResizeObserver` 和重新锚定兜底。
2. **唯一明确的图片等待上限是正文 `background-image` 的 3000ms 特例。** 这是为滚动模式恢复时“上一节全页插图迟到，导致目标章节向下漂移”而加。成功先取得自然尺寸；`error` 立即降级为无背景；既不 `load` 也不 `error` 时最多等 3 秒，之后先显示正文，图片若更晚成功再扩展。这一机制已经进入 v0.11.20。
3. **首个 `stabilized` 只表示主 section 已排版、已应用 anchor、容器已可见，不表示相邻 section、字体和所有迟到资源已经完成。** 后续最多补齐 5 页、8 个 section 的相邻内容，并再次发 `stabilized`。Readest 应用收到第一次事件就关闭 loading。
4. **Readest 的容错是局部而非完整。** 无效 XHTML 会退回 HTML；未列入 manifest 但 ZIP 中实际存在的常见图片可继续加载；相邻 section 失败不会阻塞当前页；当前分支还跳过不可用 iframe 文档。但 ZIP 解压、递归资源替换、iframe `load` 没有通用 deadline，主 section 失败也没有可靠的显式“降级就绪”状态。
5. **当前分支比 v0.11.20 多两个值得吸收的恢复修复。** 一是无 client rect 的绝对定位全页图片回退到 section 起点并补齐 scroll bounds，避免该页滑动失效；二是不可用 iframe 文档不再继续读 `body`。两者都是 v0.11.20 发布后的变更，不能把当前源码能力误称为正式版已具备。
6. **表格和公式的布局包装值得借，手势所有权不应照抄。** Readest 只包装 table 与 display MathML，并用 `cfi-skip` 保持定位；一次性 `ResizeObserver` 判断是否真溢出。可它只要某轴存在 overflow，就连在边界也永久吞掉该轴手势，这正会复现 Atha 用户反馈的“表格上无法翻页”。

对 Atha 最合适的方案不是“全量等图片”，而是：主 section 快速可见，**只对会改变目标位置的可见资源做有界等待**；超时或失败进入可观测的 degraded-ready；保存逻辑 anchor 并在迟到布局时按 generation 重新锚定；任何异步阶段都必须有全局 deadline 和 stale-load guard。

## 证据边界与源码锚点

本报告使用以下标记：

- **[M]** Readest 当前 `main` 或当前 foliate-js 分支源码事实；
- **[R]** Readest v0.11.20 正式版源码事实；
- **[S]** Web 标准或成熟依赖的官方文档；
- **[I]** 由调用顺序推出、仍需 Atha 真实运行测试验证的设计判断。

固定快照如下：

| 对象 | 固定提交 | 用途 |
| --- | --- | --- |
| Readest 当前 `main` | [`6d5a89ceeedcaec9422e91002c7e22af9cbedf68`](https://github.com/readest/readest/tree/6d5a89ceeedcaec9422e91002c7e22af9cbedf68)，2026-08-09 | 当前应用接线、ZIP loader 与测试 |
| 当前 foliate-js gitlink | [`f65836f77e8b66b84baacd54bfc92096578e7a84`](https://github.com/readest/foliate-js/tree/f65836f77e8b66b84baacd54bfc92096578e7a84)，2026-08-07 | 当前渲染、恢复、布局稳定和错误分支 |
| Readest v0.11.20 | [`1df1505fc5033fc949463c9908f2d53bd0fbdfa6`](https://github.com/readest/readest/tree/1df1505fc5033fc949463c9908f2d53bd0fbdfa6)，2026-07-20 | 最新正式版边界 |
| v0.11.20 foliate-js | [`dd71f2be356563c16a23272686189fcfb45d0b82`](https://github.com/readest/foliate-js/tree/dd71f2be356563c16a23272686189fcfb45d0b82) | 判断哪些机制已随正式版交付 |

`main` 的 live ref 与 gitlink 由本轮 `git ls-remote` 和 `git ls-tree` 核对。本文是只读源码与官方规范研究，没有运行 Readest APK，也没有在 PCT-AL10 上做恢复计时或视觉验收；因此不把源码存在的逻辑等同于真机性能结论。

## 1. 恢复位置的完整调用链

### 1.1 应用只在 `view.init()` 返回后标记 view initialized

**[M]** `FoliateViewer` 创建 `foliate-view`，调用 `view.open(bookDoc)`，安装 transform、样式和 renderer 属性，然后选择保存位置或 `0`：有 `lastLocation` 时 `await view.init({ lastLocation })`，否则 `await view.goToFraction(0)`，最后才 `setViewInited(bookKey, true)`。[`FoliateViewer.tsx` L662-L803](https://github.com/readest/readest/blob/6d5a89ceeedcaec9422e91002c7e22af9cbedf68/apps/readest-app/src/app/reader/components/FoliateViewer.tsx#L662-L803)

但外层直接调用 `openBook()`，没有 `.catch()` 或恢复 fallback。[`FoliateViewer.tsx` L819-L823](https://github.com/readest/readest/blob/6d5a89ceeedcaec9422e91002c7e22af9cbedf68/apps/readest-app/src/app/reader/components/FoliateViewer.tsx#L819-L823) 因此任何未被底层吞掉的拒绝都可能使 `setViewInited` 永远不执行。Readest 不能作为“所有失败都会降级 ready”的依据。

### 1.2 CFI 先解析 section，再延迟解析 DOM Range

**[M]** EPUB CFI 先在 OPF 中找 `itemref/idref`，映射到 spine index；anchor 本身保存为 `doc => CFI.toRange(doc, parts)`，等目标文档真正加载后才执行。[`epub.js` L833-L847](https://github.com/readest/foliate-js/blob/f65836f77e8b66b84baacd54bfc92096578e7a84/epub.js#L833-L847)

`View.init()` 对已解析的位置等待 renderer `goTo`，否则根据设置跳正文起点或第一个 linear section。[`view.js` L325-L336](https://github.com/readest/foliate-js/blob/f65836f77e8b66b84baacd54bfc92096578e7a84/view.js#L325-L336) `resolveNavigation()` 自身会捕获解析错误并返回 `undefined`，但 `init()` 没有包住 renderer 调用；一般 `goTo()` 才有自己的 catch。[`view.js` L503-L526](https://github.com/readest/foliate-js/blob/f65836f77e8b66b84baacd54bfc92096578e7a84/view.js#L503-L526)

这套模型值得 Atha 保留两个分离的值：

- section 级粗定位必须在内容加载前可得；
- DOM Range 只能在净化、样式注入与目标文档建立后解析；
- Range 失败应回退到同一 section 的比例、元素或 section 起点，而不是把整本书判坏。

### 1.3 当前分支补了“无 rect 也必须落页”

**[M]** 绝对定位的全页图片可能让 CFI Range 没有任何 client rect。当前 paginator 不再直接返回，而是落到主 section 起点；页数测成 0 时也做相同处理，从而初始化 `scrollBounds`，保证下一次 swipe 不被静默丢弃。[`paginator.js` L3076-L3137](https://github.com/readest/foliate-js/blob/f65836f77e8b66b84baacd54bfc92096578e7a84/paginator.js#L3076-L3137)

该修复来自发布后的 [`f6bce4ce81d7cc6cd5df156a9867e3f0daa0427c`](https://github.com/readest/foliate-js/commit/f6bce4ce81d7cc6cd5df156a9867e3f0daa0427c)，对应“全页封面上所有 swipe 都失效”的真实问题。Atha 的恢复与手势测试必须专门加入 `position:absolute` 图片、空 Range 和零分页测量，不能只测普通段落。

## 2. EPUB 图片究竟在哪一步等待

### 2.1 ZIP 资源先解出，图片 decode 不在这一层

**[M]** foliate Loader 加载 XHTML 时递归处理本地引用：`link[href]`、所有 `[src]`、`poster`、`object[data]`、XLink、`srcset`、内联 CSS 和 `@import` 都会先解析，所引用的图片、字体或其他资源被解成 Blob 并创建 Object URL，再生成目标 XHTML URL。[`epub.js` L851-L938](https://github.com/readest/foliate-js/blob/f65836f77e8b66b84baacd54bfc92096578e7a84/epub.js#L851-L938)、[`L982-L1069`](https://github.com/readest/foliate-js/blob/f65836f77e8b66b84baacd54bfc92096578e7a84/epub.js#L982-L1069)

这意味着目标 section 进入 iframe 前，引用资源的 **ZIP 读取与 URL 改写** 已经在关键路径上；但 Blob 已可访问不等于像素已经 decode。应用的 zip.js 接线直接调用 `entry.getData(new TextWriter/BlobWriter)`，没有传 `AbortSignal` 或 deadline。[`document.ts` L224-L282](https://github.com/readest/readest/blob/6d5a89ceeedcaec9422e91002c7e22af9cbedf68/apps/readest-app/src/libs/document.ts#L224-L282)

**[S]** zip.js 官方 `EntryGetDataCheckPasswordOptions` 明确提供 `signal?: AbortSignal`，用于取消解压；`useWebWorkers` 和 `useCompressionStream` 的默认值也都是 `true`。[zip.js API](https://gildas-lormeau.github.io/zip.js/api/interfaces/EntryGetDataCheckPasswordOptions.html) Readest 却在全局配置中关闭 Web Worker 和 CompressionStream。[`zip.ts` L1-L10](https://github.com/readest/readest/blob/6d5a89ceeedcaec9422e91002c7e22af9cbedf68/apps/readest-app/src/utils/zip.ts#L1-L10) 这是它针对自身 Tauri/WebView 兼容边界的选择，不应在 Atha 未做同机 A/B 前照搬。

### 2.2 普通 `<img>` 没有显式 `decode()` 门

**[M]** iframe 的 `load` 回调先执行应用的 `afterLoad`，再读取方向与背景、设定列布局、限制 `img/svg/video` 最大尺寸、渲染并观察 `body`。普通图片没有 `await img.decode()`；字体则只用 `doc.fonts.ready.then(() => expand())` 做迟到扩展。[`paginator.js` L632-L767](https://github.com/readest/foliate-js/blob/f65836f77e8b66b84baacd54bfc92096578e7a84/paginator.js#L632-L767)、[`L895-L1054`](https://github.com/readest/foliate-js/blob/f65836f77e8b66b84baacd54bfc92096578e7a84/paginator.js#L895-L1054)

Readest 应用层的 `applyImageStyle()` 也只做两阶段 computed-style 读取与属性写入，把百分比尺寸改为像素并识别行内图片；它不检查 `complete`、`naturalWidth` 或 decode 结果。[`style.ts` L1205-L1266](https://github.com/readest/readest/blob/6d5a89ceeedcaec9422e91002c7e22af9cbedf68/apps/readest-app/src/utils/style.ts#L1205-L1266)

**[S]** HTML Standard 规定 `HTMLImageElement.decode()` 返回在解码完成时 fulfill、不可解码时以 `EncodingError` reject 的 Promise；它还指出先 decode 再插入可避免首次绘制时的同步解码掉帧。[HTML Standard: `decode()`](https://html.spec.whatwg.org/multipage/embedded-content.html#dom-img-decode-dev) 同一标准说明 `loading=lazy` 图片不延迟 window `load`，并建议提供 width/height 或 aspect ratio 防止迟到图片造成布局漂移。[HTML Standard: lazy loading](https://html.spec.whatwg.org/multipage/embedded-content.html#attr-img-loading)

因此“已经收到 iframe `load`”不能作为“所有可见图片均已 decode”的严格证明，尤其对原书携带的 lazy 图片。反过来，统一 `Promise.all(img.decode())` 也会让损坏图片、超大动画图和实现异常成为整页 ready 门，Readest 并没有选择这条路。

### 2.3 `background-image` 是有边界的唯一特例

**[R]** foliate 对 `body` 的背景图另建 `Image()`，在首次 render 前等待 `load` 或 `error`，并以 `Promise.race` 设置 3000ms 上限。成功时记录自然尺寸；失败时按无背景继续；超时后若图片迟到，滚动模式再扩展。[v0.11.20 `paginator.js` L620-L690](https://github.com/readest/foliate-js/blob/dd71f2be356563c16a23272686189fcfb45d0b82/paginator.js#L620-L690)

这不是通用图片策略，而是一个很窄的恢复补丁：滚动模式先加载目标 section 之前的全页插图，若它在恢复定位后才变高，目标章节整体被推走。对应浏览器测试构造了延迟 250ms、立即 error 和永不返回三种 `Image`，分别断言位置不漂移、错误仍显示、挂起最终解除。[`paginator-scrolled-restore.browser.test.ts` L1-L107](https://github.com/readest/readest/blob/6d5a89ceeedcaec9422e91002c7e22af9cbedf68/apps/readest-app/src/__tests__/document/paginator-scrolled-restore.browser.test.ts#L1-L107)、[`L142-L242`](https://github.com/readest/readest/blob/6d5a89ceeedcaec9422e91002c7e22af9cbedf68/apps/readest-app/src/__tests__/document/paginator-scrolled-restore.browser.test.ts#L142-L242)

**[I]** Atha 可借鉴“只等会改变 anchor 前方几何的资源”，但不应照抄每张图 3 秒：

- 使用一次导航的全局 deadline，而不是每个资源各自产生完整上限；
- 只等待首屏内、缺少稳定占位尺寸且会改变目标 offset 的资源；
- `load`、`error`、timeout、abort 都必须减少同一个 pending 计数；
- deadline 到达后发 degraded-ready，保留迟到后的重新锚定，不继续遮住正文；
- 普通尺寸稳定或在 anchor 之后的图片不进入 ready 门。

## 3. `stabilized` 到底保证什么

**[M]** paginator 展示新 section 时先设 `opacity=0`，等待 section URL、iframe load 与初排版，解析 anchor 并滚到目标，再设 `opacity=1` 和发第一次 `stabilized`。随后 `#fillVisibleArea()` 非阻塞地补相邻 section；填充结束后再发一次事件。[`paginator.js` L3299-L3377](https://github.com/readest/foliate-js/blob/f65836f77e8b66b84baacd54bfc92096578e7a84/paginator.js#L3299-L3377)、[`L3452-L3505`](https://github.com/readest/foliate-js/blob/f65836f77e8b66b84baacd54bfc92096578e7a84/paginator.js#L3452-L3505)

官方浏览器测试也特意要求监听器必须在 `goTo` 前安装，因为第一次事件在 `goTo` 返回前同步发出；另一个测试证明 fill 可以在第一次事件后继续增加 view 数量。[`paginator-stabilization.browser.test.ts` L20-L36](https://github.com/readest/readest/blob/6d5a89ceeedcaec9422e91002c7e22af9cbedf68/apps/readest-app/src/__tests__/document/paginator-stabilization.browser.test.ts#L20-L36)、[`L123-L183`](https://github.com/readest/readest/blob/6d5a89ceeedcaec9422e91002c7e22af9cbedf68/apps/readest-app/src/__tests__/document/paginator-stabilization.browser.test.ts#L123-L183)、[`L281-L307`](https://github.com/readest/readest/blob/6d5a89ceeedcaec9422e91002c7e22af9cbedf68/apps/readest-app/src/__tests__/document/paginator-stabilization.browser.test.ts#L281-L307)

Readest 应用一收到 `stabilized` 就 `setLoading(false)`，然后才异步做 warichu 重排和 Word Lens gloss 刷新。[`FoliateViewer.tsx` L519-L543](https://github.com/readest/readest/blob/6d5a89ceeedcaec9422e91002c7e22af9cbedf68/apps/readest-app/src/app/reader/components/FoliateViewer.tsx#L519-L543) 所以它的产品语义接近“目标页可读”，不是“页面永不再变”。

**[I]** Atha 应把状态拆开，不再让一个 `ready` 同时承担四种含义：

| 状态 | 最低保证 | 是否允许继续后台工作 |
| --- | --- | --- |
| `document-ready` | 主 section 可解析且安全 DOM 已建立 | 是 |
| `positioned` | locator 已解析或已按清晰顺序回退，目标已落位 | 是 |
| `visual-ready` | 主内容已可见；会改变首屏/anchor 的关键资源已完成或超时 | 是 |
| `settled` | 当前 generation 的资源计数为 0，或均已有终态 | 否 |
| `degraded-ready` | 正文可读，但记录了超时、资源失败或定位回退 | 是 |

恢复 UI 应在 `visual-ready` 或 `degraded-ready` 解除遮罩；benchmark 再单独观测 `settled`。这样坏图片不会永久挡住正文，性能数据也不会把“快速空白成功”误算成优秀。

## 4. 损坏资源与慢资源的实际容错

### 已有容错

- **[M] 无效 XHTML 回退。** XHTML 出现 parser error 或缺 namespace 时，Readest 记录 warning 后按 HTML 重解析，而不是拒绝整本书。[`epub.js` L1002-L1011](https://github.com/readest/foliate-js/blob/f65836f77e8b66b84baacd54bfc92096578e7a84/epub.js#L1002-L1011)
- **[M] manifest 漏项容忍。** 引用路径未出现在 manifest 时，只要 ZIP 中存在且扩展名是已知图片，就临时构造图片 item；字体也有独立 fallback。其他未知路径保留原 href，让文档继续。[`epub.js` L943-L980](https://github.com/readest/foliate-js/blob/f65836f77e8b66b84baacd54bfc92096578e7a84/epub.js#L943-L980)
- **[M] 大小写容忍。** Readest ZIP loader 建立大小写不敏感索引，但若两个 entry 只差大小写则不猜测，避免错误选中。[`document.ts` L253-L275](https://github.com/readest/readest/blob/6d5a89ceeedcaec9422e91002c7e22af9cbedf68/apps/readest-app/src/libs/document.ts#L253-L275)
- **[M] 相邻 section 失败不阻塞。** preload 的 load、content 和 iframe 任一步抛错都会被捕获，只放弃该相邻 section；当前主 section 继续显示。[`paginator.js` L3378-L3450](https://github.com/readest/foliate-js/blob/f65836f77e8b66b84baacd54bfc92096578e7a84/paginator.js#L3378-L3450)
- **[M] 不可用 iframe 守卫。** 当前分支在文档没有 `documentElement/body` 时销毁 view、恢复 opacity 并发出 `stabilized`。[`paginator.js` L3321-L3329](https://github.com/readest/foliate-js/blob/f65836f77e8b66b84baacd54bfc92096578e7a84/paginator.js#L3321-L3329) 该行为来自 v0.11.20 之后的 [`f94b2512f4fea532fb485abbb65512fa4788a835`](https://github.com/readest/foliate-js/commit/f94b2512f4fea532fb485abbb65512fa4788a835)。

### 仍然存在的失败面

- **[M] 没有通用超时。** `entry.getData()`、递归 `loadHref()`、`section.load()`、`loadContent()` 和 iframe `load` 都可能无限等待；只有 body background 有 3 秒 cap。
- **[M] 主 section 失败语义不完整。** `section.load()` 的 rejection 被转为 `{}` 后送入 `#display()`。[`paginator.js` L3614-L3629](https://github.com/readest/foliate-js/blob/f65836f77e8b66b84baacd54bfc92096578e7a84/paginator.js#L3614-L3629) 对数字 anchor 可能继续显示已有或空 view；对 CFI 函数 anchor 仍可能在读取缺失 `primaryView.document` 时抛错。不能把它描述为可靠降级。
- **[M] 应用只做粗粒度错误显示。** `DocumentLoader.open()` 对 ZIP 的特殊错误改写为“unsupported or corrupted”，其他异常直接重抛；没有按 spine/resource 标记局部可用性。[`document.ts` L385-L472](https://github.com/readest/readest/blob/6d5a89ceeedcaec9422e91002c7e22af9cbedf68/apps/readest-app/src/libs/document.ts#L385-L472)
- **[M] 迟到回调仍有竞态。** 当前 stabilization 测试还专门压制销毁后 queued iframe load 触发的 `getComputedStyle` 错误，并将其标为已知 cleanup race。[`paginator-stabilization.browser.test.ts` L65-L79](https://github.com/readest/readest/blob/6d5a89ceeedcaec9422e91002c7e22af9cbedf68/apps/readest-app/src/__tests__/document/paginator-stabilization.browser.test.ts#L65-L79)

**[I]** Atha 的实现应比 Readest 多一层明确契约：每次 open/restore 产生 generation id 和 AbortController；解压、sanitize、资源加载、decode gate、公式排版和定位任务都检查 generation；达到全局 deadline 后 abort 能取消的任务，不能取消的回调因 generation 不匹配而静默丢弃。降级记录只保留计数、阶段、资源类型、耗时和错误类，不记录书名、文件路径、原文或资源 URL。

## 5. 图片、表格和公式章节如何进入阅读布局

### 图片

**[M]** paginator 在每次 columnize/scrolled layout 中为 `img/svg/video` 设置 `max-width/max-height`、`object-fit: contain` 和 `break-inside: avoid`；作者给出的更小 max 尺寸会保留。[`paginator.js` L895-L930](https://github.com/readest/foliate-js/blob/f65836f77e8b66b84baacd54bfc92096578e7a84/paginator.js#L895-L930) Duokan 类全页图片在分页模式用 absolute pinning；滚动模式则清掉残留定位，让图片回到正常 flow。[`paginator.js` L931-L980](https://github.com/readest/foliate-js/blob/f65836f77e8b66b84baacd54bfc92096578e7a84/paginator.js#L931-L980)

固定布局 EPUB 的 viewport 优先级是 SVG `viewBox`、meta viewport、出版物 viewport、首张图片 `naturalWidth/naturalHeight`，最后用 1000×2000 兜底。[`fixed-layout.js` L8-L31](https://github.com/readest/foliate-js/blob/f65836f77e8b66b84baacd54bfc92096578e7a84/fixed-layout.js#L8-L31) 它依赖 iframe load 后的自然尺寸，也没有额外 decode deadline。

当前固定布局滚动模式还有成熟的资源预算：最多保留 12 页、并发加载 3 页，按离可见区域最近优先；generation 防止过时完成回写，失败页进入 terminal `error` 而不是紧循环重试。[`fixed-layout.js` L232-L247](https://github.com/readest/foliate-js/blob/f65836f77e8b66b84baacd54bfc92096578e7a84/fixed-layout.js#L232-L247)、[`L1004-L1102`](https://github.com/readest/foliate-js/blob/f65836f77e8b66b84baacd54bfc92096578e7a84/fixed-layout.js#L1004-L1102) 这套预算适合吸收为 Atha 图片页/固定布局候选，但不能直接套到普通 reflowable EPUB 的整章 DOM。

### 表格和 MathML

**[M]** Readest 只给所有 `table` 和 display MathML 包一层 `.scroll-wrapper`；行内 MathML 留在文字流中。包装层带 `cfi-skip`，所以书签和标注的 CFI 不因纯布局节点改变。[`scrollable.ts` L133-L179](https://github.com/readest/readest/blob/6d5a89ceeedcaec9422e91002c7e22af9cbedf68/apps/readest-app/src/utils/scrollable.ts#L133-L179)

wrapper 初建时 iframe 还不可见，宽度为 0，所以 Readest 用一次性 `ResizeObserver` 等第一次有效布局，判断是否真有 overflow 后立即 disconnect，避免持久 observer 在翻页时造成 relayerize storm。[`scrollable.ts` L211-L235](https://github.com/readest/readest/blob/6d5a89ceeedcaec9422e91002c7e22af9cbedf68/apps/readest-app/src/utils/scrollable.ts#L211-L235)

对过高且不可分栏的 `inline-block/flex/grid/table`，foliate 只在根节点真的发生纵向 overflow 时扫描，将对应 display 降级为可分页的 block/flex/grid/table。这是稀有错误路径，不把每章都全量扫描。[`paginator.js` L869-L894](https://github.com/readest/foliate-js/blob/f65836f77e8b66b84baacd54bfc92096578e7a84/paginator.js#L869-L894)

**不应照抄的部分：** `shouldTableScrollConsumeTouch/Wheel()` 只看 wrapper 是否存在该轴 overflow，不看当前 `scrollLeft/scrollTop` 和手势方向的剩余空间。因此到了右、左、上、下边界仍然消费，测试也把这个行为固定为产品目标。[`scrollable.ts` L43-L70](https://github.com/readest/readest/blob/6d5a89ceeedcaec9422e91002c7e22af9cbedf68/apps/readest-app/src/utils/scrollable.ts#L43-L70)、[`scrollable.test.ts` L161-L267](https://github.com/readest/readest/blob/6d5a89ceeedcaec9422e91002c7e22af9cbedf68/apps/readest-app/src/__tests__/utils/scrollable.test.ts#L161-L267) Atha 应继续使用“起手方向仍有剩余滚动空间才把本序列交给 overflow owner；边界上的新手势交给 page owner”的更强仲裁。

## 6. Atha 应借鉴与拒绝的清单

| Readest 机制 | Atha 决策 | 理由 |
| --- | --- | --- |
| CFI 先定 section，加载后再求 Range | 借鉴 | 允许坏 section 局部降级，不把 book open 与精确定位绑死 |
| 无 rect、零页数回退 section 起点 | 直接加入测试并借鉴 | 同时解决恢复和全页图片滑动失效 |
| XHTML 失败退 HTML、manifest 漏图片按 ZIP entry 补 | 借鉴兼容策略 | 符合“其他阅读器能开就尽量开”的目标 |
| 首个 stabilized 先让正文可见，邻章后台填充 | 借鉴状态拆分 | ready 不应等全书资源 |
| body 背景图有界等待 | 借鉴原则，不复制 3000ms 常量 | 仅等待会改变定位的资源，使用全局 deadline |
| 普通图片只靠 iframe load + ResizeObserver | 部分借鉴 | 不全量 decode，但需补 lazy/迟到图片的可观测 pending 与锚定 |
| zip.js 不传 AbortSignal | 拒绝 | 成熟库已经提供取消能力 |
| 全局关闭 workers/CompressionStream | 先 A/B | 平台兼容 workaround 不能当普适性能优化 |
| main section load 失败后 `{}` 继续 | 拒绝 | 需要显式 degraded state、fallback locator 与不变式 |
| 一次性 ResizeObserver 判 table/math fit | 借鉴 | 避免翻页期间重复 relayout |
| table/math 在边界仍永久吞手势 | 拒绝 | 与 Atha 明确交互验收冲突 |
| 固定布局 12 页缓存、3 并发、近者优先、generation | 作为独立模块候选 | 适合图片页，不应污染 reflowable 热路径 |

## 7. 最小测试与 benchmark 矩阵

### 7.1 合成 fixture

至少加入以下小型、可公开的合成 EPUB，不使用用户私有书内容：

| 场景 | 注入方式 | 必须断言 |
| --- | --- | --- |
| 普通有 width/height 图片 | 本地 PNG/JPEG | `visual-ready` 有界；定位误差不超过一行或约定 px |
| 无固有占位的迟到图片 | 延迟 Blob/资源响应 | 先降级可见或有界等待；迟到后 anchor 不漂移 |
| `loading=lazy` 首屏/首屏外图片 | 两张不同位置图片 | iframe load 不被误当 settled；首屏外图不阻塞 |
| decode error 图片 | 截断图片字节 | 失败计数终态；正文仍可翻页、选择和查词 |
| 永不 settle 的图片 gate | 测试 double | 全局 deadline 后 degraded-ready，不永久 loading |
| 上一 section 的 body background | 延迟、error、hang 三组 | 恢复目标不漂移；失败和挂起不阻塞 |
| SVG 公式图片 | SVG、`object`、XLink | 引用改写正确；坏子资源只降级局部 |
| MathML 行内/块级 | inline 与 display 各一 | inline 不包装；display 包装且 locator 不变 |
| 宽表内嵌图片 | 横向 overflow | 表内可滚；边界新手势能翻页；点击不误开 |
| absolute 全页封面 | Range 无 rect | 落到 section 起点；左右 swipe 均有效 |
| 无效 XHTML | parsererror 后可按 HTML 解析 | 章节可读，并记录 compatibility fallback |
| manifest 漏图但 ZIP 有 entry | 图片不列 manifest | 图片仍能显示或明确降级，不拒绝整书 |
| 主 section load 抛错 | loader double | 跳过/占位/下一可读 section 三者选定一项且不挂起 |
| 相邻 section load 抛错 | preload double | 当前目标正常；失败 section 不重试风暴 |
| 旧 generation 迟到 | 连续快速开两章 | 第一章回调不能改写第二章布局或状态 |

### 7.2 指标和停止条件

每个场景至少记录：

- `open -> document-ready`、`open -> positioned`、`open -> visual-ready`、`open -> settled`；
- ZIP inflate、sanitize、资源替换、图片 gate、公式/表格布局、anchor 解析各阶段耗时；
- `resource_pending_peak`、`resource_failed_count`、`resource_timeout_count`、`locator_fallback_count`；
- ready 前后的 Layout/Style/Long Task、峰值 JS heap 与图片解码内存；
- 恢复后首屏锚点像素偏差，以及迟到资源完成后的最大漂移；
- ready 后首个点击翻页、首个短 flick、表格边界翻页是否成功。

建议先设不依赖具体设备的正确性门：所有 failure/hang 场景都在测试 deadline 内进入 `visual-ready` 或 `degraded-ready`；current generation pending 最终为 0；无未处理 rejection；恢复误差不随迟到资源累积。性能阈值再以 Linux GUI 和 PCT-AL10 各自的 warm/cold 中位数与 p95 建基线，不能把 Linux WebKitGTK 数字冒充 Android System WebView。

## 8. 落地顺序

1. 先定义 `document-ready/positioned/visual-ready/settled/degraded-ready` 事件与 generation/abort 接口，不改变现有 UI。
2. 给 zip.js `getData()` 和 section pipeline 贯通 `AbortSignal`，加一次导航的总 deadline；失败转换为结构化局部结果，不再靠永不返回的 Promise 表示状态。
3. 保存 section + locator + fallback fraction；Range 无效、无 rect 或零页时按固定顺序降级，并始终初始化可导航边界。
4. 给真正会改变目标 offset 的可见图片建立有界 gate；其他图片保留 ResizeObserver 迟到重锚定，不做全量 `decode()`。
5. 保留 table/display MathML 的 `cfi-skip` wrapper 与一次性 fit measurement，同时继续采用 Atha 的方向和边界 owner。
6. 跑合成 fixture 的错误矩阵，再跑 Linux GUI timing/trace，最后由用户在 PCT-AL10 上做真实阅读恢复和滑动验收。

## 未决问题

1. Android System WebView 对 Blob URL 上 `loading=lazy`、`decode()` 和 iframe load 的具体时序，需要在 PCT-AL10 当前 WebView 版本上实测，不能只按标准推断。
2. Atha 当前 EPUB pipeline 是否已经在 sanitize 阶段移除 `loading=lazy` 或补尺寸，需要实现前以有效运行产物核对；本报告没有修改该策略。
3. 全局 deadline 的数值应由用户已导入书籍的冷开与 warm restore 分布决定，而不是直接采用 Readest 的 3000ms。
4. 损坏的主 spine item 应显示占位并允许手动跳过，还是自动跳到下一 linear section，属于产品契约，需要在实现 change 中明确；无论哪种都不能永久 loading。

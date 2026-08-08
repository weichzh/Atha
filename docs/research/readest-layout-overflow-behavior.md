---
description: 对照固定 Readest 与 foliate-js 源码，判断 Atha 的 layout-cut 应作为阅读阻断还是诊断信号。
---

# Readest 分页溢出与 Atha `layout-cut` 研究

## 结论先行

**不，Readest 不是把任一正文、图片或表格矩形越过单页边界当作整本书失败。** 它的固定 `foliate-js` 分页器会先用列布局呈现，并针对已知原因做局部恢复：长词换行、图片/视频/SVG 的最大可用尺寸与等比缩放，以及仅在确实发生纵向列溢出时，把过高的不可分割 inline-* 容器降级为可分页 display。对于不适合分页的内容，Readest 提供 scrolled flow：取消列约束、让宿主容器 `overflow: auto`，继续阅读而不终止会话。

Atha 修复前的 `layout-cut` 同时承担了两件不该混在一起的事：它是很好的**回归探针**，但在正常打开、字号调整、窗口重排和延迟资源完成后都会向普通用户抛错，成为**阅读阻断**。这比 Readest 严格得多，也会把书源的可恢复版式问题误报成“本书无法阅读”。

建议保留“已知安全内容绝不能静默裁掉”的不变量，但把 `countCutRects()` 从全局断言改为诊断/测试指标；先采取已有、可解释的渲染修复。只有真实样本证明局部修复仍不够时，才降级到该 section 的滚动阅读并保留 Locator。不能恢复的加载、资源信任、Locator 或持续布局不稳定才应阻断并明确报错。

## 范围与固定证据

本报告先只读检查 Atha 修复前基线 `f32487a` 与仓库已有的 Readest clone，随后单列修复后的目标端结果。

| 对象 | 固定锚点 | 证明方式 |
| --- | --- | --- |
| Readest v0.11.20 release | [`1df1505fc5033fc949463c9908f2d53bd0fbdfa6`](https://github.com/readest/readest/tree/1df1505fc5033fc949463c9908f2d53bd0fbdfa6) | 本机已有 clone 可解析的 `v0.11.20` tag（提交主题 `release: version 0.11.20`） |
| v0.11.20 的 foliate-js gitlink | [`dd71f2be356563c16a23272686189fcfb45d0b82`](https://github.com/readest/foliate-js/tree/dd71f2be356563c16a23272686189fcfb45d0b82) | release tree 的 `packages/foliate-js` gitlink；本机 submodule 未初始化，因此源码从官方永久锚点读取 |
| 本机 Readest `main` clone | [`629ab2919a5812156af6152015ddfd0c34c6843b`](https://github.com/readest/readest/tree/629ab2919a5812156af6152015ddfd0c34c6843b)，foliate [`f65836f77e8b66b84baacd54bfc92096578e7a84`](https://github.com/readest/foliate-js/tree/f65836f77e8b66b84baacd54bfc92096578e7a84) | `.tmp/readest-insets-main` 的 `HEAD`；其包版本仍写 `0.11.20`，但不是 release tag。另一个本机 main clone 是 `2acb9fad0b578e590eec19b47f790b66461ac38f`，gitlink `df623dbe6610fd98a7c2d5d7a5c23bfcfc7d19f3` |
| Atha 比较对象 | 修复前基线 `f32487a` 的 `reader/web/pagination.mjs` 与 `reader/web/diagnostics.mjs` | `git show` 静态代码阅读 |

除非另有标记，“Readest”以下指 v0.11.20 release + 它的固定 foliate-js；本机两个 `main` snapshot 保留用于核对策略没有反向变化，不能外推为已发布 APK 的实际行为。结论是静态证据，不是 Android 真机验收。

## Readest / foliate 的实际策略

### 1. 分页是列流，不是逐页矩形断言

`View.render()` 依据 `flow` 选择 `columnize()` 或 `scrolled()`；分页路径为文档根设置 `column-width`、`column-fill: auto`、固定页面轴尺寸与 `overflow: hidden`，并强制 `overflow-wrap: break-word`。随后 `expand()` 从内容范围计算所需 column/page 数，扩大 iframe/视图长度，使溢出的**列**成为后续可导航页面，而不是把它们当错误。[`paginator.js` L704-L709](https://github.com/readest/foliate-js/blob/dd71f2be356563c16a23272686189fcfb45d0b82/paginator.js#L704-L709)、[`L746-L802`](https://github.com/readest/foliate-js/blob/dd71f2be356563c16a23272686189fcfb45d0b82/paginator.js#L746-L802)、[`L922-L948`](https://github.com/readest/foliate-js/blob/dd71f2be356563c16a23272686189fcfb45d0b82/paginator.js#L922-L948)。

它的源码没有等价于 Atha `countCutRects() === 0` 的“扫描全书/可见页 rectangle 后 throw”断言；这里的 `overflow: hidden` 是列分页的实现细节，后续列通过尺寸展开和 page navigation 抵达，不是宣称所有 DOM 盒子必须在单个页面高度内。

### 2. 超大图片：clamp 后继续，而非失败

每次布局均遍历 `img, svg, video`，清掉旧的 inline max 值、尊重更小的作者 CSS 上限、否则把最大宽/高约束到可用 page 区，使用 `object-fit: contain`、`box-sizing: border-box` 与 `break-inside: avoid`。Duokan 全页封面是明确例外，仅在分页中绝对定位并等比铺满；滚动模式不会套这个固定高度策略。[`paginator.js` L830-L914](https://github.com/readest/foliate-js/blob/dd71f2be356563c16a23272686189fcfb45d0b82/paginator.js#L830-L914)。

这说明“图片不被无提示裁掉”是它维护的品质目标，但路径是约束/重排，非把一次测量失败升级为整本错误。

### 3. 过高的 inline-block / inline-flex / inline-grid / inline-table：有条件 reflow

foliate 明确注释了问题：高的 atomic inline-level box 无法跨列分片，会让第一页后的列被裁，甚至令整个章节看似消失。它只在根的 `scrollHeight > clientHeight + 1` 时扫描；若元素高度超过可用页高，就把 display 从 `inline-block`、`inline-flex`、`inline-grid`、`inline-table` 分别改为 `block`、`flex`、`grid`、`table`，让内容继续分页。没有找到可恢复元素也不会在这处抛出异常。[`paginator.js` L804-L829](https://github.com/readest/foliate-js/blob/dd71f2be356563c16a23272686189fcfb45d0b82/paginator.js#L804-L829)。

这覆盖了“表格被错误写成 inline-table”及同类容器，不等于通用表格重写；但 Readest 的 Markdown 适配器针对自己生成的 `pre` 使用 `white-space: pre-wrap` 和 `overflow-wrap: break-word`，并给图片 `max-width: 100%`，以避免长代码/图片破坏分页。[`md.ts` L30-L39](https://github.com/readest/readest/blob/1df1505fc5033fc949463c9908f2d53bd0fbdfa6/apps/readest-app/src/utils/md.ts#L30-L39)。对 EPUB 的宽表和 display math，Readest 另以 scroll-wrapper 包裹：合适宽度时 `overflow: visible`，过宽时 `overflow: auto`，让它横向可滚动而非裁掉。[`scrollable.ts` L151-L179](https://github.com/readest/readest/blob/1df1505fc5033fc949463c9908f2d53bd0fbdfa6/apps/readest-app/src/utils/scrollable.ts#L151-L179)、[`style.ts` L402-L427](https://github.com/readest/readest/blob/1df1505fc5033fc949463c9908f2d53bd0fbdfa6/apps/readest-app/src/utils/style.ts#L402-L427)。

### 4. 作者分页 CSS 会被转换，重排会恢复锚点

打开书籍时，Readest 将 `page-break-*` 改为 column-break，把 `break-*-page` 改成 column；这避免 EPUB 作者样式与 CSS columns 的语义错位。[`paginator.js` L1543-L1556](https://github.com/readest/foliate-js/blob/dd71f2be356563c16a23272686189fcfb45d0b82/paginator.js#L1543-L1556)。resize/字体变化重渲染所有已载入 view，随后同步回滚至当前 anchor 并派发 `stabilized`，而不是因一次中间尺度变化终止阅读。[`paginator.js` L1857-L1874](https://github.com/readest/foliate-js/blob/dd71f2be356563c16a23272686189fcfb45d0b82/paginator.js#L1857-L1874)。

### 5. 不能可靠分页时，切为滚动并连续加载

scrolled flow 会取消 columns 和固定高宽，把内容保留在自然文档尺寸；外层 container 使用 `overflow: auto`。`View.render()` 正是以 `flow === 'scrolled'` 为分支条件。[`paginator.js` L704-L742](https://github.com/readest/foliate-js/blob/dd71f2be356563c16a23272686189fcfb45d0b82/paginator.js#L704-L742)、[`L1297-L1314`](https://github.com/readest/foliate-js/blob/dd71f2be356563c16a23272686189fcfb45d0b82/paginator.js#L1297-L1314)。它在滚动时预加载相邻 section，最多八个且目标为前方五屏，并保持浏览器 scroll anchoring；边界/加载失败只影响相邻段（记录 warning），不会把已有 section 的阅读会话全局失败。[`paginator.js` L3098-L3134](https://github.com/readest/foliate-js/blob/dd71f2be356563c16a23272686189fcfb45d0b82/paginator.js#L3098-L3134)、[`L3333-L3370`](https://github.com/readest/foliate-js/blob/dd71f2be356563c16a23272686189fcfb45d0b82/paginator.js#L3333-L3370)。

分页模式也不是全书一次性成功才可读：short section 会装入前一个/下一个 section 补齐 spread；翻到已载入 section 直接复用，连续滚动时避免空白闪屏。[`paginator.js` L3449-L3478](https://github.com/readest/foliate-js/blob/dd71f2be356563c16a23272686189fcfb45d0b82/paginator.js#L3449-L3478)。

## Atha 修复前严格点及其后果

`countCutRects()` 暂时移除书的 transform，用 text Range client rect 加上 `img, table, figure` 的整体 rect，与当前 page 顶/底比较，容差仅 0.75 CSS px；可选 `visibleOnly` 仍含当前及后一列。[`pagination.mjs` L233-L274](../../reader/web/pagination.mjs#L233-L274)。因此它测到的是“在 column/page-height 模型里跨出页面轴的片段”，不能区分：

- 应被图片尺寸约束或 atomic-display 降级修复的真实裁切；
- 可分片文本的正常 column fragment/字体加载瞬态；
- 分页不适合的长表、代码块或定高作者布局，本可继续滚动阅读的内容。

在修复前基线 `f32487a` 中，它被当作硬断言用于首开、字体调节、viewport resize 与延迟资源后的翻页；异常最终进入 `fail()`，普通用户会得到“正文或图片越过页面边界”的 error 页面。诊断又在偏好回归、基准的全布局扫描和最终 `ready` 记录中复用该值。

这解释了修复前模拟器上“layout-cut 很严格”的感受：它并不是一个已证明的安全边界，而是将布局质量指标接到了用户可见失败链路。低 density/异常 viewport 会放大字号与物理页面几何，因而更容易触发；但即使在真实密度下，未修复的书源 `inline-table`、大图或代码块仍会走相同阻断路径。

## 应保留与应降级的边界

| 保留为阻断性 invariant | 改为诊断或可恢复路径 |
| --- | --- |
| 不可信书籍的脚本、网络、路径与未知资源边界；资源加载失败不能伪装成完整内容。 | `countCutRects()` 的数量、元素类别、section、viewport、字号：诊断/fixture gate 指标，不直接让普通阅读失败。 |
| Locator 必须能落在可见内容；重排后无法恢复锚点应显式报错或回到已知安全位置。 | 书源作者把可分页内容设成过高 `inline-*`：只在实际溢出时最小 display 降级后重排。 |
| 真正持续不稳定的 layout（设定的有限稳定窗口后仍改变）应可见地失败，避免保存错误位置。 | 超大 `img/svg/video`：在本列可用面积内等比 clamp；记录是否被约束。 |
| 明确声称的 paginated mode 不得**静默**裁掉已知可见文本/媒体。 | 表格、`pre/code`、不可分割长内容未能保持分页：允许该 section/会话进入 scroll，而非中止整本书；保留进入/退出时的 Locator。 |

最小实现应复用现有分页器，而不是复制 Readest：把普通运行时 `assert(countCutRects(...) === 0)` 移出错误链，保留诊断 API；在 Atha 样式层给普通图片加可用尺寸约束与长词换行，并把表格 / 代码放入受控页内滚动容器。现有 diagnostics / verify-sample / benchmark 继续保留零 cut 门；普通 Android gate 记录 ready 的 cut 数作为非阻断诊断。只有真实样本证明这些局部恢复仍不够时，才增加 inline-* display 降级或 section 级 scrolled-flow。

## 验证限制

固定 Readest 源码与 Atha 调用链的比较属于静态证据，不声称已验证 Readest 的实际 APK 行为。Atha 的后续模拟器结果单列如下；ARM 真机仍未覆盖。

## Atha 后续实测

随后在 Linux 的 API 36、x86_64、16 KiB AVD 上对当前 README 做了不读取正文的几何探针。360×640 CSS viewport 中，page 范围为 88–568 px；下一栏一个 `H2` 的元素盒从 90 px 开始，仍安全位于 page 内，但浏览器 Range 矩形从 85.25 px 开始，导致 `countCutRects(true)` 仅报告这一处 cut。它证明本次阻断来自文字测量矩形外扩，而不是内容盒实际被裁。

Atha 因此把普通运行路径的四处 cut 硬断言移出错误链，给普通图片加等比限幅，把表格 / 代码放入受控页内滚动容器，并保留 diagnostics、Locator、安全和持续不稳定布局门。相同候选随后在 720×1280、320 dpi、4 GiB 的固定 AVD 上通过系统 picker、首稳 / ready、目录首 / 中 / 末、全书搜索、翻页、强停恢复、健康和双日志隐私检查。该结果验证本次最小修复，不代表已实现 Readest 的完整 inline-* 降级或 scrolled-flow，也不替代 ARM 真机性能证据。

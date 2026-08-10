---
description: Chromium 114 多列长内容合成边界、Atha 对照与最小验证顺序。
---

# Chromium 114 多列长内容合成

## 后续状态

本文记录实施前假设。后续 PCT trace 已证明长章整层 transform 是热点，当前实现因此采用 20,000px 原生 `scrollLeft` 回退；下文“不复制该阈值”只代表研究阶段的停止条件。当前事实以 `docs/changes/reader-gesture-performance.md` 和 `docs/codebase/MAP.md` 为准。

## 证据边界

- Chromium 事实固定到提交
  [`bdabf09e`](https://github.com/chromium/chromium/tree/bdabf09e0e69fd4ba0be83dc8269b2ebf99a9c94)，
  即 Chromium 114 的上游分支源码。PCT-AL10 的实际 WebView provider 可能包含厂商补丁，仍需先用
  `dumpsys webviewupdate` 记录运行版本。
- Readest 对照固定到 v0.11.20 的应用提交
  [`1df1505f`](https://github.com/readest/readest/tree/1df1505fc5033fc949463c9908f2d53bd0fbdfa6)
  及其 foliate-js 提交
  [`dd71f2be`](https://github.com/readest/foliate-js/tree/dd71f2be356563c16a23272686189fcfb45d0b82)。
- 下文只说明源码路径和由这些路径支持的推断，不把静态源码阅读称为 PCT-AL10 性能验收，也不把逻辑
  `.book` 等同于一个确定的 compositor layer 或 GPU 纹理。

## 固定源码事实

### 长层与 tiles

Chromium 114 的 `PictureLayer` 使用稀疏 tiling。`TileManager` 依据可见区、即将可见区和 eventually
区域安排 raster；`PictureLayerTiling` 更新 live rect，删除范围外 tile，并只为新进入范围的部分创建
tile。长多列内容因此不等于把整章一次性 raster 成一张纹理。

- [`how_cc_works.md` 的 PictureLayer 与 tiling](https://github.com/chromium/chromium/blob/bdabf09e0e69fd4ba0be83dc8269b2ebf99a9c94/docs/how_cc_works.md#L119-L128)
- [`TileManager` 的优先级与 raster 调度](https://github.com/chromium/chromium/blob/bdabf09e0e69fd4ba0be83dc8269b2ebf99a9c94/docs/how_cc_works.md#L231-L253)
- [`PictureLayerTiling` 的 live rect、删除与创建](https://github.com/chromium/chromium/blob/bdabf09e0e69fd4ba0be83dc8269b2ebf99a9c94/cc/tiles/picture_layer_tiling.cc#L603-L704)

由此支持的风险判断是：长 `.book` 可能增加首次 layer promotion、可见及近邻 tile raster、翻页时的
tile churn、checkerboard、图片 decode 和内存淘汰压力；高 DPR 会放大每个可见 tile 的设备像素成本。
实际 layer 数量、bounds、tile 数量和内存仍必须从目标运行时 trace 取得。

### transform 与 `will-change`

- Chromium 114 将 `will-change: transform` 计为直接 compositing reason，但最终 layer 的拆分、合并和
  bounds 仍由合成决策决定。
  [固定源码](https://github.com/chromium/chromium/blob/bdabf09e0e69fd4ba0be83dc8269b2ebf99a9c94/third_party/blink/renderer/core/paint/compositing/compositing_reason_finder.cc#L70-L96)
- 直接 compositing reason 会阻止部分纯 2D translation 被 decomposite；因此长期保留 `will-change`
  也可能固化本可合并的层。
  [固定源码](https://github.com/chromium/chromium/blob/bdabf09e0e69fd4ba0be83dc8269b2ebf99a9c94/third_party/blink/renderer/platform/graphics/compositing/pending_layer.cc#L350-L359)
- Blink 主线程的 rAF 或 style 变更通过 BeginMainFrame 和 commit 同步到 compositor。手动逐帧写
  `transform` 不等同于已经建立的 compositor CSS animation；settle transition 建立后才可能在后续帧
  独立运行。
  [固定源码说明](https://github.com/chromium/chromium/blob/bdabf09e0e69fd4ba0be83dc8269b2ebf99a9c94/docs/how_cc_works.md#L64-L85)

### `scrollLeft` 与线程边界

`scrollLeft` 不是天然更快。Chromium 114 的 JS setter 会先调用 `UpdateStyleAndLayoutForNode`，再计算
滚动位置和 snap，并进入 programmatic scroll。若滚动节点可直接更新 compositor scroll offset，则不必
重走完整 paint；否则仍需 paint-property 更新与失效。这里的 style/layout 更新在干净树上不等于每帧完整
relayout，但 setter 本身仍由主线程 JS 发起。

- [`Element::setScrollLeft`](https://github.com/chromium/chromium/blob/bdabf09e0e69fd4ba0be83dc8269b2ebf99a9c94/third_party/blink/renderer/core/dom/element.cc#L1566-L1615)
- [`PaintLayerScrollableArea::UpdateScrollOffset`](https://github.com/chromium/chromium/blob/bdabf09e0e69fd4ba0be83dc8269b2ebf99a9c94/third_party/blink/renderer/core/paint/paint_layer_scrollable_area.cc#L424-L480)
- [同步 JS 输入转交 Blink 主线程](https://github.com/chromium/chromium/blob/bdabf09e0e69fd4ba0be83dc8269b2ebf99a9c94/docs/how_cc_works.md#L51-L60)

真正无阻塞的浏览器原生滚动可以在 compositor thread 继续。Readest 默认分页手势却使用 non-passive
`touchmove`、`preventDefault()` 和 JS `scrollBy()`，所以输入仍经过主线程，只是位移落在浏览器原生
scroll property tree 上。

- [Readest 默认 touchmove 路径](https://github.com/readest/foliate-js/blob/dd71f2be356563c16a23272686189fcfb45d0b82/paginator.js#L2177-L2231)
- [Readest rAF 滚动回退](https://github.com/readest/foliate-js/blob/dd71f2be356563c16a23272686189fcfb45d0b82/paginator.js#L2690-L2707)

Readest 的 `20,000 CSS px` 是该实现中针对 oversized view 的经验阈值。源码注释记录了高 DPR 下巨大
transformed section 的 Blink 卡顿和 GPU texture 风险，但这不是 Chromium API 保证，也不是 Atha 可直接
复制的切换阈值。

- [阈值与源码注释](https://github.com/readest/foliate-js/blob/dd71f2be356563c16a23272686189fcfb45d0b82/paginator.js#L255-L285)

## Atha 对照

- 分页 `.book` 的主框只有一页宽高，通过 CSS columns 形成横向 overflow；当前页和拖动预览都写整个
  `.book` 的 `translateX()`。见 `reader/atha-reader.css` 和 `reader/web/pagination.mjs`。
- Atha 在拖动开始后的同一路径才添加 `data-swipe-dragging`，由 CSS 应用
  `will-change: transform`；首次 pointermove 仍可能同时支付 promotion 或 raster。拖动结束后删除该
  hint，避免永久占层，是合理的现有边界。
- Atha 的 non-passive `pointermove` 在主线程判定 owner、调用 `preventDefault()`，再由 rAF 合并 transform
  写入。它和 Readest 的 JS `scrollBy()` 都不是零主线程手势，差异主要在最终使用 transform property tree
  还是 scroll property tree。
- `.reader` 同时具有 `filter: brightness(...)` 和 `transform: scale(...)`。它们可能改变合成层组织，不能
  只根据 `.book` 的 CSS 推算最终 layer 数量。

延迟公式图片也属于长多列布局的测量边界：

- `content.mjs` 依据 `<img>` 的 `width`、`height` 属性识别公式，移除 `src`，并以
  `visibility:hidden` 保留盒子；稍后重新设置 `src`、等待 `decode()`，再解除隐藏。
- `pagination.mjs` 的 `applyFormulaScale()` 在分页计算前依据属性写入 inline pixel `width` 和 `height`。
  这通常会覆盖普通来源 CSS 的 `width:auto; height:auto`，降低资源到达后改变 used size 的机会。
- 但不能据此声称绝对无 reflow。Chromium 114 只有在 computed logical width 和 height 都 fixed、min/max
  条件满足且图片不是 flex item 时，intrinsic-size change 才只做 paint invalidation；其余情况明确标记
  layout 和 full paint invalidation。
  [`LayoutImage` 固定源码](https://github.com/chromium/chromium/blob/bdabf09e0e69fd4ba0be83dc8269b2ebf99a9c94/third_party/blink/renderer/core/layout/layout_image.cc#L166-L208)
- `visibility:hidden` 仍参与 layout，`decode()` 也不提供布局事务保证。`!important` 尺寸规则、flex item、
  其他 min/max 约束或未保持 fixed computed size 时，设置 `src` 后的 intrinsic-size change 仍可使 multicol
  fragmentation 重新计算。

## 最小 A/B

按成本和信息增益只做三组：

1. **短章与长章，不改代码**：固定当前页内容、手势距离和轮次，只改变同类内容在前后累计的列数。记录
   LayerTree 的 layer 数量及 bounds、首次视觉反馈、RasterTask、tile create/delete、checkerboard 和
   SurfaceFlinger 有效呈现间隔，先确认是否存在随总长度增长的拐点。
2. **瞬时 `will-change` 与无 hint**：在同一长章用运行时样式覆盖对比现状和无 hint；只有首帧 promotion
   明确占主导时，才补一轮“提前一个稳定帧预热”。不先形成产品代码。
3. **transform 与 overflow scroll PoC**：保持内容、pointer JS、rAF cadence、拖动距离和 settle 时长相同，
   只替换位移输出。比较主线程 style/layout、commit、tile churn、呈现卡顿和分页语义，不预设任一路径胜出，
   也不使用 Readest 的 `20,000 CSS px` 作为切换点。

这些结果仍只是 PCT-AL10 指定 WebView provider、指定 build 和指定 fixture 的真实目标证据，不能外推其他
WebView、设备、DPR 或私有书籍。

## 明确拒绝

- 不永久设置 `translateZ(0)`、`will-change: transform` 或其他强制 layer promotion。
- 不在整本 `.book` 上设置 `contain: paint` 或 `content-visibility:auto`；前者可能裁剪列 overflow，后者对
  始终与视口相交的整本内容不能形成可靠虚拟化，并可能改变 fragmentation 边界。
- 不复制 Readest 的 `20,000 CSS px` 阈值，也不在 trace 前整体改成 `scrollLeft`。
- 不以整层宽高乘四估算真实 GPU 内存，不把逻辑 `.book` 称为一张 GPU 纹理。
- 不引入 canvas 截图、固定三页虚拟化或双分页架构；这些都超过本轮验证问题。

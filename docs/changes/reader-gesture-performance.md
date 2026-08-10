---
description: 用可信基准修复翻页仲裁，并压缩导入、首次打开与重内容滑动热路径。
---

# 阅读手势与翻页性能

## Status

implemented

## Problem

分页模式把图片、公式、表格和代码整体视为受保护目标，导致这些内容占据页面时点按与横向拖动不能翻页。拖动热路径还在每个 `pointermove` 同步读取样式和几何；松手后又在 170ms 收束动画中重复扫描整章 Locator，并为同尺寸公式的成功解码执行完整重排，重内容章节因此比普通书更慢。

现有 Linux 门只用不可信的合成 `PointerEvent` 确认进入 dragging 状态，没有覆盖真实命中、逐帧跟手、媒体误开、表格边界移交或动画稳定。Readest 0.11.20 真机与固定源码证明其图片拖动优先于点击，但横向表格即使到边界仍永久截获手势；Atha 借用前者，不复制后者。

首个优化候选在 PCT-AL10 恢复用户已导入的公式重书时又出现 `image-load` 终止错误；同一压力章节在 Linux WebKitGTK 连续恢复通过，公开合成样本也没有复现。该错误发生在新增的可见公式提前停止分支之后，必须先以不含书籍身份或内容的批次计数锁定 Android WebView 114 时序，并保证可恢复的迟到或损坏图片不再阻塞正文阅读，再继续调整动画。

PCT-AL10 随后暴露章节首屏向前翻会落入前一节尾部连续空白页。跨节导航虽然请求了 `pages - 1`，但分页总数只取整章 `scrollWidth`；书源尾部强制分页或空盒可扩张滚动宽度，却不代表存在可读内容。跨节回退必须以最后真实内容列为边界，不能让用户逐页跳过尾部空列。

最终候选的真机短划还暴露了 WebView 114 的输入分叉：首个横向 Pointer move 后平台发送 `pointercancel`，但同一根手指的 Touch move / end 继续到达。只监听 Pointer 终止会让余下位移失去 owner，或让页面停在拖动 transform；直接在 down 时捕获到外层 reader 又会改写媒体、链接、表格和选区的 click / pointerup target。分页手势因此必须在该分叉后延续同一 owner，并以 touchend 的最终坐标完成一次导航。

书架导入原本同步完成完整解析、资源提取和 ReaderManifest 发布，用户在文件选择后只能等待；跨 section 前又先清空当前 DOM，再获取、校验并排版目标 section，因此章节边界会显得整页刷新。导入应先耐久登记源文件，首次打开再通过既有 importer 准备并显示进度，后续打开直接复用产物；章节切换应保留当前内容直到目标准备完成，并以小型会话缓存减少往返读取。

阅读页启动时还会先把第 0 节的书内封面绘制到屏幕，再恢复上次 Locator，形成一次错误内容闪现。启动阶段必须用不透明阅读底色保持单一加载状态，等位置、排版和交互全部稳定后再揭示正文；失败时则立即让出加载层并显示错误。

当前真机候选又暴露了加载阶段证据缺口：用户看到翻入页的图片从无到有，并指出此前 benchmark 已否定把 Opera 式空白后揭示直接当作性能方案。既有 5 + 20 手势门在公式全部完成后才测量，只证明稳定资源后的翻页，不证明逐资源空白占位优于当前 / 相邻页预解码。最终策略必须先用可控慢资源做同场 A/B，不能用准备耗时、静态尺寸提示或预热后的手势数字代替。

## Scope

- 分页模式使用一次手势一个 owner 的最小仲裁：多点、选区、表单、链接、弹窗与纵向意图保持受保护，图片、公式、表格和代码允许横向翻页；
- 左右区域点按优先翻页，图片与表格中心点按打开查看器，其他正文中心点按切换工具栏；横向拖动提交后抑制兼容 `click` 与 `dblclick`，避免误开预览；
- 内嵌表格压缩为整页宽预览并裁掉超长部分，不再拥有横向手势；代码等仍可横向溢出的容器在起手方向有空间时滚动自身，到边界后由下一次手势翻页；
- 用 `requestAnimationFrame` 合并拖动更新，缓存分页步长和显示比例，短章节逐帧只写 transform，超长章节逐帧只写原生 `scrollLeft`；
- 固定尺寸公式成功校验、解码和显现不触发重排；只有失败替换导致布局变化时才捕获 Locator、重排并恢复；
- `image-load` 失败前只记录 pass、成功 / 失败、剩余数量与 generation 等固定计数；不记录书名、路径、正文、URL、资源名、哈希或异常文本；
- 对迟到或损坏的本地图片优先降级显示正文，不因当前页仍有可恢复资源而终止整本书；安全边界、generation 守卫与失败占位保持不变；
- EPUB v5 首次准备只为同时缺少显式 `width` 与 `height` 的本地图片，从图片头写入经过方向与尺寸上限校验的原生 HTML 宽高与 Atha 标记，不因整节作者样式或无关行内样式跳过；阅读器在书源 CSS 之前为最多 512 个唯一尺寸对生成零特异性的 `contain-intrinsic-size` 规则，常见未分层书源与用户 CSS 继续覆盖。额外尺寸对退回 HTML 宽高与 `height:auto`，不运行逐图 CSS 匹配。整页 / 整节等待和全图预载继续禁止；普通图片在稳定盒内异步绘制，只有延迟公式使用同尺寸局部空白；
- 缓存同一稳定页面的内容偏移，避免控制、进度和书签在一次翻页后重复全章扫描；
- 导入只复制并校验受支持源文件、写入兼容书架记录；首次打开在 blocking worker 中调用既有 importer，壳层显示原生不定进度条，成功产物供后续打开直接复用；
- 阅读会话保留当前 DOM 直到目标 section 完成校验，以共享三槽和 8 Mi 字符预算复用已准备 section 或相邻原始 XHTML；失败时保留上一稳定内容，关闭书籍时以 generation 和 Promise identity 拒绝在途回写并释放缓存；
- 阅读页从挂载起以无内容信息的不透明底色和点状动画覆盖书内封面，只有恢复上次 Locator、完成排版并绑定交互后才揭示正文；失败路径同时撤下加载层；
- `atha-book` 资源请求在读锁内直接读取当前 `BookRoot`，不再为每个 CSS、图片或公式请求深拷贝整份书根索引；
- 分页总数以真实内容范围为上限，章节首屏向前一次直接到上一节最后真实内容页；
- 本切片实际运行的文档、Linux GUI、公式压力与 PCT 构建 / 校验 / 安装入口使用 Bash，不再经 PowerShell 启动；
- PCT 实时入口每 500ms 输出可见内容 layer 的 SurfaceFlinger 呈现更新 cadence；自动短划结束后再读取一次 gfxinfo app frame duration，并汇总呈现 cadence、P95、最大间隔和慢帧。静止页只标记 idle，不记作 0 FPS；
- 在现有 Linux Tauri / WebKitGTK 门中增加 W3C Pointer Actions 可信输入、rAF 时序与表格边界矩阵，不增加 WebDriver、手势或动画依赖；录屏只做感知复核，Performance API 是数值门槛；
- 继续只提供左右分页和纵向滚动两种阅读方式；CSS 社区继续只保留模块包接口。

## Non-Goals

- 不引入 Readest 的截图覆盖层、Canvas / WebGL 卷页、全书解析预热或整套 Foliate 架构；
- 不允许书内脚本、网络资源或新的路径权限；
- 不在本切片调整字号、设置页视觉、词典、书架、CSS 模块数据或阅读统计 schema；
- 不因合成压力样本改写 DPR / brightness 模型；只有 Linux WebKitGTK A/B 证明它是主瓶颈时另立 change。
- 不在没有真实 trace 前引入完整 ready 状态机、全书图片预载、虚拟化、图片 worker 或通用资源调度器。
- 不在本切片机械删除仍供旧 Windows 流程追溯的 `.ps1`；后续入口在实际使用前逐项迁移并验证。

## Acceptance Criteria

- [x] 图片、公式和普通表格上的左右区域点按及明确横向拖动均恰好翻一页，且不误开查看器；图片与表格中心点按打开查看器，代码中心点按仍切换工具栏，代码双击和键盘预览不变；
- [x] 内嵌宽表固定在页面宽度并省略过长单元格，超高表格由页面裁切；全屏投影保留表格结构、公式、图片、完整文本和原生双向滚动，50%–400% 缩放、复位、关闭与焦点返回可用；
- [x] 多点、选区、链接、表单、弹窗和纵向意图不被翻页劫持；纵向滚动模式仍使用原生滚动；
- [x] 拖动帧没有 geometry / layout read，每次输入序列只缓存一次几何；成功公式显现不重排，失败替换仍恢复同一 Locator；
- [x] 最终源码的 Linux Tauri / WebKitGTK 门请求 W3C touch Actions，事件均为 `isTrusted`，并分别记录请求与实际 `pointerType`；每类 5 次预热、20 次测量均单步正确；
- [x] 最终源码的横拖首次可观察 transform / scroll 页面状态更新 P95 不超过 33.4ms、拖动期间主线程连续 rAF 间隔 P95 不超过 25ms、最大间隔不超过 50ms；分页松手使用 300ms 收束动画且到稳定 P95 不超过 400ms，点按松手到首次可观察页面状态更新 P95 不超过 50ms；
- [x] 用户已导入且曾在 PCT-AL10 报 `image-load` 的 EPUB 可恢复到正文；单个迟到、损坏或缺失图片最多降级自身，不再把整本书切到错误页；
- [x] 失败计数事件通过严格 ASCII / 长度 / 范围 parser，Android AppLog 中先于通用错误出现，且隐私检查证明没有内容字段；
- [x] 同机 Readest 既有十轮 presentation 基线与最终 Atha 公共书、公式书自动前后滑分开保存；比较只使用 SurfaceFlinger 实际呈现，不把 Linux rAF 或 ADB 输入冒充自然手指录屏；
- [x] 章节首屏向前点按或横划一次落在上一节最后真实内容页，书源尾部强制分页产生的空列不计入页数；
- [x] 当前 required docs / workflow、Linux GUI、公式压力和 PCT 候选链路均有已实跑的 Bash 入口，正式验证不调用 `.ps1`；
- [x] PCT-AL10 WebView 114 在 Pointer 序列被 `pointercancel` 后由同一 Touch 序列继续跟手并在 touchend 恰好翻一页；无后续 Touch 的取消在 250ms 内复位，迟到 pointerup 不取消下一根指针；
- [x] `scripts/check-pct-reader-fps.sh` 可实时监视或自动执行单次前后滑，区分静止、app render 与 SurfaceFlinger presentation，并过滤 pending fence 与 128 槽截断风险；
- [x] 加入书架只耐久登记源文件；首次打开显示进度并准备书籍，删除耐久源后仍可从已发布缓存二次打开；旧书架记录继续可读；
- [x] section 切换在目标准备前不关闭当前内容，三槽缓存命中不再获取或解析 XHTML，相邻 section 只做有界原文预取；准备或排版失败保留上一稳定内容，书籍关闭后全部释放；
- [x] 阅读页启动加载层先于书内内容存在，并在上次 Locator 与交互恢复后才淡出；加载失败不会被遮住，动画尊重 `prefers-reduced-motion`；
- [x] 5.4 MiB 本地 EPUB 的 v5 release 五轮中位数为登记 11ms、首次准备 232ms、完整性校验后的缓存打开 27.213ms；结果不包含书籍身份、路径或哈希；
- [x] 真实 5.4 MiB 扫描型 EPUB 的 3047 张无尺寸 JPEG 全部获得原生稳定尺寸；EXIF 旋转、异常尺寸、作者 CSS、损坏 EXIF、缺失扫描尾的 JPEG、IDAT 后 PNG `eXIf` 及无耐久源的旧 v2 / v3 / v4 缓存均有回归测试，单个异常只跳过增强，不拒绝整本书；
- [x] 同场慢资源诊断确认固定几何与正文的首次可观察页面状态更新为 14–15ms，成功资源的页数和文字 anchor 均不变化；普通图片或整页 / 整节等待范围内资源的控制组额外增加 99 / 246 / 754ms，永不终态约 1s，因此拒绝这种等待揭示闸门，临时诊断代码已删除；
- [x] 最终源码重建并通过包结构与签名校验后，以 Bash PackageInstaller session 不清数据更新 PCT-AL10；自动启动、前后滑、页面落稳和 SurfaceFlinger presentation 均通过，不再等待用户手动复核。

## Architecture Impact

present

- Design purpose: 把内容激活与翻页从静态 target 黑名单改为按区域、方向和溢出边界仲裁，并让翻页热路径不随整章复杂度重复做无关工作。
- Drivers / quality scenarios: `A-CTRL-02` 要求媒体覆盖页面时仍可完成单页导航；`A-PERF-02` 要求重图片 / 公式 / 表格章节的跟手帧不做布局读，释放后使用 300ms 收束动画并在 400ms 内稳定。
- Modules / interfaces: `LocalLibrary` 拥有耐久源登记与首次准备，格式 importer 继续拥有精确内容身份；Svelte 书架只根据 `prepared` 显示原生进度；`session` 与 `content` 拥有三槽 section 会话缓存；`interaction` 拥有 owner、区域和 click 抑制；`pagination` 拥有 rAF 写入、长章节原生横向滚动、步长与 Locator 缓存；diagnostics 与 runner 只暴露无内容的测试时序。
- Candidate and tradeoffs: 复用 Pointer Events、W3C Actions、浏览器滚动、CSS transition 和现有 Navigation 队列，不引入手势库；Readest 的 6 / 8px 方向认领、拖动优先和 300ms rAF 收束语义可借鉴，但其表格永久截获与截图动画管线被拒绝。
- Evidence / review trigger: 合成 DOM red / green、Linux Tauri 可信输入与 rAF 基准、PCT-AL10 自动滑动与 SurfaceFlinger presentation，以及独立 review；自动化真机输入不冒充自然手指触摸。

## Files And Steps

1. 保留已通过的 owner、边界、rAF、缓存与 Linux 可信输入门，不重写现有手势架构；
2. 先用 Readest 源码、公开合成 EPUB、真实 Linux 压力章节和 PCT 脱敏计数区分图片恢复假设；只修被证据命中的最小共享分支；
3. 在 Linux 与 PCT 证明曾失败 EPUB 可读、失败图片局部降级且性能没有明显回退后，再处理稀疏短划动画；
4. 以 PCT-AL10 自动前后滑与 SurfaceFlinger presentation 校准收束结果；自然手指主观手感不作为本次关闭阻塞项。
5. 复用现有内容身份和 importer，把书架导入拆成耐久登记、首次准备与缓存打开，并在 section 边界只增加三槽会话缓存和相邻原文预取。

## Checks

- reader module 语法、现有 Node 测试、Svelte check / build 与 workspace Rust 检查；
- `bash scripts/check-reader-linux.sh` 的完整导入诊断、跨章边界、可信指针矩阵、帧基准与日志隐私；
- `bash scripts/check-reader-formula-performance.sh --epub <path>` 的公式 / 页数下限与 5 + 20 逐场景指针指标；
- 公开 W3C EPUB 的 Linux 恢复、PCT 系统录屏和 SurfaceFlinger / 帧差基线；
- terminal `image-load` telemetry parser、Android AppLog 顺序与日志隐私；
- PCT-AL10 上自动启动、前后滑、页面落稳、SurfaceFlinger presentation 与内容无关日志检查；
- `seeds_private_formula_gui_benchmark` 的匿名 release 五轮登记 / 首次打开 / 缓存打开耗时；
- AutoCorrect、文档 gate、`git diff --check` 与独立 review。

## Rollback

恢复旧 target 保护、同步 transform、同步 importer 与 section 关闭顺序即可；新增书架字段可选且旧记录可读，不迁移偏好、消息、词典、CSS 模块或统计数据。

## Approval

用户已明确要求在 EPUB 兼容完成后研究 Readest 的源码、控制与动画，用录屏和更好的 benchmark 修复图片、公式和表格上的点击 / 滑动翻页失效及重内容卡顿，并要求日常验证使用 Linux GUI、最终可使用 PCT-AL10 真机。

## Result

`Interaction` 现在按一次序列一个 owner 仲裁分页、横向溢出和内容激活。图片、公式、表格与代码不再整体截断页区点按和横拖；内嵌表格不再接管横向手势。多点、选区、链接、表单、弹窗和纵向意图保持受保护，已提交横拖同时抑制兼容 `click` 与 `dblclick`。

表格在正文中以整页宽、紧凑字号和单元格省略号显示，超出单页高度的部分直接裁掉；中心单击会把已经过书籍安全清洗的表格 DOM 克隆到 Readest 式暗色全屏查看层，保留行列结构、公式图片、普通图片和替代文本，同时再次移除链接目标、事件属性、行内样式、焦点属性和非公式 class。尚未进入正文加载批次的公式在投影中保持空白，由复用安全校验的三个并发队列按表格顺序渐进填充；关闭查看层会取消余下队列，并阻止在途结果回写。图片复用同一查看层，二者均提供关闭、放大、缩小、复位、50%–400% 缩放值和放大后的原生滚动；焦点和阅读位置在关闭后恢复。实现只使用既有 dialog、Lucide 图标、CSS transform 与浏览器滚动，没有增加依赖。

CSSOM 验证继续拒绝无有效规则的乱码，但只含合法注释的样式现在按空样式接受；这补齐了既有 Unicode 迁移诊断与实际验证器之间的缺口，不放宽子资源、Shadow DOM 穿透或大小限制。

分页在起手时缓存视口、DPR 换算、页步长与稳定页 Locator 偏移；move 热路径只更新内存并由单个 rAF 写 transform 或 `scrollLeft`，松手使用 300ms 收束动画。显示宽度超过 20,000px 的长章节使用浏览器原生横向滚动，短章节继续使用 transform；新手势从中断收束后的实际 `scrollLeft` 起步，连续划动不回跳到逻辑整页坐标。fragment 文本偏移只在当前 section 和当前排版首次解析，重新排版时失效。

`content.loadVisible()` 显式返回 `loaded` 与 `layoutChanged`；普通正文图片不参与首屏 ready 闸门，也没有新增整节等待或全图预载。EPUB v5 首次准备复用 `imagesize` 读取图像头，并用 `kamadak-exif` 处理方向 5–8；单边 8192、总计 2000 万像素上限不变。同时缺少显式宽高的本地图片获得原生 `width` / `height` 和 Atha 标记，即使 section 存在作者 stylesheet 或图片存在无关行内样式也能在解码前稳定宽高比。阅读器在书源 CSS 之前为最多 512 个唯一尺寸对生成零特异性的 `contain:size` / `contain-intrinsic-size` 规则，常见未分层作者或用户规则可继续覆盖；超出上限的尺寸对退回原生属性和 `height:auto`。旧 `data-atha-intrinsic-*` 路径只兼容无耐久源的 v4 缓存。几何盒与正文立即存在，图片像素随后异步绘制；慢资源对照只否定普通图片或整页 / 整节等待全部资源的揭示闸门。失败图片优先沿用连接状态下的非零实际盒，随后退回合法原生、固有或 HTML 属性尺寸；合法零像素盒和已经脱离 DOM 且仅靠书源 CSS 定形的失败图片不能保证完全相同。显式延迟公式先完成 SVG 获取、解析和安全校验，再设置可呈现 `src`；设置 `src` 后只等待 50ms 终态窗口，超时保持同尺寸空白并由迟到 `load` / `error` 局部收尾。该窗口不约束前置校验 wall time。分页收束期间向左额外覆盖一页，保证回划目标页进入同一加载批次。损坏但可读出 SOF 宽高的无 EXIF JPEG 即使缺失后续扫描标记也保留提示；损坏 EXIF 和 IDAT 后的 PNG `eXIf` 只跳过增强。

消息快照的当前 presentation 参数与阅读偏好保持一致：字号接受整数 16–40，默认 19；紧凑、标准和舒适行高分别使用 1.55、1.8 和 2.05。旧 32px 快照兼容分支不变。

分页总数不再直接信任整章 `scrollWidth`，而是从书根到最后一个有意义文本或媒体节点计算内容范围。因此书源内部有意留白仍保留，尾部强制分页产生的空盒不再变成可到达的假页；章节首屏向前一次直接进入上一节最后真实内容列。

PCT-AL10 的 WebView 114 会把空 `pointerType` 的横滑在第一个小位移后转成 `pointercancel`，随后只继续派发 Touch Events。Interaction 现在保留已认领 owner，以非 passive touchmove 继续同一预览，并用 touchend 最终坐标提交；真正的 touchcancel 仍立即复位，没有后续 Touch 的异常取消最多保留 250ms。分页模式声明 `touch-action: none`，滚动模式仍保留原生 `pan-y`。

Linux runner 增加 13 场景 W3C Actions 矩阵和普通 / 公式压力章节的逐场景 P95。长章节实际滚动发生在 closed ShadowRoot 外层 `#page`；诊断器不再从书内 DOM 使用恒为 `null` 的 `closest('.page')`，而是直接采样真实 scroller 的 `scrollLeft`，避免把已移动页面误报为零视觉帧。它只在显式诊断查询下安装匿名目标，不给产品增加手势库、动画依赖或第二阅读模型。私密样本身份和章节只来自忽略 sidecar，输出与 AppLog 不包含路径、标题、作者、正文或哈希。

书架现在把受支持源文件登记到 `SourceBooks` 并写入带可选 `sourcePath` 的 schema 1 记录，不在选择文件时解析整本书。首次打开通过 Tauri blocking worker 调用原 importer，书架以原生不定进度条反馈；发布后的 `ImportedBooks` 先按精确格式 marker 和元数据淘汰错误格式，再核对 manifest 与全部声明文件，缺失资源或空 section 从耐久源重建。同进程 importer 共用一个准备锁，避免并发首次打开争用 staging；再次登记会复用健康源，以验证后的 staging 原子覆盖身份异常源，并重建损坏记录。EPUB v2 至 v5 完整缓存都保持可读；存在耐久源时尝试把 v2 至 v4 升级到 v5，升级失败回退原缓存，无耐久源的旧记录直接继续打开。动态 importer 元数据继续补齐标题、作者与封面，既有 eager `import` 和内容身份保持兼容。

section 切换不再先清空 live DOM。`content` 用同一 LRU 管理完整校验后的 detached body / CSS 与相邻原始 XHTML，总计最多三个 key 且文本预算为 8 Mi 字符；目标准备和排版成功后才替换内容，失败时保留上一稳定 DOM 与位置。关闭书籍会推进 generation、清空缓存，异步 XHTML、CSS 与 SVG 结果只有 generation 和 Promise identity 都仍匹配时才能回写。资源协议也不再逐请求深拷贝 `BookRoot`，图片与公式请求只在共享读锁内完成一次受限读取。

阅读路由现在从 Svelte 首次挂载起显示不透明的书页底色和三点加载动画。运行时仍可在遮罩下完成第 0 节初始化，但只有 `readerState.restore()`、书签与交互绑定全部结束后才设置 `data-reader-ready` 并淡出；错误路径也会结束 busy 状态，因此既不会先露出书内封面，也不会让失败提示被加载层盖住。该实现只复用原生 CSS、`aria-busy` 和现有启动顺序，没有引入新的 ready 状态机或动画依赖。

## Review

独立评审连续检查了 owner、DPR、溢出边界、兼容事件、多点、纵向意图、rAF、Locator 缓存、迟到图片、滚动补载、跨章末页、耐久源替换、格式别名、并发首次准备和 section 缓存生命周期。固有尺寸复核另发现 HTML 尺寸会压过用户 CSS、作者样式章节仍做无效扫描、表格待加载公式无界启动，以及 50ms 超时会让名义并发上限失效；这些问题均已在共享路径修正。最终窄复核确认三个 worker 等待真实 `load / error / abort` 终态，关闭会清空队列、移除在途 `src` 并阻止 detached DOM 回写，没有剩余 P0 / P1。剩余 P2 是：`@layer` 内作者 / 用户规则可能被未分层原生提示压过；失败占位的合法零像素盒、仅靠 CSS 定形且已脱离 DOM 的坏图无法精确复原；公式缩放、无耐久源 v4 提示和失败占位依赖的行内样式会被消息快照清理；超过 512 个唯一尺寸对或触及 1 MiB 快照 CSS 预算时，live 与快照提示集合可能不同。均不扩大当前安全契约，遇到真实样本再升级。

## Evidence And Residual Risks

旧诊断器每个 rAF 强制读取 computed style，只保留视觉值发生变化的帧，还把最后视觉采样到松手的尾段混入帧间隔；它曾让相同源码分别出现 26ms 和 30ms 的场景级 P95，不能作为 compositor 或产品掉帧证据。修正后，拖动冻结分页状态、读取内联 transform / 真实 `scrollLeft`，并以 pointer down 至 pointer up 间全部连续 rAF 的相邻时间差计量；视觉更新数量仍单独验证。该指标是 Linux WebKit 主线程 rAF cadence，不是 SurfaceFlinger presentation 或真实 FPS。

固定最终源码连续两次完成 Linux Tauri / WebKitGTK 0.55.1 正式门。每轮在普通与私有公式压力书上执行 13 个场景、每场景 5 次预热与 20 次测量，共记录 440 次非保护动作；1332 / 1332 张公式均在测量前稳定。两轮最差聚合值分别为横拖首次可观察页面状态更新 32 / 32ms、点按 7 / 7ms、连续 rAF P95 17 / 19ms、最大 rAF 19 / 20ms、松手稳定 352 / 352ms，AppLog 隐私门均通过。

表格公式队列专测固定覆盖 8 个公式、最大并发 3、关闭后 5 个未启动、3 个在途收到 abort、迟到 detached DOM 写入为 0，重开后 8 张均成功且无 pending。该门主动排除资源加载过程，只证明稳定后翻页基线；慢资源 A/B 单独负责揭示策略。

本地 Headless Chromium 用公开 XHTML 将 section 请求固定延迟 3 秒：截图 `artifacts/local/audits/reader-startup-loading-headless.png` 只显示不透明阅读底色与三点动画；MutationObserver 观察到 section 已发布而 `data-reader-ready` 尚未出现时，加载层仍为可见且不透明。最终页面为 `status=pass`、`aria-busy=false`、`data-reader-ready` 与 `aria-hidden=true`。这只证明浏览器 DOM 与视觉覆盖顺序，不替代 Linux Tauri 或 PCT-AL10 的真实打开观感。

跨章门在公开样本落到上一节第 0 / 1 页，在压力样本落到第 99 / 100 页，均与独立内容几何 oracle 的最后一页一致。合成探针额外制造两个尾部空列，分页仍分别保持 1 页和 89 页。滚动资源探针把未加载公式从 1313 降到 1257，目标资源真实完成，加载后和显式重排后的 Locator 均保持可见。最终源码的 Node 13 / 13、消息后端 21 / 21、Svelte check、production build、workspace Rust 119 passed / 10 opt-in ignored 与 Clippy 已通过；定向回归覆盖四路并发首次打开、staging 清理、过期 marker、损坏文本元数据、无源残缺缓存、当前 16–40 快照字号与 EPUB / CBZ 错误后缀。最终源码的可信 Linux GUI 矩阵也已重新通过。

该门请求 touch Actions，事件均为可信，但当前 WebKitGTK 实际报告 `pointerType=mouse`，所以最高证据是 Linux 真实 GUI 与可信自动化指针，不是实体触摸。代码块与表格共用 `table, pre` 仲裁分支，既有结构化检查覆盖其中心操作和预览，但本轮 13 场景矩阵没有复制一组等价的代码块横拖；最终 PCT 自动前后滑与 SurfaceFlinger presentation 补充真实目标证据，但仍不冒充自然手指主观手感。

真机性能取证使用仅含 `arm64-v8a` 且临时开放 WebView 调试入口的诊断 APK，安装证据位于忽略目录 `artifacts/local/audits/pct-reader-install-20260810T062101Z-453056`。前一手势候选恢复 release 调试边界后的 APK SHA-256 为 `11ebb15bf809b5c5e7ffa15b5b1a42b3306e08fa37a9c8f30cec949bc247399d`，安装证据位于 `artifacts/local/audits/pct-reader-install-20260810T065730Z-484783`。

纳入快速登记、首次准备、完整缓存判定、并发准备保护、section 会话缓存与资源协议修复后的最终 APK SHA-256 为 `c121cf4b89ec0fa4415eb651bd3335bb8cbb34d94b70a477b6936b432d7471d4`，已通过包名、仅 `arm64-v8a`、16 KiB ZIP / ELF 对齐与 v2 / v3 签名校验。Bash PackageInstaller session 在不清数据的前提下更新 PCT-AL10，设备端回读判定 `installed=true`，应用随后启动并位于前台；安装证据位于 `artifacts/local/audits/pct-reader-install-20260810T094834Z-640018`。该结果不替代用户对首次打开进度、二次打开与滑动手感的真实触摸验收。

加入启动加载层后的最新 APK SHA-256 为 `95ef460d1285575be88aa2deb96e1b575f380e49cb6951447f60a1b41b4140dc`，再次通过仅 `arm64-v8a`、16 KiB ZIP / ELF 对齐与签名校验，并由 Bash PackageInstaller session 在不清数据的前提下更新 PCT-AL10；安装证据位于 `artifacts/local/audits/pct-reader-install-20260810T101756Z-660421`。冷启动后前台组件为 `com.atha.reader/com.atha.reader.MainActivity`。该证据只证明当前包已安装和可启动；书内封面是否在真实打开过程中完全不可见仍由用户亲自复核。

安装后用公开四 section 样本复核缓存打开，Tauri `operation=open` 在 PCT 上记录 1ms。一次 ADB 合成反向横划从末节准确进入前一节，SurfaceFlinger 观察到 12 次 presentation；两个活跃采样窗的更新 cadence 为 54.1Hz 与 30.0Hz，但完整窗口只有 raw-only 覆盖，不能计算可靠 P95。该结果只算真机目标 smoke，不冒充真实手指、首次准备或最终帧基准。

最终 v5 缓存链路在匿名 5.4 MiB 本地 EPUB 上以 release 模式运行五轮：登记中位数 11ms、首次准备中位数 232ms、逐项完整性校验后的缓存打开中位数 27.213ms。该结果是 Linux 本地后端证据，只证明完整 importer 已从加入书架移到首次打开，且热打开在检查声明文件后仍远低于可感知延迟；Android SAF 复制耗时、进度动画可见性和真机首次 / 二次打开仍需在新 APK 上复核。

加入 v4 私有固有尺寸提示后，同一 5.4 MiB 扫描型 EPUB 在 Linux debug 测试中完成 3047 / 3047 张图片标注，首次准备约 870ms、缓存打开约 30.3ms。该样本包含缺失标准 SOS 标记但仍能从 SOF 读取宽高的 JPEG；它证明无 EXIF 路径不会因非关键扫描尾损坏放弃全部占位，损坏 EXIF 则由独立回归证明只跳过增强。debug 单次时间不能与上面的 release 五轮中位数直接比较。最终公开 Linux GUI 0 + 1 smoke 仍通过 13 场景语义、跨章和日志隐私门；单样本帧间隔不作为正式性能结论。

PCT Chrome CPU profile 把旧热路径锁定为反复几何读取：`getBoundingClientRect()` 自耗时约 43.7–59.5ms，fragment 定位约 20.9ms。稳定图片几何与 fragment 偏移缓存后，前者降至约 11.9ms，后者不再进入热点，单个收束 tick 从约 29ms 降至约 14.2ms。六轮前后交替短划均整页落稳，中位数为 18 次 SurfaceFlinger presentation、38.6 Hz 呈现更新 cadence，中位最大间隔 41.7ms；旧整章 transform 候选只有 3–4 次 presentation、约 9–10 Hz、最大间隔 183–216ms。实时 500ms 监视也能在动作结束后明确转为 `no-new-buffer`，不会把静止页记成 0 FPS。

同机 Readest 既有十轮短划为每轮 17–28 个有效 presentation，多数轮 P95 为 16.7–33.3ms。Atha 的呈现次数已接近该区间，但仍出现 33–67ms 间隔，不能描述为稳定 60 FPS 或完成自然手指验收。浏览器原生 smooth scroll A/B 虽增加 presentation，却把动作延长到约 605–616ms，因过慢且不可控被拒绝；当前保留 300ms rAF 收束。

本地 Chromium production build 在 360 × 780 CSS px 下确认六列表格缩进页面宽度并省略长值，中心单击打开全屏完整表格，横向滚动可查看右侧列；图片查看层在 100% 下完整居中，200% 下放大并可双向滚动。四张原图与 SHA-256 位于忽略目录 `artifacts/local/audits/content-viewer-headless/`。该证据只覆盖本地视觉、真实 click、缩放和滚动，不替代 PCT-AL10 WebView 114 或用户手指验收。

包含该查看层的 arm64 候选 APK SHA-256 为 `4bdc8acd36c58155891ad6a85389270aa9b220b16f97e296ca77065179ac6810`，通过包名、仅 `arm64-v8a`、16 KiB ZIP / ELF 对齐及 v2 / v3 签名验证。Bash PackageInstaller session 在不清数据的前提下更新 PCT-AL10，证据位于 `artifacts/local/audits/pct-reader-install-20260810T110715Z-685466`；随后 `com.atha.reader/.MainActivity` 成为 resumed activity。该证据不替代用户对表格点击、查看层滚动与缩放的真实触摸验收。

表格公式 DOM 投影、EPUB v3 固有尺寸、EXIF 方向与旧 v2 缓存回退合入后的 arm64 中间包 SHA-256 为 `38a7518463ca6379aa8f4e6fe2f93edd4c18e1f3aa97ccd3b17d2387f3a55f7c`。该包曾通过包名、仅 `arm64-v8a`、16 KiB ZIP / ELF 对齐及 v2 / v3 签名校验，并由 Bash PackageInstaller session 在不清数据的前提下更新 PCT-AL10；安装证据位于 `artifacts/local/audits/pct-reader-install-20260810T123924Z-778692`。冷启动耗时 210ms，随后复核前台组件为 `com.atha.reader/com.atha.reader.MainActivity`。独立复核随后以 v4 私有提示、用户 CSS 覆盖和表格公式有界加载替代该实现，因此该包已经过时，不能作为最终源码候选。

完成上述复核修正后的当前 arm64 用户测试 APK SHA-256 为 `a1e9b7e36b2a3d969617555cdea0f69c6af3eae5922fd7aad373bab17ab056ae`，再次通过包名、仅 `arm64-v8a`、16 KiB ZIP / ELF 对齐及 v2 / v3 签名校验。Bash PackageInstaller session 在不清数据的前提下更新 PCT-AL10，安装回读为 `installed=true`，证据位于 `artifacts/local/audits/pct-reader-install-20260810T132548Z-796961`；随后 `com.atha.reader/.MainActivity` 为 resumed activity。该证据只证明当前包已安装且可启动；用户已经否定其资源揭示观感，因此它不是最终候选。

包含最终资源几何、诊断器和消息快照修正的 arm64 APK SHA-256 为 `e3fc7583fb8c48d6d414fe974c4b28ba963886dd39e1d25ce14083c5ab4e2ca3`，证书 SHA-256 为 `9773139815f885602b3180576e6a1515aebbbe411439cf0b30b32245c4e45f58`；包名、仅 `arm64-v8a`、16 KiB ZIP / ELF 对齐及 v2 / v3 签名校验全部通过。Bash PackageInstaller session 在不清数据的前提下更新 PCT-AL10，设备回读 `installed=true`，证据位于 `artifacts/local/audits/pct-reader-install-20260810T165209Z-1136016`；冷启动后 resumed activity 为 `com.atha.reader/.MainActivity`。

最终包在 PCT-AL10 上完成六个 ADB 注入的目标滑动窗口。公共书同节前进 / 后退分别提交 11 / 12 个 SurfaceFlinger presentation，页面按 6 / 30 → 7 / 30 → 6 / 30 落稳；跨节前进 / 后退分别提交 13 / 14 个 presentation，并在前节末页 1 / 1 与后节首屏 1 / 30 之间双向切换，没有空白尾页。用户已导入的公式书可直接打开并完整显示行内公式，前进 / 后退分别提交 15 / 7 个 presentation，页面按 1 / 17 → 2 / 17 → 1 / 17 落稳。每个三秒窗口均在动作后进入 `no-new-buffer`，没有 128 槽截断；对应内容无关证据位于 `artifacts/local/audits/atha-reader-gesture-performance/fps-20260810T165507Z`、`fps-20260810T165558Z`、`fps-20260810T165701Z`、`fps-20260810T165720Z`、`fps-20260810T165824Z` 与 `fps-20260810T165839Z`。release 边界不开放 CDP，因此这些窗口采用 monitor raw-only 聚合，不能给出可靠总体 P95；页面语义由动作后截图逐次确认。该证据是实际目标设备与 SurfaceFlinger 呈现，但输入仍由 ADB 注入，不冒充自然手指主观手感。

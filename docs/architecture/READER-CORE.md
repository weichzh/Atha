# 阅读内核

## 责任

阅读内核负责把导入后的书籍内容以 HTML、CSS 和本地资源的形式呈现。它不以某个书籍封装格式为中心：EPUB、MOBI、AZW 或其他来源将来只是在导入后提供同一类内容文档。

## 兼容性契约

- HTML、CSS、字体、数学公式与 SVG 的呈现以浏览器为质量基准。
- 浏览器基准不意味着执行不受控网页：书内脚本禁用；外部网络资源默认拦截，用户可单次或按书确认加载。
- 无法可靠呈现的内容必须明确报错；不进行不可见的兼容性猜测或书源修复。

## 渲染技术

WebView2 是当前唯一阅读渲染技术。宿主只提供窗口、受控资源、导航拦截与有限遥测，HTML、CSS、布局和绘制继续由浏览器完成；不维护自研或组合式第二引擎。

只有外部引擎的成熟度发生实质变化，并能在 ATHA 困难样本上同时证明浏览器兼容、文本选择与重锚、安全、无裁切和同机性能优势时，才通过新的研究与 change 重新决策。本轮及可预见功能开发只对 WebView2 做常规优化，不提前建设缓存数据库、预热系统或虚拟化框架。

### 应用壳与宿主

产品入口采用 Tauri 2 与 Svelte 5，但仍只有一个 WebView2。Svelte 只拥有顶部栏、底部工具栏、面板和 dialog；现有 reader kernel 继续直接控制 closed Shadow DOM，不把 XHTML、页内节点或分页热状态放进组件树。Vite 构建时按既定顺序拼接现有阅读模块，避免形成第二份内核。

Tauri 复用后端书根、EPUB 导入、共享 CLI、窗口尺寸和诊断逻辑。书籍资源仍走受控 `atha-book` 协议；可信的 Svelte 应用壳只调用固定书架 command，书内文档没有 command 接口，XHTML 和图片不经 IPC 传输。阅读器遥测仍由严格校验、串行发送的独立 command 接收。应用响应使用 `Permissions-Policy` 禁用相机、麦克风、定位、显示捕获等浏览器权限，reader 完成前会从真实文档策略复核关键能力确实不可用。直接 Wry/Tao 的 `atha-reader-host` 在迁移期保留为回归基线。

## 样式层

默认样式提供稳定、克制的阅读体验。Preferences 把系统/浅色/纸张/深色主题、亮度、字号、字体、三档行距和点击/滑动翻页作为应用默认值，把书源样式开关与用户 CSS 作为本书覆盖；R5 起两层分别校验、恢复和持久化。四边距不是偏好：阅读页固定使用上 144、右 32、下 144、左 32 设备像素，系统缩放不改变这些排版值；上下安全区使 48 CSS px 工具栏在最高 300% 系统缩放下也只进入页眉页脚。亮度只过滤阅读页，不改变壳层控件。

书籍 Shadow DOM 中固定按书源 CSS、Atha 阅读样式、用户 CSS 排列。书内 style、stylesheet link 和元素 inline style 都纳入书源样式开关，外链与内联 CSS 保持原 DOM 顺序。用户 CSS 可检查、启停和撤销，拒绝 `@import`、`url()` 与 Shadow 边界选择器；它不能修改应用壳。主题、字体、密度或样式层变化统一由 Navigation 在重排前捕获 Locator，布局稳定后恢复。

应用内样式社区、评分、JavaScript 扩展和发布流程不属于阅读内核。远程共享的具体协议在确有需求时再确定。

## 本地书架与应用内导入

Tauri 无启动书籍参数时显示 Svelte 书架，并通过官方文件对话框选择一个或多个 EPUB。`reader::library::LocalLibrary` 是书架边界，只暴露列出、导入、打开、读取封面和移除；它复用 `reader::epub`，以完整源文件 SHA-256 作为书籍身份，在 `%LOCALAPPDATA%/Atha/Library` 为每书保存一份受限 JSON，在既有 `%LOCALAPPDATA%/Atha/ImportedBooks/<sha256>` 保留导入缓存。移除只删除书架记录，因此再次导入仍可恢复同一内容身份下的阅读状态。

EPUB importer 从 OPF 有界提取标题、至多 16 位作者和一个受支持的封面资源；无封面时由壳层显示占位。Svelte 只接收书籍身份、标题、作者、封面可用性和导入时间，不接收源路径、缓存路径或书籍内容。打开书籍后，宿主把动态 `atha-book` 根切换到已校验缓存；`atha-cover` 根据书架记录只读提供封面。书架沿用 Readest 的选择文件、内容哈希去重、耐久目录和打开链路，不采用其同步、分组、转换队列、多来源或全局状态结构。

## 书籍输入与阅读会话

阅读页的运行时书籍输入始终是受控书根内的 schema 1 manifest。manifest 以书籍内容哈希标识版本，声明有序且唯一的 section、可访问资源和可选 TOC；未知字段、重复项、超量输入、编码绕过、绝对路径、查询和书根越界均拒绝。单 XHTML `entry` 只作为现有样本的兼容入口。

Windows host 的 `--epub` 是运行时 manifest 之前的导入入口。后端 `reader::epub` module 读取一个 EPUB3 rendition 的 OCF、OPF manifest、spine 和 navigation document，把 spine XHTML 与当前支持的 CSS、SVG、PNG、JPEG、GIF、WebP 原子写入 `%LOCALAPPDATA%/Atha/ImportedBooks/<source-sha256>`，再交回既有 `BookRoot` 与 `ReadingSession`。缓存目录和 `contentVersion` 都使用完整源文件 SHA-256，因此相同内容跨路径复用身份，内容改变则形成新身份；导入器不解释 Locator、分页或阅读状态。

首版只支持 UTF-8 XML 的 EPUB3、单 package、XHTML spine 和 EPUB3 TOC。源文件和解压总量上限为 512MiB，成员数上限 10000，单成员上限 16MiB；加密、DOCTYPE、外部 URL、重叠/重复/Windows 歧义路径、未知 spine 类型，以及缺失的 spine、navigation 或受支持资源均明确失败。内联 SVG `image href` 只有在指向 manifest 已声明的同书资源时才加载；其他 SVG 外部引用继续拒绝。EPUB2/NCX fallback、多 rendition、远程资源、字体、混淆、修复和多格式工厂不属于当前契约。

`Section` 是一次只加载一份的顺序内容单元；`ReadingSession` 是当前打开书籍的瞬时状态，只负责按索引打开 section、关闭内容和报告 `opening`、`content-loaded`、`layout-stable`、`closed` 或 `failed`。打开另一 section 前必须释放上一 section 的 DOM、书源样式和缓存；关闭后不保留书籍 DOM。TOC 跳转、Locator 和耐久阅读位置不属于 R1 会话。

### Locator 与导航

schema 1 Locator 是同一书籍内容版本内的内容坐标：起点由 section id 和该 section DOM 文本节点文档顺序中的 UTF-16 偏移组成，range 可再带一个同 section、不早于起点且不超出实际文本的终点。它可严格序列化、解析并按 manifest section 顺序比较；跨 section range 没有当前选择消费者，等真实交互需要时再扩展。显示页码只是当前布局的投影，不进入 Locator。

字号和 CSS 重排前捕获当前可见 Locator，布局稳定后再定位到包含该文本偏移的页面。损坏 Locator、错书版本、未知 section、越界偏移或缺失 TOC fragment 回落到安全 section 起点，并在只读诊断中记录原因，不让会话失效。进度与书签只恢复同一内容版本的 Locator；R7 只为带原文快照的标注增加同 section 唯一原文重锚。

`Navigation` 组合 reading session、Locator 与 pagination，统一处理页内移动、section 边界、全书近似进度和 TOC 跳转。移动阅读壳层默认沉浸，点击正文中央临时显示覆盖层；目录以受控原生 TOC 为数据源投影全屏按钮列表，书签作为对应章节下的目录项，添加与取消只由右上角书签入口触发。点击章节或书签后等待现有导航队列稳定，再关闭目录并返回沉浸阅读。

### 阅读状态与书签

Windows host 使用持久 WebView2 profile，并从规范入口路径计算只含 16 个十六进制字符的稳定状态键，不把用户路径交给页面。EPUB 导入入口的规范路径位于以完整源 SHA-256 命名的缓存目录，因此移动源文件不改变状态键；manifest 同时提供相同内容版本。旧 `entry` 兼容入口仍由 host 根据 XHTML 字节生成 64 个十六进制字符的内容指纹。页面以状态键分区三个 schema 1 记录：应用偏好跨书共享，本书偏好与书签按书保存，进度仅保存内容版本和 Locator。输入有严格结构、长度与书签数量上限；损坏状态被安全丢弃或在定位时回落，存储不可用时当前会话仍可继续。

稳定导航只在同一任务末尾合并写入一次小型进度记录，并在页面隐藏或离开时同步 flush。恢复顺序是有效偏好优先，再恢复同内容版本且可定位的进度；错版本进度不应用，错版本书签保留并显示为不可跳转。书签只提供当前位置创建、去重、跳转与删除；书籍身份迁移、跨版本重锚、同步和历史记录不属于本层。

### 书内搜索

Search 按 manifest section 顺序只读获取 XHTML，以 `DOMParser` 拒绝解析错误、doctype 和 active content，移除样式节点后扫描与渲染 DOM 相同顺序的正文文本；明确隐藏的文本以不可匹配的等长哨兵保留 offset。它不加载书籍资源、不替换当前内容 DOM，也不改变 reading session；命中项使用原文本 UTF-16 偏移生成 schema 1 range Locator，再由 Navigation 跳转并验证目标起点。可定位结果必须在当前页可见，其他需完整渲染才能确定的候选明确报告失效。

R6 只提供不区分大小写的字面量搜索。查询最长 128 个 UTF-16 code unit，单次最多保留 2000 条结果并明确报告截断；新查询和显式取消都通过 `AbortController` 终止旧扫描，旧扫描不得回写新状态。结果、错误和进度只存在于当前页面，任一章节失败不会让阅读会话失效。worker、持久缓存、搜索索引、历史和高级匹配只在真实大书证明需要时增加。

### 标注与引用

Annotation Store 以独立的每书 schema 1 记录保存用户事实，不与高频进度或本书偏好一起重写。每条记录包含稳定 id、highlight 或 note 类型、schema 1 `SourceAnchor`、笔记、创建/更新时间和 `deletedAt` tombstone；删除不物理移除。写入先由 localStorage 成功接收完整记录，再替换内存状态；存储不可用或记录损坏时禁止覆盖并只在标注域报告，不使 reading session 失败。

`SourceAnchor` 包含 canonical range Locator、至多 4096 个 UTF-16 code unit 的原文、前后各 32 个 code unit 的上下文和原文 UTF-8 SHA-256，字段语义可直接映射到未来消息链路的 `source_anchor`。同版本先验证 Locator 指向的原文；版本或文本不一致时，只在原 section 中接受唯一原文命中并更新 canonical Locator，零个、多个命中或缺失 section 都报告重锚失败。

Annotations 从原生选择产生 `SourceAnchor`，只把当前 section 的未删除事实投影到浏览器 CSS Custom Highlight。切章和重新渲染后按事实重画，字号与样式重排继续使用同一 Range；Range 与 overlay 不进入存储。有效新选区附近显示复制、标注和笔记；点击已有标注则恢复其 Range，并显示复制、重选、笔记和删除。重选使用浏览器原生选区分两步保存新锚点，保持原记录 id 与笔记；重叠命中选择最近更新的一条，其他记录仍可从笔记页管理。

笔记继续使用最长 2000 字符的纯文本 dialog，同一个入口负责新建、为 highlight 添加笔记和预填编辑。全屏笔记页只投影未删除的 highlight 与 note；项目正文通过既有 Locator 跳转并返回沉浸阅读，独立编辑和删除动作不触发跳转。删除调用 Annotation Store 的 tombstone 写入并立即撤销正文投影。颜色、样式、notebook、同步、tombstone 压缩、导入与 SQLite 留待后续真实需求。

### 翻页输入

`Interaction` 只把键盘、滚轮、鼠标页区和单指横向滑动解释为前后翻页意图，再交给 Navigation 串行执行。它不直接修改分页或 section；编辑区、对话框、表格、代码与非折叠文本选择保留浏览器原生行为。图片和公式的点击、键盘预览语义不阻止滚轮翻页；应用壳控件仍受保护。

标准离散滚轮输入逐次产生翻页意图；小幅高频输入先累计阈值，并在同一精密手势的空闲窗口结束前抑制惯性尾流。`scripts/check-reader-wheel.ps1` 用真实浏览器记录书内媒体目标、4 次间隔 100ms 的离散输入接受率和事件到 Navigation 稳定的 P95；50ms 门槛只用于固定五页样本，同页 benchmark 与多章节样书继续分别记录分页成本和跨章成本。

### 文本、链接与脚注

正文选择与系统复制命令保留浏览器原生行为。选择动作条的复制只在用户手势中把已保留的原生 Range 交给浏览器复制命令；应用不读取剪贴板，不持久化复制内容，也不经 IPC 或网络传输。内容边界只接受同书 XHTML 链接和无凭据 HTTP/HTTPS 外链；危险 scheme、目标窗口与下载属性仍拒绝。同书链接统一由 Navigation 按 section URL 和 fragment 跳转，fragment 定位跳过目标开头的不可见空白；未知目标安全回落。外链不在 WebView 导航或请求网络，只显示已阻止反馈。

同章 noteref 可以把目标纯文本投影到原生 dialog，关闭后焦点返回触发链接；跨章节脚注作为普通书内链接导航。脚注 HTML 不复制到应用壳，也不因此放宽脚本或资源信任边界。

### 图片与公式预览

非链接图片在完成资源校验后获得按钮语义、键盘焦点和可见焦点状态，单击、Enter 或 Space 使用应用壳现有原生 dialog 预览同一受控资源。链接包裹的图片只保留链接语义，避免一个内容节点同时触发链接和预览。

预览只设置独立图片元素的已校验本地 URL、安全标题和替代文本，不复制书源 HTML、样式或脚本。普通图片始终保留原色，公式预览沿用正文的明暗主题过滤；关闭后焦点返回原图片，且不改变 section、页码或 Locator。图集、缩放、平移、保存和 OCR 等能力在出现明确需求前不建设。

### 表格与代码预览

表格和块级预格式化内容保持原生语义与选择能力，并获得键盘焦点和可见焦点。双击或在自身焦点上按 Enter、Space 使用现有原生 dialog 打开独立预览；正文中的单击、链接与选择仍优先，表格和代码区域不触发背景翻页。

表格预览由应用从 caption、行、表头、单元格安全文本及最多 100 的合法行列跨度重建；单元格中的图片只使用限长替代文本。代码预览只设置 `textContent` 并保留空白。两者均不克隆书源 HTML、样式、链接、图片或事件属性，关闭后恢复焦点且不改变 section、页码或 Locator。缩放、拖拽、复制按钮、导出、执行和编辑留待明确需求。

### 首个验收基线

长期验收样本位于本机忽略目录 `fixtures/local/`。当前清单包含三个既有单章节样本，以及从《数学及其历史 (2026)》固定哈希源文件重复导出的“1.1 算术与几何”“1.2 勾股数组”“1.3 圆上的有理点”三章节 R1 样本；源 EPUB 不修改，也不提交样本内容到仓库。`scripts/export_reader_sample.py` 支持导出单章节或带 manifest 的多章节样本，`scripts/check-reader-samples.ps1` 统一运行实际 Windows host 与明暗主题截图验收。

阅读页填充 WebView 视口，内部画布尺寸等于视口 CSS 像素乘 `devicePixelRatio`。页内字号、固定边距、栏宽、公式和图形尺寸继续使用绝对设备像素；显示层以 `1 / devicePixelRatio` 抵消系统 DPI。Windows 窗口、48 CSS px 覆盖工具层和错误提示使用系统逻辑像素并遵循 DPI。窗口停止调整后，Pagination 经 Navigation 队列以变化前 Locator 重排并恢复位置；控制层显隐不改变书页、正文列或 Locator 几何。

正式内容回归覆盖 780 × 1680、960 × 720 的 DPR 1 视口，以及 390 × 840 的 DPR 2 视口。780 × 1680 不再是产品页面的固定尺寸；benchmark 记录每次运行的真实内部设备像素尺寸，以免跨布局误判性能。阅读器的一页是有固定四边距的分页内容区，不是任意滚动位置的截图：不得裁切文字行、公式或图形。左右边距固定为 32 设备像素，上下边距固定为 144 设备像素，其中包含不会被系统缩放工具栏越过的页眉页脚安全区；用户设置不能修改这四项。首个字号基线为 32px、行距为 1.6，均可从阅读设置调整。

公式按已标记的语义类别处理。行内公式保留书源的相对宽高，以当前正文与书源基准字号的同一倍率等比缩放并对齐基线；不得强行变为同一高度。行间公式独立居中，超出可用宽度时整体缩小而不裁切。普通 SVG 和插图不套用公式规则。

阅读页跟随系统 `prefers-color-scheme`。暗色下只对 `.math-inline` 与 `.math-display` 图片应用反色；普通插图保持原色。清单显式声明样本是否应含公式以及普通图片数量，避免零内容空通过。

### M2 交付门槛

`scripts/check-reader-gate.ps1` 是 M2 的组合验收入口。它先构建当前源码并运行四困难样本，再从固定哈希的《数学及其历史》生成仅用于 fixture 的全 XHTML manifest，核对 173 个 section、固定全书搜索、三轮 host 与全部 WebView2 后代的 working set、绕过正常关闭的强杀恢复，最后运行 10 样本 benchmark。全 XHTML 模式不是 EPUB 导入器，不解析 OPF spine，也不建立 M3 书籍身份。

本机 nearest-rank P95 门槛固定为：冷启动 2000ms、首个稳定页 750ms、热打开 120ms、翻页 50ms、字号重排 150ms。进程树内存继续采样和记录，但不设置失败门槛。只有总 gate 测出瓶颈时才增加对应优化；运行结果由代码库地图和当前 change 保存，不把本机数值当作跨设备性能承诺。

`scripts/check-tauri-reader.ps1` 对产品入口运行前端检查、production build、workspace Rust 检查、Tauri debug build、真实 EPUB import probe 和相同五项性能门槛。benchmark 模式只运行性能探针，不夹带功能验收。

### M3 EPUB 入口门槛

`scripts/check-epub-source.ps1` 是单格式真实输入验收入口。它运行锁定的 Rust 检查，使用固定 SHA-256 的《数学及其历史 (2026)》通过 `--epub --verify-import` 启动真实 Windows WebView2 host，并核对导入结果为 173 个 spine section、2527 个受支持资源和 197 条 EPUB3 TOC。import probe 只验证前三个真实 spine section 的加载、旧 DOM 释放、重开和网络安全探针；内容特定的排版、交互、搜索、标注、恢复、内存与性能继续由 M2 gate 拥有。

## 性能策略

- 使用内容已知尺寸或缓存测量结果为重内容预留空间，减少重排。
- 当前视口和即将进入视口的内容优先；其他公式、SVG 和重资源可渐进完成。
- 初次打开可以执行可复用预处理，后续读取优先复用结果。
- 缓存有效性与书籍内容、渲染内核版本和生效样式版本相关；任一变化后按需重建。
- 性能优先模式可以削减部分视觉效果，但不得伪造或篡改原文内容。
- 优先保持单个阅读会话中的 WebView 与阅读页存活，并先校正真实用户阶段的计时；持久缓存、渐进解码和其他专项优化由后续功能与实测瓶颈驱动。

## 位置与版本

阅读位置按书籍状态键与内容版本保存，样式变化后仍以内容 Locator 恢复到同一文本附近。EPUB 状态键绑定内容哈希缓存路径，不绑定源文件位置；内容版本变化时不猜测旧位置。

## 非责任

- 阅读页不定义书籍格式 parser；EPUB3 导入细节只属于后端 `reader::epub` module；
- 不承载消息、AI 或同步协议；
- 不替用户修复损坏书源；
- 不预设跨机器性能数值，后续规格基于困难书籍样本决定。

## 相关文档

- 产品定义：`docs/product/OVERVIEW.md`
- 消息与共读：`docs/architecture/MESSAGE-READING.md`
- 数据库基线：`docs/codebase/DATABASE.md`

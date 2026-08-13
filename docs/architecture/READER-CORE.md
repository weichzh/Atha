# 阅读内核

## 责任

阅读内核负责把导入后的书籍内容以 HTML、CSS 和本地资源的形式呈现。它不以某个书籍封装格式为中心：EPUB、CBZ、FB2 / FBZ、MOBI / AZW / AZW3、Markdown 与 TXT 都在导入后投影为同一 ReaderManifest / BookRoot，其他来源也必须先归一到同一内容模型。

## 兼容性契约

- HTML、CSS、字体、数学公式与 SVG 的呈现以浏览器为质量基准。
- 浏览器基准不意味着执行不受控网页：书内脚本禁用；外部网络资源默认拦截，用户可单次或按书确认加载。
- 无法可靠呈现的内容必须明确报错；不进行不可见的兼容性猜测或书源修复。

## 渲染技术

平台 WebView 是当前唯一阅读渲染技术：Windows 使用 WebView2，Linux 使用 WebKitGTK，Android 使用系统 WebView。宿主只提供窗口、受控资源、导航拦截与有限遥测，HTML、CSS、布局和绘制继续由浏览器完成；不维护自研或组合式第二引擎。

只有外部引擎的成熟度发生实质变化，并能在 ATHA 困难样本上同时证明浏览器兼容、文本选择与重锚、安全、无裁切和同机性能优势时，才通过新的研究与 change 重新决策。本轮及可预见功能开发只对各平台现有 WebView 做常规优化；会话内只保留三槽 section 缓存和相邻原文预取，不建设缓存数据库、全书解析预热或虚拟化框架。

### 应用壳与宿主

产品入口采用 Tauri 2 与 Svelte 5，但每个平台仍只有一个 WebView。Svelte 只拥有顶部栏、底部工具栏、面板和 dialog；现有 reader kernel 继续直接控制 closed Shadow DOM，不把 XHTML、页内节点或分页热状态放进组件树。Vite 构建时按既定顺序拼接现有阅读模块，避免形成第二份内核。

Tauri 复用后端书根、全部已支持格式导入、共享 CLI、窗口尺寸和诊断逻辑。书籍资源仍走受控 `atha-book` 协议；可信的 Svelte 应用壳只调用固定书架 command，书内文档没有 command 接口，XHTML 和图片不经 IPC 传输。阅读器遥测仍由严格校验、串行发送的独立 command 接收。应用响应使用 `Permissions-Policy` 禁用相机、麦克风、定位、显示捕获等浏览器权限；浏览器暴露策略检查 API 时，reader 会从真实文档策略复核关键能力确实不可用，WebKitGTK 未暴露该 API 时仍由原生响应头和 Rust 静态检查守住边界。直接 Wry/Tao 的 `atha-reader-host` 在迁移期保留为回归基线。

Android 继续 edge-to-edge，但不依赖旧系统 WebView 尚未完整支持的 CSS `safe-area`。`MainActivity` 在 UI 线程观察 `systemBars | displayCutout`，只缓存四边物理像素并原样下传 insets；boolean-only bridge 负责阅读态状态栏显隐与图标明暗，Web 层把同一组值转换为 CSS 像素后只消费自有变量。书架与工具面板避开状态栏 / 导航栏，阅读态隐藏状态栏时左上章节标题仍在 cutout 安全区下方；工具打开时隐藏章节标题并把内容区放到顶部工具栏之下。native 不给 WebView 再加 padding，也不叠加 `env(safe-area-inset-*)`，避免新旧 WebView 双重避让。

## 样式层

默认样式提供稳定、克制的阅读体验。Preferences 把系统/浅色/纸张/深色主题、亮度、16–40 逻辑 CSS px 字号、字体和三档行距作为应用默认值，把左右翻页 / 上下滚动、书源样式开关、三档左右边距、段首缩进、段距和有序 CSS 模块作为本书覆盖。旧的单段 `userStylesheet` 恢复时迁移为 `legacy-user-css` 模块；旧应用记录中的四个自由边距字段和点击 / 滑动开关继续忽略。上下正文安全区固定为 144 设备像素，Android 只在顶部 / 底部安全区更大时扩大对应排版边距；左右边距默认 32，可按书选择 24 / 32 / 48 设备像素。亮度只过滤阅读页，不改变壳层控件。

书籍 Shadow DOM 中固定按书源 CSS、Atha 阅读样式、可视排版 CSS 与已启用用户模块排列。书内 style、stylesheet link 和元素 inline style 都纳入书源样式开关，外链与内联 CSS 保持原 DOM 顺序。每书至多 32 个用户模块，新建或导入模块单个至多 32 KiB、启用组合至多 64 KiB；旧单段 CSS 仍按原 32768 UTF-16 字符上限完整迁移，超过组合上限时只保留为停用恢复副本。名称、分组、ID、顺序、启停和 schema 1 JSON 导入均严格校验。`style-module-package.mjs` 独立拥有模块包解析、序列化和结构限制，并注入 `content.validateStylesheet()` 复用同一 CSSOM 边界；Preferences 只拥有 UI、每书状态、按模块隔离的预览 timer 和组合。手动保存只取消同模块的待处理预览，删除、导入和重置清理失效任务。任一启用组合仍拒绝 `@import`、子资源、转义和 Shadow 边界选择器；验证、重排或持久化失败统一恢复上次状态、渲染与 Locator。CodeMirror 只在模块页可见时按需增强隐藏 textarea 的编辑体验，不能修改应用壳或绕过 CSSOM。主题、字体、密度或样式层变化统一由 Navigation 在重排前捕获 Locator，布局稳定后恢复。

应用内样式社区、评分、JavaScript 扩展和发布流程不属于阅读内核。完整 GitHub 社区已暂缓；未来任何数据源只能把文本交给同一个 schema 1 模块包 codec，不在阅读内核加入账号、网络、审核或任意插件执行能力。

## 本地书架与应用内导入

Tauri 无启动书籍参数时显示 Svelte 书架，并通过官方文件对话框选择一个或多个 EPUB、CBZ、FB2 / FBZ、MOBI / AZW / AZW3、Markdown 或 TXT。`reader::library::LocalLibrary` 是书架边界，只暴露列出、登记、导入、打开、读取封面、移出书架和删除本地书籍数据；已知 `.epub`、`.cbz`、`.fb2` / `.fbz`、`.mobi` / `.azw` / `.azw3`、`.md` / `.markdown`、`.txt` 严格分派到对应 importer。Android content URI 由 Tauri 内置 PathPlugin 经 `ContentResolver` 取得显示文件名，只保留允许列表后缀后复制到 Picker cache；provider 不返回可用后缀时稳定拒绝，不从 URI 或正文猜格式。加入书架时只把源文件耐久写入 `SourceBooks`、计算既有内容身份并写受限记录；首次打开在 blocking worker 中调用同一 importer 发布 `ImportedBooks`，以后直接打开缓存。同进程准备由共享锁串行发布，缓存命中先核对格式 marker 与元数据，再核对 manifest 及其声明的全部 section / resource；EPUB v2 至 v5 完整缓存都可读，有耐久源时才尝试把 v2 至 v4 升级到 v5，失败仍回退旧缓存。缺失、空 section 或损坏产物由任一同身份耐久后缀重建。再次登记同一内容可原子修复身份异常的耐久源或重建书架记录。EPUB / CBZ 保持原始文件 SHA-256，Markdown / TXT 使用各自格式域，FB2 / FBZ 使用解包后 FB2 XML 与固定格式域生成身份，Kindle 三种后缀使用同一固定格式域，因此相同字节不会因封装后缀产生副本。移出书架只删除书架记录；删除本地数据还删除同身份耐久源、导入缓存和本书浏览器状态，但始终保留 Message、Anchor、Snapshot 与资产。

EPUB importer 从 OPF 有界提取标题、至多 16 位作者和一个受支持的封面资源；EPUB2 的 `meta name="cover"` 与 EPUB3 的 `cover-image` 最终投影为同一封面字段。CBZ 只消费可选 `ComicInfo.xml` 中有界的 `Title`、`Writer` 与唯一有效 `FrontCover`。FB2 从 `description/title-info` 有界投影书名、作者和封面引用。无效、冲突或超限的可选元数据不会扩大受信任内容边界；无封面时由壳层显示占位。Svelte 只接收书籍身份、标题、作者、封面可用性、导入时间和是否已准备，不接收源路径、缓存路径或书籍内容。未准备书籍首次打开时显示原生不定进度条；完成后从 importer 元数据补齐标题、作者和封面。宿主随后把动态 `atha-book` 根切换到已校验缓存；`atha-cover` 根据书架记录和已发布元数据只读提供封面。书架沿用 Readest 的选择文件、内容哈希去重、耐久目录和打开链路，不采用其同步、分组、转换队列、多来源或全局状态结构。

书架搜索与排序只投影内存中的受限 `LibraryBook[]`：搜索匹配标题 / 作者，默认保持后端导入顺序，另提供稳定的书名与作者顺序。进度页只读同源 schema 1 进度记录，并复用阅读器的内容版本、Locator、大小和精确字段约束；合法同书记录只表示“在读”，缺失或无效记录表示“未开始”，存储不可访问时禁用进度投影而不伪造状态。显式选择模式串行完成当前结果全选、批量移出或批量删除本地数据；普通模式没有常驻破坏性按钮。封面使用浏览器原生 lazy loading / async decoding，移动端保持三列，未引入虚拟列表、同步状态或新 DTO。

`backend::local_data::LocalData` 是应用级本地数据协调边界。schema 1 `.atha-data` 只包含规范化书架记录、全部 `SourceBooks`、离线词典、嵌套 MessageStore `.atha-backup` 和生产 localStorage allowlist；`ImportedBooks`、Picker cache 与日志不进入制品。恢复在当前数据根内完成唯一 entry、路径、数量、容量、哈希、书源身份、词典、MessageStore 和浏览器状态校验，再以同文件系统目录切换和 SQLite Online Backup 发布。`prepared` 表示尚未发布，第一次目录切换前耐久写入 `publishing` marker；启动看到未提交发布时回滚，`committed` 等待 WebView 同步替换浏览器状态并确认；文件数上限 100,000，总展开上限 8 GiB，浏览器状态上限 16 MiB。

资料库管理菜单提供完整备份、完整恢复和书籍文件、阅读缓存、消息与快照、词典、阅读设置的分类占用。批量选择同时提供“移出书架”和“删除本地数据”，使用不同确认文案；恢复的浏览器状态替换失败会回写操作前 allowlist，删除则保留耐久 intent 与已经完成的定点清理，在重载后幂等续做，两者都不读取或改写其他 origin 数据。缺少耐久原文件的旧书架项不会伪装成完整备份，界面要求用户重新选择原文件补齐后再试。Tauri `local_data_maintenance` 只接受主窗口资料库根路由，SAF 继续复用 `platform_file`，产品日志只记录操作、固定阶段、稳定错误码和耗时。

## 书籍输入与阅读会话

阅读页的运行时书籍输入始终是受控书根内的 schema 1 manifest。manifest 以书籍内容哈希标识版本，声明有序且唯一的 section、可访问资源和可选 TOC；未知字段、重复项、超量输入、编码绕过、绝对路径、查询和书根越界均拒绝。单 XHTML `entry` 只作为现有样本的兼容入口。

Windows host 的 `--epub` 是运行时 manifest 之前的导入入口。后端 `reader::epub` module 读取单个 EPUB2 或 EPUB3 rendition 的 OCF、OPF manifest、spine 和 navigation document，把 spine XHTML 与当前支持的 CSS、SVG、PNG、JPEG、GIF、WebP 原子写入 `%LOCALAPPDATA%/Atha/ImportedBooks/<source-sha256>`，再交回既有 `BookRoot` 与 `ReadingSession`。EPUB2 按 OPF `spine@toc` 解析对应的 `application/x-dtbncx+xml` NCX，把 `navMap` 的嵌套 `navPoint` 按前序拍平成现有 `ReaderManifest.toc`；EPUB3 优先使用唯一 XHTML nav，缺失时回退到 `spine@toc` 指向的有效 NCX，两者都没有时以空 TOC 继续导入可读 spine。NCX 中多个目录项规范化到同一 href 时稳定保留文档顺序中的第一项。两条路径共享相同的 section、资源、路径、大小和 XML 深度边界，不建立第二套 importer。v5 首次准备对同时缺少显式 `width` 与 `height`、可读取尺寸且未超预算的本地图片使用 `imagesize` 与 EXIF 方向补入原生 HTML 尺寸和 Atha 标记；8192 单边、20000000 像素预算不变。阅读器在书源 CSS 之前为最多 512 个唯一尺寸对生成零特异性的 `contain:size` / `contain-intrinsic-size` 规则，常见未分层作者与用户 CSS 可继续覆盖；额外尺寸对退回原生属性与 `height:auto`。旧 v4 `data-atha-intrinsic-*` 只为无耐久源缓存保留兼容。获得增强的普通图片在解码前已有稳定宽高比；几何盒随正文立即存在，像素随后异步绘制，不建立普通图片、整节或全书的等待揭示闸门。失败图片优先保持连接状态下的非零实际盒，随后退回合法原生、固有或 HTML 属性尺寸；合法零像素盒或已经脱离 DOM 且只靠书源 CSS 定形时不承诺精确复原。损坏 EXIF、IDAT 后的 PNG `eXIf`、解析或 XML 增强失败只跳过提示，已经读到 SOF 的无 EXIF JPEG 可忽略后续扫描尾缺失。v2、v3、v4 与 v5 完整缓存都可读；有耐久源时 v2 至 v4 按需重建为 v5。XHTML 身份以 OPF manifest 的 `application/xhtml+xml` 为准，不依赖文件扩展名；`BookRoot` 只对 reader manifest 已声明的 section 返回 XHTML MIME。缓存目录和 `contentVersion` 都使用完整源文件 SHA-256，因此相同内容跨路径复用身份，内容改变则形成新身份；导入器不解释 Locator、分页或阅读状态。

`reader::text` 是 Markdown / TXT 的具体导入 adapter，不建立格式 factory。Markdown 使用 `pulldown-cmark 0.13.4` 的事件流生成受控 XHTML：原始 HTML 转义，链接只保留 label，图片只保留 alt，占位不获取路径或网络；首个 H1 前内容和各 H1 形成 section / TOC，固定的最小样式只补齐代码换行、表格边框、等宽代码与引用缩进。TXT 由 BOM、严格 UTF-8 或 `chardetng 1.0.0` + `encoding_rs 0.8.35` 识别，至少两个高置信整行章节标题才建立语义 TOC；相邻章节按约 1 MiB 软上限合并为 XHTML section，每章保留受控 fragment。两种格式都在发布前复核源文件未变化，使用同文件系统 staging 与原子发布；Markdown 源与单 section 上限为 16 MiB，ReaderManifest 仍最多 1000 sections / 2000 TOC items。

`reader::cbz` 以路径分段 ASCII 数字自然序排列 ZIP 内的 JPEG / PNG，忽略隐藏段、`__MACOSX` 和非图片成员，并为每图生成一个 Atha 控制的 XHTML section 与一个声明资源。`imagesize 0.15` 只校验 JPEG / PNG 魔数、非零尺寸、8192 单边和 20000000 像素预算；ZIP CRC 与 WebView `img.decode()` 继续覆盖其余损坏，尾部损坏显示可访问占位并允许继续导航。

`reader::fb2` 用已有 `quick-xml 0.41` 的声明编码支持做两遍有界流式解析，并用 `base64 0.22.1` 解码书内二进制。它只接受直接 `.fb2` 或恰含一个根级 `.fb2` 成员的 `.fbz`，将正文、notes body、目录、内部锚点和 JPEG / PNG 图片投影为 Atha 控制的 XHTML 与 manifest；源 stylesheet、外链、DTD、处理指令、脚本、未知正文元素、未知二进制类型、损坏引用及超限输入均拒绝。直接 FB2 上限为 64 MiB；FBZ 复用共享 archive 边界，成员仍受 16 MiB 上限。

`reader::kindle` 使用固定且关闭默认 features 的 `boko 0.5.0` 恢复 PalmDOC / MOBI6 与纯 KF8 / AZW3。进入依赖前，Atha 独立验证 256 MiB 源上限、PDB record 表、MOBI version、compression、encoding、encryption、词典索引和正文 / HUFF 预算；纯 KF8 按 header 路由，不信任 `.azw` 后缀。依赖输出继续经过 Atha XHTML 白名单、链接重写、JPEG / PNG / GIF magic 与尺寸检查、唯一目录目标和原子发布，DRM、字典、未知压缩 / 编码、活动内容、悬空资源与超限输入稳定拒绝。`boko` 的公共 raw API 当前不能加载 KF8 flow stylesheet，因此首版移除对应 link，不发布悬空 CSS；书内图片和安全 inline style 保留，源样式完整保真留待上游 API 或真实需求证明需要最小 fork 时再补。

首版 EPUB 兼容边界是 UTF-8 XML、单 package 和最多 2000 个 XHTML spine；TOC 是可选能力，EPUB3 XHTML nav 缺失时先尝试兼容 NCX，导航成员缺失或内容不合法时再降级为空。NCX 可无 DOCTYPE，或只包含精确 canonical NCX 声明；带声明时要求合法且唯一的 `playOrder`，不加载外部 DTD 或通用实体；重复 href 只保留文档顺序中的第一项。正文和搜索在 `DOMParser` 前只白名单精确的 HTML5、XHTML 1.1 与兼容扩展 XHTML 1.0 Strict 声明，并先剥离声明；仅在该白名单分支把 XHTML 预定义外的 `&nbsp;` 规范为数值实体，未知、重复或带 internal subset 的声明继续拒绝。container 只允许位于根元素前的单一外部 DOCTYPE，OPF 继续拒绝 DOCTYPE；两者都不加载 DTD 或通用实体。

EPUB 后缀已由导入入口确认时，导入器以有界的 container 和 package 结构识别书籍，不再因 `mimetype` 成员缺失、压缩或排序不规范拒绝正文。归档成员只归一字面 `.` 段；OPF / nav 引用逐段严格解码一次百分号编码，仍拒绝编码后的点穿越、分隔符、控制字符、二次编码、查询、绝对路径和书根越界。多个 manifest ID 只有在规范路径、media type 和 properties 都相同时才可作为兼容别名；重复 ID 或冲突别名继续拒绝。缺失的 spine 成员被跳过，但至少保留一份可读 XHTML；缺失或无效封面不发布封面，缺失的已支持图片、样式表和 SVG 不进入 ReaderManifest。前端只对这些同书可选资源使用替代文本或移除对应书源样式单元，不发起未声明或网络请求；脚本、事件处理器、表单、未知资源属性和外部 URL 仍硬拒绝。

EPUB / CBZ / FBZ 共用 `reader::archive` 的 512 MiB 源文件与声明解压总量、10000 成员、加密、重叠、symlink、重复 / Windows 歧义和路径边界。索引阶段允许至多 32 MiB 的成员，以识别但不发布较大的可选字体；实际读取、写入与 WebView 资源仍受 16 MiB 单成员上限，写入成员与 CBZ 页面另按实际读取量累计。`zip 8.6` 没有打开前的 `max_entries` 配置；Atha 先以标准 terminal EOCD hint 拒绝超过 10000 项、trailing garbage 与歧义 terminal EOCD，再在打开后校验实际条目数。该 hint 不是完整 ZIP parser，fallback / ZIP64 在 post-open 检查前的最坏预分配仍受源文件上限约束；一个可由系统 ZIP 工具打开的 pseudo-ZIP64 变体仍受 `zip 8.6` 上游解析限制。`META-INF/encryption.xml` 只接受精确 IDPF 或 Adobe 字体混淆算法、已声明且存在的字体 MIME 和唯一安全路径；这些字体不发布并回退系统字体，其他加密或未知结构继续整书拒绝。UTF-16 XML、DTBook、OEBPS 文档、fallback 链、完整 EPUB2 Reading System、多 rendition、远程资源和通用修复框架尚未完成。

## 离线词典

`reader::dictionary::LocalDictionaries` 独立于普通书籍导入器，在应用数据根的 `Dictionaries` 目录事务导入、列出和移除词典。MDict v2 的 MDX 与最多四个 MDD 固定交给 `mdict-rs 0.1.4`，MDX 精确查询最多跟随八层链接；MDD 先查资源范围和 32 MiB 上限，再按块读取。经典 Kindle 只支持当前样本所需的 MOBI6、CP1252、HUFF/CDIC 与正排 INDX 精确查询，导入时按真实 HUFF 解压长度生成有界 `dictionary.offsets`，查询按稀疏首键和正文累计偏移二分后只解压目标 records；旧导入缺失 sidecar 时可重建。ORDT、自定义排序、names / keys 屈折变化、KF8 词典、DRM、全文和模糊查询稳定拒绝或不命中。

词典源、查询、词头、释义和资源均属于私有内容。Tauri 只把词典 ID、查询和安全结果送入固定 command，后台 lookup 使用 blocking worker，日志只保留操作、格式、资源数、结果数、耗时与稳定错误码。释义先移除脚本、来源样式、表单、嵌入、媒体、图片和资源节点，再把未知元素降为 `span` 并删除全部来源属性；结果同时保留兼容纯文本 `definition` 与只含固定语义元素和后端生成角色标记的 `definitionHtml`。Svelte 只在这条后端白名单边界使用 `{@html}`，应用 CSS 提供段落、义项、音标、词性、列表、引用、表格、ruby 与上下标排版；来源 CSS、可点击地址和 MDD 富资源仍不渲染。选区“查词”只发送当前正文选区，不建立 provider、缓存、网络或插件接口。

`Section` 是顺序内容单元；`ReadingSession` 是当前打开书籍的瞬时状态，只负责按索引打开 section、关闭内容和报告 `opening`、`content-loaded`、`layout-stable`、`closed` 或 `failed`。打开另一 section 时保留当前 live DOM，目标 XHTML 完成解析、白名单校验和资源准备后再原子替换；准备或排版失败时保留上一稳定 section 与位置。会话以共享三槽、8 Mi 字符预算的 LRU 保留已准备 detached body / CSS 或相邻 section 原始 XHTML，关闭 generation 使全部在途结果失效并释放 live DOM 与会话缓存。TOC 跳转、Locator 和耐久阅读位置不属于 R1 会话。

### Locator 与导航

schema 1 Locator 是同一书籍内容版本内的内容坐标：起点由 section id 和该 section DOM 文本节点文档顺序中的 UTF-16 偏移组成，range 可再带一个同 section、不早于起点且不超出实际文本的终点。它可严格序列化、解析并按 manifest section 顺序比较；跨 section range 没有当前选择消费者，等真实交互需要时再扩展。显示页码只是当前布局的投影，不进入 Locator。

字号和 CSS 重排前捕获当前可见 Locator，布局稳定后再定位到包含该文本偏移的页面。损坏 Locator、错书版本、未知 section、越界偏移或缺失 TOC fragment 回落到安全 section 起点，并在只读诊断中记录原因，不让会话失效。进度与书签只恢复同一内容版本的 Locator；R7 只为带原文快照的标注增加同 section 唯一原文重锚。

`Navigation` 组合 reading session、Locator 与 pagination，统一处理页内移动、section 边界、全书近似进度和 TOC 跳转。移动阅读壳层默认沉浸，点击正文中央临时显示覆盖层；目录以受控原生 TOC 为数据源投影全屏按钮列表，书签作为对应章节下的目录项，添加与取消只由右上角书签入口触发。点击章节或书签后等待现有导航队列稳定，再关闭目录并返回沉浸阅读。

### 阅读状态与书签

Windows host 使用持久 WebView2 profile，并从规范入口路径计算只含 16 个十六进制字符的稳定状态键，不把用户路径交给页面。所有导入入口的规范路径位于以内容版本命名的缓存目录，因此移动源文件不改变状态键；EPUB / CBZ 内容版本保持原始文件 SHA-256，FB2 / FBZ 共享解包 XML 的格式域身份，Kindle 三后缀共享一个格式域，Markdown / TXT 以不同固定格式域隔离相同字节。旧 `entry` 兼容入口仍由 host 根据 XHTML 字节生成 64 个十六进制字符的内容指纹。页面以状态键分区应用偏好、本书偏好与书签、进度三个 schema 1 记录；另有一个应用级 schema 1 阅读统计记录，以内容版本区分书籍。输入有严格结构、长度与书签、日期、书籍数量上限；损坏状态被安全丢弃或在定位时回落，存储不可访问时当前会话仍可继续。

稳定导航只在同一任务末尾合并写入一次小型进度记录，并在页面隐藏或离开时同步 flush。恢复顺序是有效偏好优先，再恢复同内容版本且可定位的进度；错版本进度不应用，错版本书签保留并显示为不可跳转。书签只提供当前位置创建、去重、跳转与删除；纯图片或只有不可见字符的页面通过当前 Locator 识别已有书签，不依赖可见文字偏移；书籍身份迁移、跨版本重锚、同步和历史记录不属于本层。

阅读统计只在排版稳定、沉浸阅读、页面可见、窗口聚焦且最近 5 分钟有键盘、指针、滚轮或导航活动时累计。打开工具层、隐藏、失焦和闲置都会暂停；15 秒心跳使用单调时钟，超过 30 秒的间隔按休眠或调度中断丢弃，墙钟只负责把已接受的短区间按本地午夜拆分。原子 localStorage 记录最多保留最近 400 天和 2048 本书，总长仍受 512 KiB 上限约束；写入失败只降级为当前会话内存统计，不阻断阅读，也不写入产品日志。进度面板从同一记录投影今日、近 7 天、本书累计和每天至少 1 分钟的连续阅读；多窗口并发、账户同步、趋势图和导出不属于当前边界。

### 书内搜索

Search 按 manifest section 顺序只读获取 XHTML，与正文共享精确 DOCTYPE 白名单和剥离逻辑，再以 `DOMParser` 拒绝解析错误、残留 doctype 和 active content，移除样式节点后扫描与渲染 DOM 相同顺序的正文文本；明确隐藏的文本以不可匹配的等长哨兵保留 offset。它不加载书籍资源、不替换当前内容 DOM，也不改变 reading session；命中项使用原文本 UTF-16 偏移生成 schema 1 range Locator，再由 Navigation 跳转并验证目标起点。可定位结果必须在当前页可见，其他需完整渲染才能确定的候选明确报告失效。

R6 只提供不区分大小写的字面量搜索。查询最长 128 个 UTF-16 code unit，单次最多保留 2000 条结果并明确报告截断；新查询和显式取消都通过 `AbortController` 终止旧扫描，旧扫描不得回写新状态。结果、错误和进度只存在于当前页面，任一章节失败不会让阅读会话失效。worker、持久缓存、搜索索引、历史和高级匹配只在真实大书证明需要时增加。

### 标注与引用

正式产品由 `backend::messages::MessageStore` 保存阅读事实；旧的每书 localStorage Annotation Store 只作为一次性迁移输入和非 Tauri 浏览器回归夹具。首次启用正式存储时，旧标注与笔记原子、幂等地迁移为根 Message；全部提交前不写完成凭据，成功后阅读页不再双写。

`SourceAnchor` 包含 canonical range Locator、至多 4096 个 UTF-16 code unit 的原文、前后各 32 个 code unit 的上下文和原文 UTF-8 SHA-256。同版本先验证 Locator 指向的原文；版本或文本不一致时，只在原 section 中接受唯一原文命中并更新当前 Locator，零个、多个命中或缺失 section 都报告重锚失败。原始 Locator 与历史快照保持不可变。

Annotations 从原生选择产生 `SourceAnchor` 与 `SourceSnapshot` 候选，只把当前 section 的未删除根 Message 投影到浏览器 CSS Custom Highlight。切章和重新渲染后按事实重画，字号与样式重排继续使用同一 Range；Range 与 overlay 不进入存储。有效新选区附近显示查词、复制、标注和笔记；点击已有标注则恢复其 Range，并显示查词、复制、重选、笔记和删除。选区有效性由 `content.selectionRange()` 统一以真实 `Range.collapsed` 和正文归属判定，不信任厂商 WebView 可误报的 `Selection.isCollapsed`。重选使用浏览器原生选区分两步创建新的 Anchor 与 Snapshot，保持 Message 身份与笔记修订；重叠命中选择最近更新的一条。

笔记入口负责新建、为 source-only Message 添加正文和预填编辑，并打开定位根消息的半屏对话。消息输入器使用最长 8000 字符的受限 Tiptap JSON，由后端派生纯文本；可视编辑与原始 Markdown 共用同一耐久事实，Markdown 无法表示的结构会保留原内容并拒绝切换。全屏笔记页投影所有未删除根 Message，并提供章节/全文筛选、本书导出和对话入口；对话浮层负责回复、引用、修订、关系、快照和跳回。删除写入 Message 墓碑并立即撤销正文投影。标注颜色、notebook、同步和 tombstone 压缩留待后续真实需求。

### 翻页输入

`Interaction` 在左右模式把键盘、滚轮、页区点按和单指横向滑动解释为前后翻页意图，再交给 Navigation 串行执行。每个 Pointer 序列只认领一次 owner：链接、表单、弹窗、多点、非折叠选区与纵向意图保持受保护；图片、公式、表格和代码可在左右区域或明确横拖时翻页。横向溢出容器起手方向仍有空间时独占并滚动自身，已在对应边界时把整次序列交给翻页，序列中不反复换 owner。媒体中央单击仍打开预览；表格和代码中央单击仍切换工具层，既有双击或键盘预览不变。已提交的横拖同时抑制兼容 `click` 与 `dblclick`。

横拖开始时只读取一次视口几何和布局 / 显示比例；后续输入只更新内存中的最新位移，由单个 `requestAnimationFrame` 写入 transform 或横向滚动值。显示宽度超过 20,000px 的长章节使用浏览器原生 `scrollLeft`，短章节继续使用 transform；中断收束后的下一次手势从实际视觉位置起步，不回跳到逻辑整页坐标。DPR 缩放下，Pointer 的 CSS 像素位移先换算为内部布局像素。松手后的 CSS 或 rAF 收束为 300ms。在上下模式，浏览器原生纵向滚动继续拥有正文手势，键盘、滚轮或边界纵向滑动才跨 section；华为 WebView 114 的空 `pointerType` 仍归一为 touch，原生 `pan-y` 触发 `pointercancel` 时继续由既有 touch 边界链路处理。

标准离散滚轮输入逐次产生翻页意图；小幅高频输入先累计阈值，并在同一精密手势的空闲窗口结束前抑制惯性尾流。`scripts/check-reader-wheel.ps1` 用固定快速样本在真实浏览器记录书内媒体目标、4 次间隔 100ms 的离散输入接受率和事件到 Navigation 稳定的 P95；50ms 门槛只用于该快速样本，同页 benchmark 与多章节样书继续分别记录分页成本和跨章成本。

### 文本、链接与脚注

正文选择与系统复制命令保留浏览器原生行为。选择动作条的复制只在用户手势中把已保留的原生 Range 交给浏览器复制命令；应用不读取剪贴板，不持久化复制内容，也不经 IPC 或网络传输。内容边界只接受同书 XHTML 链接和无凭据 HTTP/HTTPS 外链；危险 scheme、目标窗口与下载属性仍拒绝。同书链接统一由 Navigation 按 section URL 和 fragment 跳转，fragment 定位跳过目标开头的不可见空白；未知目标安全回落。外链不在 WebView 导航或请求网络，只显示已阻止反馈。

同章 noteref 可以把目标纯文本投影到原生 dialog，关闭后焦点返回触发链接；跨章节脚注作为普通书内链接导航。脚注 HTML 不复制到应用壳，也不因此放宽脚本或资源信任边界。

### 图片与公式预览

非链接图片在完成资源校验后获得按钮语义、键盘焦点和可见焦点状态，单击、Enter 或 Space 使用应用壳现有原生 dialog 预览同一受控资源。链接包裹的图片只保留链接语义，避免一个内容节点同时触发链接和预览。

预览只设置独立图片元素的已校验本地 URL、安全标题和替代文本，不复制书源 HTML、样式或脚本。普通图片始终保留原色，公式预览沿用正文的明暗主题过滤；查看层提供 50%–400% 缩放、复位与放大后的原生双向滚动，关闭后焦点返回原图片，且不改变 section、页码或 Locator。图集、保存和 OCR 等能力在出现明确需求前不建设。

### 表格与代码预览

表格和块级预格式化内容保持原生语义、选择能力、键盘焦点和可见焦点。双击或在自身焦点上按 Enter、Space 使用现有原生 dialog 打开独立预览；链接、选择和中央单击仍优先，左右页区点按与明确横拖按统一 owner 规则翻页。宽内容在中部横拖时滚动自身，到对应边界后的新手势才移交翻页。

表格预览深克隆已经过书籍安全清洗的表格 DOM，以保留 caption、行列、跨度、公式和图片；投影再次移除链接目标、事件属性、行内样式、焦点属性与非公式 class。查看层先打开，尚未加载的公式保持同尺寸空白，并复用正文 SVG 安全校验以最多三个并发按表格顺序渐进加载；worker 等待真实 `load`、`error` 或 `abort` 终态后复用，关闭则立即取消剩余队列并阻止在途结果回写。代码预览只设置 `textContent` 并保留空白。两者关闭后恢复焦点且不改变 section、页码或 Locator；复制按钮、导出、执行和编辑留待明确需求。

### 首个验收基线

长期验收样本位于本机忽略目录 `fixtures/local/`。当前清单包含三个既有单章节样本，以及从《数学及其历史 (2026)》固定哈希源文件重复导出的“1.1 算术与几何”“1.2 勾股数组”“1.3 圆上的有理点”三章节 R1 样本；源 EPUB 不修改，也不提交样本内容到仓库。`scripts/export_reader_sample.py` 支持导出单章节或带 manifest 的多章节样本，`scripts/check-reader-samples.ps1` 统一运行实际 Windows host 与明暗主题截图验收。

阅读页填充 WebView 视口，内部画布尺寸等于视口 CSS 像素乘 `devicePixelRatio`。用户字号以 16–40 逻辑 CSS px 保存，默认 19；Pagination 写入 `字号 × devicePixelRatio` 的内部设备像素，显示层再以 `1 / devicePixelRatio` 抵消系统 DPI，因此同一档保持可解释的 CSS 绝对大小。正文边距、栏宽、公式和图形继续使用设备像素，Windows 窗口、48 CSS px 覆盖工具层和错误提示使用系统逻辑像素。窗口停止调整或用户改变排版后，Pagination 经 Navigation 队列以变化前 Locator 重排并恢复位置；短暂无文字矩形时保留已校验偏移和当前页，而非误判内容不安全。控制层显隐不改变书页、正文列或 Locator 几何。安全失败在界面显示稳定错误代码与处理阶段，但不暴露书籍路径或内容。

正式内容回归覆盖 780 × 1680、960 × 720 的 DPR 1 视口，以及 390 × 840 的 DPR 2 视口。780 × 1680 不再是产品页面的固定尺寸；benchmark 记录每次运行的真实内部设备像素尺寸，以免跨布局误判性能。阅读器的一页是有受控边距的分页内容区，不是任意滚动位置的截图：不得裁切文字行、公式或图形。上下边距固定为 144 设备像素，其中包含不会被系统缩放工具栏越过的页眉页脚安全区；左右边距默认 32，并可按书选择 24 / 32 / 48。默认字号为 19 逻辑 CSS px，行距为无单位 1.8；两者均可从阅读设置调整。

普通图片以读者可用宽高为上限等比缩放；内嵌表格固定为页宽、固定布局并裁掉超高内容，完整内容由查看层缩放和原生双向滚动，块级预格式化内容仍由受控容器限制在单页。`countCutRects()` 继续进入 ready 遥测与 verify-sample / benchmark 门，但不再让普通首开、字号、窗口重排或延迟资源完成失败；真正的资源、安全、Locator 与持续布局不稳定错误仍阻断阅读。

公式按已标记的语义类别处理。行内公式保留书源的相对宽高，以当前正文与书源基准字号的同一倍率等比缩放并对齐基线；不得强行变为同一高度。行间公式独立居中，超出可用宽度时整体缩小而不裁切。普通 SVG 和插图不套用公式规则。

阅读页跟随系统 `prefers-color-scheme`。暗色下只对 `.math-inline` 与 `.math-display` 图片应用反色；普通插图保持原色。清单显式声明样本是否应含公式以及普通图片数量，避免零内容空通过。

### M2 交付门槛

`scripts/check-reader-gate.ps1` 是 M2 的组合验收入口。它先构建当前源码并运行四困难样本，再从固定哈希的《数学及其历史》生成仅用于 fixture 的全 XHTML manifest，核对 173 个 section、固定全书搜索、三轮 host 与全部 WebView2 后代的 working set、绕过正常关闭的强杀恢复，最后运行 10 样本 benchmark。全 XHTML 模式不是 EPUB 导入器，不解析 OPF spine，也不建立 M3 书籍身份。

本机 nearest-rank P95 门槛固定为：冷启动 2000ms、首个稳定页 750ms、热打开 120ms、翻页 50ms、字号重排 150ms。进程树内存继续采样和记录，但不设置失败门槛。只有总 gate 测出瓶颈时才增加对应优化；运行结果由代码库地图和当前 change 保存，不把本机数值当作跨设备性能承诺。

`scripts/check-tauri-reader.ps1` 对产品入口运行前端检查、production build、workspace Rust 检查、Tauri debug build、真实 EPUB import probe 和相同五项性能门槛。benchmark 模式只运行性能探针，不夹带功能验收。

### M3 EPUB 入口门槛

`scripts/check-epub-source.sh --epub <path>` 是当前单格式真实输入的 Bash 导入入口。它运行锁定的 Rust 检查与 release 导入探针，私密输出只保存在仓库忽略目录；需要真实 GUI 时复用 `scripts/check-reader-linux.sh`，不复制第二套 WebDriver runner。既有 Windows WebView2 结构证据只作为历史记录，不是当前 Linux 验收入口。

EPUB2 / NCX 兼容测试由 Rust 测试代码动态生成原创最小书和恶意变体，核对 metadata、legacy cover、嵌套目录前序、DOCTYPE / 实体与路径边界；正向制品由固定 EPUBCheck 5.3.0 作为开发期规范 oracle，不进入运行时或 APK。正式 Android 证据复用同一忽略制品与 `scripts/check-android-reader.ps1 -VerifyEpub2NcxFixture`，已覆盖系统 picker、嵌套 fragment 目录跳转、强停重开和同一 section / page 恢复；该证据来自 API 35 x86_64 16 KiB 模拟器，不替代 ARM 真机性能验收。

### CBZ 入口门槛

`scripts/check-cbz-source.ps1` 从 Rust 测试 writer 生成并锁定原创 CBZ，运行 workspace Rust 检查并通过保留 Windows host 的 `--book-root --manifest --verify-import` 检查实际 WebView2。`scripts/check-android-reader.ps1 -BookPath <generated.cbz> -CleanAppData -VerifyCbzFixture` 在专用 AVD 上覆盖系统 picker、逐页到末页、坏页继续、强停恢复、隐私日志与 PSS 证据；最终目标端证据由对应 change 记录。

### FB2 / FBZ 入口门槛

`scripts/check-reader-linux.sh` 从 Rust 测试 writer 生成原创 FB2，并在仓库 `.tmp` 下种入隔离的真实 `LocalLibrary`。入口运行 Svelte 与 Tauri build，再用官方 `tauri-driver` / WebKitWebDriver 驱动当前 Linux Tauri 壳，覆盖 360 / 1000 宽度的资料库管理、存储分类和两级删除布局，以及完整导入诊断、跨 section 回退、当前手势矩阵和 AppLog 隐私。手势矩阵请求 W3C touch Pointer Actions，逐次核对真实命中、`isTrusted`、左右点按 / 横拖、媒体误开、纵向意图、宽表双向内部滚动与双向边界移交，并记录 WebKitGTK 实际返回的 `pointerType`。当前 Linux WebKitGTK 把请求的 touch 映射为 `mouse`，因此该门只算可信自动化指针与性能证据，不冒充实体触摸；PCT-AL10 自动滑动与 SurfaceFlinger presentation 另作真实目标证据。

### Kindle 入口门槛

Kindle 当前最低 Bash 检查为 `mise exec -- cargo test --locked -p atha-backend --test kindle_import`。忽略目录中的两个普通 KF8 与一个词典样本已有 importer、release benchmark 和 WebKitGTK 历史证据；继续该路线前必须把私密基准、目录 / 搜索 / 重排 / 恢复和 AppLog 隐私迁入 Bash 入口，不再执行旧 PowerShell 门。该 Linux 证据不替代 Android ARM64 真机性能。

### 离线词典入口门槛

`scripts/check-dictionary-source.sh` 运行公共边界测试、私有 MDict / MDD 与经典 Kindle 真实英文输出、release benchmark 和 workspace 回归；忽略目录中的 schema 1 清单固定至少三组英文查询及归一化纯文本释义 SHA-256，门禁只读取预期值，不能由本次结果自更新。富文本公共测试另外固定语义结构保留、活动节点删除、未知元素降级、来源属性清除与普通文本包装。Linux GUI 与 PCT 原生基准的既有结果仍是历史证据；再次验收前把对应 opt-in 场景迁入同一 Bash 入口。当前 Linux WebKitGTK 不提供闭合 Shadow DOM 的可用选区对象，因此自动化不能冒充真实选区点击；Tauri / WebView 的真实长按、直接点击、抽屉、应用 PSS 与截图仍在 PCT-AL10 单独验收。

## 性能策略

- 使用内容已知尺寸或缓存测量结果为重内容预留空间，减少重排。
- 当前视口和即将进入视口的内容优先；其他公式、SVG 和重资源可渐进完成。
- 初次打开可以执行可复用预处理并显示真实忙碌状态，后续读取优先复用结果。
- 持久缓存命中先验证发布 marker、元数据、manifest 与声明文件；缺失、空 section 或版本变化后按需重建。
- 性能优先模式可以削减部分视觉效果，但不得伪造或篡改原文内容。
- 优先保持单个阅读会话中的 WebView 与阅读页存活，并先校正真实用户阶段的计时；持久缓存、渐进解码和其他专项优化由后续功能与实测瓶颈驱动。

公式密集章节只对具有合法显式宽高的 SVG 公式启用延迟资源：同尺寸占位先参与分页，首个稳定页和每次翻页优先当前页与下一页；横向收束期间向左额外覆盖一页，避免回划目标页落在加载范围外，其余公式进入相邻视口时再加载。稳定显式尺寸图片的可见性扫描缓存书内坐标，横向翻页不重复读取每张图片的 viewport geometry；布局或样式变化时整体失效。校验通过前不设置可呈现 `src`；SVG 获取、解析和安全校验完成后，设置 `src` 的终态等待窗口为 50ms，超时则公式保持同尺寸空白并由迟到 `load` / `error` 局部收尾。该 50ms 不约束前置 SVG 校验的 wall time。固定尺寸公式成功校验、解码和显现不改变布局，也不捕获 Locator 或重排。只有失败占位替换会在首个 DOM 变化前捕获一次 Locator，随后重排并恢复；离开 live 章节即释放解码和几何状态，最多三个已校验 detached section 只保留重建 DOM 所需数据。基准已经否定普通正文图片、整页或整节等待全部资源的揭示闸门，因此空白占位只稳定单个公式资源盒。

`scripts/check-reader-formula-performance.sh` 通过忽略目录中的私密 sidecar 锁定本地 EPUB 身份和章节，不把路径、标题、正文或哈希写入输出。它复用 Bash Linux Tauri 手势门，要求章节达到固定的公式数与页数下限，再对普通与重公式页面分别执行 5 次预热、20 次逐场景测量。正式门槛为横拖首次可观察 transform / scroll 页面状态更新 P95 不超过 33.4ms、拖动期间连续主线程 rAF 间隔 P95 不超过 25ms、最大间隔不超过 50ms、松手稳定 P95 不超过 400ms，以及点按松手到首次可观察页面状态更新 P95 不超过 50ms；少于 20 次的运行只作语义冒烟，不判定 P95。该 Linux 指标不是 compositor presentation、Android FPS 或实体触摸证据。

## 位置与版本

阅读位置按书籍状态键与内容版本保存，样式变化后仍以内容 Locator 恢复到同一内容位置。EPUB / CBZ 状态键绑定源文件内容哈希缓存路径，FB2 / FBZ 绑定解包 XML 的格式域哈希缓存路径，Kindle 绑定同一格式域下的源字节哈希缓存路径，均不绑定源文件位置；内容版本变化时不猜测旧位置。

## 非责任

- 阅读页不定义书籍格式 parser；EPUB2 / EPUB3、CBZ、FB2 / FBZ 与 Kindle 导入细节分别只属于后端 `reader::epub`、`reader::cbz`、`reader::fb2` 与 `reader::kindle` module；
- 阅读内核只捕获与投影消息，不拥有 SQLite、AI 或同步协议；
- 不替用户修复损坏书源；
- 不预设跨机器性能数值，后续规格基于困难书籍样本决定。

## 相关文档

- 产品定义：`docs/product/OVERVIEW.md`
- 消息与共读：`docs/architecture/MESSAGE-READING.md`
- 数据库基线：`docs/codebase/DATABASE.md`

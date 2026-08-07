---
description: 固定 Readest 与 foliate-js 源码、事实标准和 Atha CBZ 最小切片的研究结论。
---

# CBZ 格式一手研究与最小切片

## 结论

本文用三种标签区分证据强度：

- **规范保证**：规范或平台文档明确承诺的行为；
- **源码事实**：固定版本或提交中实际存在的行为，不等于跨版本承诺；
- **建议**：面向 Atha 的产品或工程选择，需要在后续 change 中批准和验证。

结论如下。

1. **建议**：第一片只支持 ZIP 容器中的静态 JPEG 与 PNG，一张图片一个 section；不支持 GIF 动画、WebP、BMP、SVG、JXL、AVIF，也不在第一片实现 RTL、自动跨页配对或双页裁切。这样可复用现有 reader locator、资源协议和 WebView2 解码路径，避免先扩展 manifest。
2. **建议**：第一片读取 `ComicInfo.xml`，但只消费当前产品会显示或使用的 `Title`、`Writer` 和 `Pages/Page[@Type="FrontCover"]`。不为未来预建 `Series`、`Manga`、`DoublePage` 等字段；无效或缺失的元数据不应使图片本身有效的书打不开。
3. **建议**：运行时保留现有 `zip 8.6` 和 `quick-xml 0.41`，新增的唯一依赖候选是 `imagesize 0.15.0`，关闭默认 feature，仅启用 `jpeg`、`png`。它负责按文件头识别格式并读取尺寸，不负责完整解码；尾部截断、坏扫描数据等仍由 WebView 的 `HTMLImageElement.decode()` 报错。
4. **建议**：排序必须由 Atha 定义为确定性的路径分段自然排序，不能照搬 Readest fork 的字典序，也不能直接采用 foliate-js main 的 locale-dependent `Intl.Collator`。同一目录内 `2.jpg` 排在 `10.jpg` 前；ASCII 大小写不参与主比较，最终以原始 UTF-8 路径字节打破平局。
5. **建议**：安全边界沿用现有 EPUB ZIP 检查：不落盘、拒绝不安全路径、加密项、符号链接、重叠成员、重复名和大小写折叠后的歧义名；限制压缩包、成员数、页面数、单成员和总解压量，并对实际读取再次计数。图片还必须有独立的宽、高和像素数上限，不能把 ZIP 上限当作解码内存上限。
6. **规范保证**：Android 的 16 KiB page-size 要求是原生二进制装载与对齐约束，不是 WebView 图片尺寸或 JavaScript heap 上限。验收既要检查 16 KiB 安装/启动，也要分别观察应用进程和 WebView renderer 的 PSS、解码延迟、掉帧与 renderer 重启。

## 固定对照源码

### Readest v0.11.20 与其 foliate-js fork

以下结论固定到 Readest tag `v0.11.20` 的提交 `1df1505fc5033fc949463c9908f2d53bd0fbdfa6`，及该版本 submodule 指向的 `readest/foliate-js` 提交 `dd71f2be356563c16a23272686189fcfb45d0b82`；不能推断为 Readest 新版本的行为。

- **源码事实**：Readest 先用 ZIP local-header 或文件尾 EOCD 特征识别 ZIP；其 Web 路径使用 `@zip.js/zip.js` 枚举成员，并额外读取 EOCD archive comment。CBZ 由 MIME `application/vnd.comicbook+zip` 或大小写敏感的 `.cbz` 后缀进入 comic adapter。见 [`document.ts` 的 ZIP 检测](https://github.com/readest/readest/blob/1df1505fc5033fc949463c9908f2d53bd0fbdfa6/apps/readest-app/src/libs/document.ts#L156-L194)、[ZIP loader](https://github.com/readest/readest/blob/1df1505fc5033fc949463c9908f2d53bd0fbdfa6/apps/readest-app/src/libs/document.ts#L204-L321) 和 [CBZ dispatch](https://github.com/readest/readest/blob/1df1505fc5033fc949463c9908f2d53bd0fbdfa6/apps/readest-app/src/libs/document.ts#L324-L419)。
- **源码事实**：fork 的 `comic-book.js` 接受 `.jpg`、`.jpeg`、`.png`、`.gif`、`.bmp`、`.webp`、`.svg`、`.jxl`、`.avif`，但扩展名比较区分大小写；它对名称直接调用 JavaScript `.sort()`，没有数字自然排序。每个图片成员变成一个 object URL 和一份只含 `<img>` 的 HTML section，封面默认第一张，rendition 标为 `pre-paginated`。见 [固定 fork 的完整 adapter](https://github.com/readest/foliate-js/blob/dd71f2be356563c16a23272686189fcfb45d0b82/comic-book.js#L1-L140)。
- **源码事实**：fork 优先解析根目录精确匹配、否则第一个 basename 匹配的 `ComicInfo.xml`；另从 ZIP comment 解析 `ComicBookInfo/1.0` JSON。它只将一组文本元数据映射到书籍 metadata，没有读取 `Manga`、`Pages`、封面声明或 `DoublePage`。同一源码没有图片魔数校验、尺寸/像素上限、隐藏项规则或坏图片恢复策略。
- **源码事实**：固定布局渲染器按 `book.dir === 'rtl'` 决定方向，并在缺少 `pageSpread` 时自动配对；CBZ adapter 没有设置 `dir` 或 section 的 `pageSpread`，因此这些行为不是 ComicInfo 驱动的。渲染器默认只预载有限相邻 spread，但这是该渲染器的缓存策略，不是 CBZ 安全边界。见 [`fixed-layout.js` 的 spread 选择](https://github.com/readest/foliate-js/blob/dd71f2be356563c16a23272686189fcfb45d0b82/fixed-layout.js#L1100-L1148) 和 [加载、预载逻辑](https://github.com/readest/foliate-js/blob/dd71f2be356563c16a23272686189fcfb45d0b82/fixed-layout.js#L1214-L1343)。

因此，Readest 是有价值的互操作性样本，却不是可以照抄的信任边界：其格式列表宽于 Atha 第一片，排序和 ComicInfo 映射也没有覆盖本任务要求的确定性、安全与封面语义。

### foliate-js main

以下结论固定到 `johnfactotum/foliate-js` main 的提交 `78914aef4466eb960965702401634c2cb348e9b1`。

- **源码事实**：main 的 CBZ adapter 只有图片枚举、object URL、单图 section、第一张封面和固定布局声明；没有 ComicInfo 或 ComicBookInfo 元数据解析。见 [main `comic-book.js`](https://github.com/johnfactotum/foliate-js/blob/78914aef4466eb960965702401634c2cb348e9b1/comic-book.js#L1-L45)。
- **源码事实**：main 使用 `new Intl.Collator([], { numeric: true })` 排序。数字排序优于 Readest fork 的纯字典序，但空 locale 列表仍依赖运行环境默认 locale；它不是跨 Windows/Android、跨用户设置的稳定序列。
- **源码事实**：main 的 view 入口只接受 local-header `PK\x03\x04` 特征，并以区分大小写的 `.cbz` 分派；ZIP 仍在 JavaScript 中交给 zip.js，且禁用 worker。见 [main `view.js`](https://github.com/johnfactotum/foliate-js/blob/78914aef4466eb960965702401634c2cb348e9b1/view.js#L1-L123)。

## CBZ、ZIP 与元数据的标准地位

### 容器

- **规范保证**：ZIP 的结构、成员、压缩方法、central directory 和 archive comment 由 PKWARE APPNOTE 定义；PKWARE 提供当前规范与历史固定版本。见 [PKWARE APPNOTE](https://support.pkware.com/pkzip/appnote) 和 [Application Note Archives](https://support.pkware.com/pkzip/application-note-archives)。
- **事实标准**：CBZ 没有类似 ISO、IETF、W3C 或 PKWARE 的单一官方规范。实践中的核心约定只是“ZIP 中放按阅读顺序排列的页面图片，并使用 `.cbz` 扩展名”。因此，图片格式、排序、隐藏文件、目录层级、元数据、RTL 与双页语义都必须由阅读器声明兼容策略；不能把某个阅读器的行为写成官方保证。

### ComicInfo.xml

- **事实标准**：ComicInfo.xml 源自已停止维护的 ComicRack，后来由 Anansi 项目整理 schema 和文档；Anansi 自己也把这段历史和此前缺少治理写入说明。它是广泛互操作的社区 schema，不是官方 CBZ 标准。见 [Anansi ComicInfo 项目](https://github.com/anansi-project/comicinfo) 与 [格式沿革](https://anansi-project.github.io/docs/comicinfo/intro)。
- **事实标准**：稳定的 [ComicInfo v2.0 schema](https://anansi-project.github.io/docs/comicinfo/schemas/v2.0) 定义了 `Title`、`Writer`、`Pages/Page` 等字段；字段说明中 `Page` 可带 `Image`、`Type=FrontCover`、`DoublePage`，`Manga=YesAndRightToLeft` 表示 RTL。见 [ComicInfo 字段文档](https://anansi-project.github.io/docs/comicinfo/documentation)。v2.1 仍标为 draft，不应成为第一片的实现基线。
- **建议**：只在根目录寻找大小写精确的 `ComicInfo.xml`；若不存在，可接受唯一一个嵌套 basename 大小写精确匹配。存在多个候选时拒绝元数据而不是任取一个。XML 解析失败、超限或字段无效时忽略元数据并继续打开有效图片。
- **建议**：第一片只消费 `Title`、`Writer` 和 `Pages/Page` 中唯一、索引有效的 `FrontCover`。`Image` 按 Atha 明确记录的零基页面索引解释；越界、重复 FrontCover 或指向非页面时回退到排序后的第一张。此处是 Atha 兼容策略，不声称 schema 对所有历史生成器保证同一索引约定。

### ComicBookInfo/1.0

- **事实标准**：ComicBookInfo 是历史项目提出的 archive-comment JSON 约定，原项目只剩 [Google Code archive](https://code.google.com/archive/p/comicbookinfo/)；Readest fork 实际从 ZIP EOCD comment 读取 `ComicBookInfo/1.0`。它同样不是官方 CBZ 标准。
- **建议**：第一片不消费 ComicBookInfo。原因不是无法读取——`zip 8.6` 已暴露 archive comment——而是当前产品只需要极小 metadata，ComicInfo 的文件式 schema 更可测试，双格式合并会立刻引入优先级和冲突规则。实际语料显示仅有 ComicBookInfo 且标题缺失时，再用一份 change 加入只读 fallback。

## Atha 可复用边界与依赖

### 现有能力

- **源码事实**：Atha 当前锁定 `zip 8.6.0` 和 `quick-xml 0.41.0`。现有 EPUB archive 检查已覆盖空包、超过 10,000 个成员、重叠数据、加密项、符号链接、总解压量、单成员、Windows 不安全路径与大小写折叠重复名，并在真实读取时用 `take(limit + 1)` 再限流。见 `backend/atha-backend/src/reader/epub/archive.rs` 与 `backend/atha-backend/src/reader/epub/mod.rs`。
- **规范/库保证**：`zip 8.6.0` 基于 PKWARE APPNOTE 6.3.9；`ZipFile` 提供 `size`、`compressed_size`、`enclosed_name`、`encrypted`、`is_symlink`，并明确警告原始 `name()` 不宜直接用于文件系统路径；`ZipArchive` 提供 `comment` 和 `has_overlapping_files`。见 [`zip 8.6.0` crate 页面](https://docs.rs/crate/zip/8.6.0)、[`ZipFile`](https://docs.rs/zip/8.6.0/zip/read/struct.ZipFile.html) 与 [`ZipArchive`](https://docs.rs/zip/8.6.0/zip/read/struct.ZipArchive.html)。
- **源码事实**：Atha 当前使用 quick-xml 的 event parser，不需要为三个 ComicInfo 字段引入另一个 XML 栈。见 [`quick-xml 0.41.0` API](https://docs.rs/quick-xml/0.41.0/quick_xml/)。
- **源码事实**：当前 ReaderManifest 只含 sections、resources 和 toc 等字段，没有阅读方向或固定布局/跨页字段；图片资源协议已支持 PNG、JPEG、GIF、WebP，并发送 `nosniff`。纯图片页仍可使用现有 section + offset 0 locator。

### 唯一新增依赖：imagesize

- **库事实**：[`imagesize 0.15.0` 的发布清单](https://docs.rs/crate/imagesize/0.15.0/source/Cargo.toml) 声明 MIT 许可证、无运行时依赖、各格式独立 feature，并将用途限定为“不加载整个文件而快速探测尺寸”。`image_type`、`reader_type` 和 `reader_size` 可从内容识别类型并读取尺寸；它不是完整图片解码器。见 [`ImageType` API](https://docs.rs/imagesize/0.15.0/imagesize/enum.ImageType.html)。
- **建议**：添加 `imagesize = { version = "0.15", default-features = false, features = ["jpeg", "png"] }`，并在锁文件固定解析结果。它比自写 PNG/JPEG header parser 更短、更易审计，也远小于引入完整 `image` 解码栈。
- **建议**：扩展名只用于筛选候选；`.jpg`/`.jpeg` 必须被识别为 JPEG，`.png` 必须被识别为 PNG。扩展名与魔数不符时拒绝该页面，不根据用户可控后缀生成 MIME。资源响应使用探测后的 `image/jpeg` 或 `image/png`，继续发送 `nosniff`。
- **边界**：尺寸探测成功不保证熵编码、尾部或完整像素流有效。浏览器创建 `<img>` 后必须等待 `decode()`；失败显示稳定的“图片损坏”页并允许继续导航，不能无限重试、崩溃 renderer 或把空白当成功。

除 `imagesize` 外，第一片不新增依赖：不用 zip.js，不加自然排序 crate，不加第二个 XML parser，不加 Rust 图片完整解码器，也不建立多格式 archive 抽象。可以把 EPUB 中与格式无关的少量 ZIP 检查下沉为共享函数；若为共享而需要引入 trait/factory，则在 CBZ 模块内复用同一套常量和直线逻辑更小。

## 文件选择、排序与显示语义

### 成员选择

**建议**规则按以下顺序执行：

1. 先对所有 ZIP 成员执行安全检查和总量计数，不能因成员稍后会被忽略就绕过 bomb、重叠或路径检查。
2. 目录成员不成为页面。路径任一 segment 以 `.` 开头、位于 `__MACOSX`，或 basename 以 `._` 开头的成员视为隐藏/平台垃圾，不成为页面。
3. 只有大小写不敏感后缀 `.jpg`、`.jpeg`、`.png` 是页面候选；其他普通文件忽略，但仍计入成员数和解压声明总量。有效候选为零则报告“不含受支持页面”。
4. 候选的头部类型必须与后缀一致；零尺寸、超尺寸、超像素或头部无效的候选拒绝。不要把 SVG 当图片接入 HTML，因为它扩大主动内容与 XML 安全边界。

### 确定性自然排序

**建议**不要依赖宿主 locale：

1. 以 `/` 分隔路径 segment，逐 segment 比较；目录名也参与顺序。
2. 每个 segment 切成连续数字与非数字 token。非数字 token 仅做 ASCII lowercase 后按 UTF-8 字节比较；数字 token 去除前导零后按有效位数、再按数字字节比较。
3. 主键相同时，以数字 token 的前导零数、原始 segment、最后完整原始路径 UTF-8 字节作稳定 tie-break。这样 `1.jpg < 02.jpg < 2.jpg` 的具体细节由测试锁定，而不是由排序稳定性偶然决定。
4. 在排序前拒绝完全重复的 ZIP 名和 ASCII case-fold 后相同的名字；否则不同平台可能选中不同实体。

Unicode 正规化和“卷 甲/卷 乙”的语言学排序不进入第一片。若真实语料证明需要，它应以明确 corpus 和期望序列驱动，而不是切换到默认 locale。

### 封面、方向和双页

- **建议，第一片**：有效的唯一 `FrontCover` 指向页面时使用它；否则排序第一张既是封面也是第 0 页。封面声明不改变正文页面的顺序，也不复制或移除该页。
- **已知延期**：第一片按 LTR、单页 section 渲染，不读取 `Manga`，不应用 `DoublePage`，不做相邻图片自动配对。这不是格式保证，而是因当前 manifest 没有方向/跨页语义而设置的产品边界。
- **停止条件**：若验收语料中 RTL 或双页是目标用户的高频必要内容，则停止“只接 archive adapter”的实现，把 `readingDirection` 与 page-spread 语义作为新的 accepted change，贯穿 manifest、渲染、定位和 Windows/Android 验收后再发布；不要只在 CSS 中猜测。

## 不可信输入边界

下表全部是 **Atha 建议值**，不是 ZIP 或 CBZ 规范保证；沿用值来自当前 EPUB 防线，图片值必须用真实设备校准。

| 风险 | 第一片策略 | 初始上限/失败行为 |
| --- | --- | --- |
| ZIP bomb | central directory 声明值先检查，实际读取再次计数；不依赖压缩比启发式 | 源文件 512 MiB、总解压 512 MiB、单成员 16 MiB；任一超限拒绝整书 |
| 路径穿越/绝对路径 | 使用 `enclosed_name` 思路并沿用 Windows 路径校验；永不解压到文件系统 | `..`、根路径、盘符、UNC、反斜线歧义、保留设备名拒绝整书 |
| 加密、符号链接、成员重叠 | 复用现有 EPUB 检查 | 任一出现即拒绝整书 |
| 重复名 | 原始名及 ASCII case-fold 名都建唯一索引 | 任一歧义即拒绝整书，不使用 first/last wins |
| 过多成员/页面 | 所有成员先计数；页面另设 manifest 可承载上限 | 成员不超过 10,000，图片页面不超过 1,000 |
| 元数据膨胀/XML 滥用 | `ComicInfo.xml` 单独读取上限，quick-xml 流式解析，只保留三个字段；禁止外部资源语义 | 建议 1 MiB、最大嵌套 64；超限或 malformed 时仅忽略元数据 |
| 扩展名伪装 | `imagesize` 探测类型并与后缀核对 | 不符时该页不可用；若无有效页面则拒绝整书 |
| 像素炸弹 | 解码前校验宽、高、`width * height` 的 checked arithmetic | 建议单边不超过 8,192、单页不超过 20 MP；超限页拒绝，阈值以 Android 实测调整 |
| 尾部损坏/浏览器不支持 | 等待 `HTMLImageElement.decode()`，捕获 `EncodingError` | 显示可导航的损坏页；记录一次诊断，不重试风暴 |

绝对解压预算比单独的“压缩比 ≤ N”更可证明：无论压缩比多高，Atha 都不会读取超过预算的输出。可以记录 `uncompressed/compressed` 比率用于测试与遥测，但第一片不凭任意比率误拒绝高度可压缩的合法图片。

像素预算按最坏情况至少约 `width × height × 4` 字节估算，仅是单张 RGBA surface；浏览器还可能持有压缩数据、缩放 surface、GPU 纹理和预载页。因此，不允许一次把整本书转换成 data URL/object URL 并常驻；只为当前页与至多一个相邻页提供资源，离开窗口后撤销引用，让 WebView 有机会回收。

## 浏览器解码、lazy loading 与 Android 16 KiB

- **规范保证**：`HTMLImageElement.decode()` 返回 Promise；图片可解码时 fulfilled，解码错误时以 `EncodingError` rejected。见 [WHATWG HTML 图片解码算法](https://html.spec.whatwg.org/multipage/embedded-content.html#dom-img-decode-dev)。这适合作为页面“可显示”的最终信号。
- **规范保证**：`loading="lazy"` 是用户代理决定何时推迟加载的提示，不承诺固定距离、并发数或内存上限。见 [WHATWG lazy-loading attributes](https://html.spec.whatwg.org/multipage/urls-and-fetching.html#lazy-loading-attributes)。Atha 的分页模式应靠 section 生命周期和缓存窗口控内存；只有未来连续滚动模式才把原生 lazy loading 当附加优化。
- **规范保证**：Android 15 起支持 16 KiB page-size 设备；官方指南要求检查原生 shared library 对齐，并说明 16 KiB 模式可能有额外内存影响。见 [Android 16 KB page-size 指南](https://developer.android.com/guide/practices/page-sizes)。它不改变 JPEG/PNG 的像素解码公式，也不提供 WebView heap 配额。
- **平台事实**：启用 multiprocess 的 WebView renderer 是独立 sandbox 进程；`WebView.getWebViewRenderProcess()` 可取得关联。见 [Android WebView API](https://developer.android.com/reference/android/webkit/WebView.html#getWebViewRenderProcess())。只量应用主进程会漏掉大部分图片解码压力。

**建议的测量方法**：

1. 记录设备型号、Android/WebView 版本、`adb shell getconf PAGE_SIZE`、构建类型和 fixture hash；16 KiB 验收必须得到 `16384`。
2. 在 reader 中对“ZIP 打开完成”“首图 header 完成”“`img.decode()` 完成”“首帧可见”和相邻页切换加 `performance.mark/measure`，汇总冷启动与热翻页 p50/p95。
3. 用 `adb shell dumpsys meminfo <package-or-pid>` 分别记录应用与 renderer 的 PSS Total、Private Dirty；官方 `dumpsys` 文档解释了这些指标。见 [`dumpsys` 指南](https://developer.android.com/tools/dumpsys) 与 [Android 内存概览](https://developer.android.com/topic/performance/memory-overview)。
4. 用 `adb shell dumpsys gfxinfo <package> framestats` 和页面切换时间线观察 jank；在同一设备上比较空 reader、普通 fixture、上限附近 fixture，而不是跨设备比较绝对数字。
5. 连续往返全书至少三轮并回到第一页；PSS 应进入平台相关的稳定区间，不应每轮近似线性增长。记录 renderer 重启、OOM/LMK、ANR、decode rejection 和最长帧。

## Ponytail 最小切片

### 要做

1. 新增一个 CBZ archive adapter，复用现有文件选取/导入、`zip 8.6` 检查、reader resource protocol、section locator 和 WebView2。
2. 用简单结构 `CbzPage { archive_name, mime, width, height }` 表示排序后的页面；manifest 中每页一个 section，封面只保存页面索引/资源引用。
3. 用 quick-xml 流式读取可选 ComicInfo 的 `Title`、`Writer`、FrontCover；解析失败回退。
4. 用 `imagesize 0.15` 的 JPEG/PNG features 做内容类型和尺寸闸门，用浏览器 `decode()` 做完整解码结果。
5. 只维护“当前页 + 至多一个邻页”的读取/解码窗口，并提供损坏页 UI。

### 不做

- 不复制 Readest 的 zip.js 路径或完整 foliate fixed-layout engine；
- 不建立通用 `BookArchive<T>`、codec registry、metadata plugin 或自定义 MIME sniff 框架；
- 不支持 RAR/7z/PDF，也不因扩展名存在就承诺 Readest 的九种图片格式；
- 不缓存整本解压数据，不提取到磁盘，不做缩略图数据库；
- 不预建 Series、Tag、Web、RTL、DoublePage 等尚未消费的模型字段。

## 原创动态 fixture

测试在运行时用 `zip::ZipWriter` 生成，不提交第三方漫画或大二进制。图片使用项目自制的小型 JPEG/PNG byte fixture，像素内编码醒目的页码、色块和左右标记；大尺寸/超量案例通过改写 header 或重复小成员构造，不真的分配上限体积。

最小 fixture 矩阵：

- `happy-natural.cbz`：`001.jpg`、`2.png`、`10.JPG`、`chapter 2/1.png`，含目录、`.DS_Store`、`__MACOSX/._2.png` 和 `notes.txt`，锁定筛选及自然顺序；
- `comicinfo-cover.cbz`：根 ComicInfo 只含 Title、Writer 与非首张 FrontCover，验证 metadata、封面和正文顺序互不污染；另生成嵌套唯一、根优先、多个嵌套、malformed、深度/大小超限变体；
- `names.cbz`：完全重复、大小写歧义、`../`、绝对路径、盘符、UNC、反斜线、Windows 设备名、目录伪装、符号链接和加密标志；
- `zip-limits.cbz`：空包、截断 central directory、重叠成员、10,001 项、1,001 页、声明/实际单项及总量越界、高压缩零数据；必要时只在测试生成后定点改写 ZIP header；
- `image-gates.cbz`：后缀与魔数不符、零尺寸、乘法溢出、8,193 单边、超过 20 MP、有效 header 但尾部截断、浏览器拒绝的坏 JPEG/PNG；
- `memory-walk.cbz`：至少 30 张自制的不同噪声/渐变页，包含接近像素预算的单页，用同一份确定性 seed 在 Windows 与 Android 生成并记录 SHA-256。

每个失败 fixture 应断言稳定错误类别，而不是依赖底层 crate 的英文错误串。排序、封面和筛选测试断言最终 manifest 的资源/section 顺序；坏尾部必须进入真实 WebView `decode()` 验收，因为 Rust header probe 无法证明完整解码。

## Windows 与 Android 验收

### Windows WebView2

- **本地/真实目标证据要求**：用正式桌面入口打开 happy、ComicInfo、限制边界和损坏图片 fixture；验证文件选择器识别 `.cbz`/`.CBZ`、首屏、封面、页序、前后翻页、关闭后再开、错误页与继续导航。
- DevTools 或正式测试钩子中确认图片响应 MIME 来自探测结果且带 `nosniff`，没有 `file:` 提取路径、控制台未处理 rejection、无限重试或整书资源同时常驻。
- 在同一台 Windows 主机重复三轮 memory-walk，记录 WebView2 renderer working set/commit、首图与翻页 p95；比较冷/热路径并确认释放后形成平台相关的稳定区间。

### Android 16 KiB + WebView

- **真实目标证据要求**：在 `PAGE_SIZE=16384` 的 emulator 或实机安装 release-equivalent APK，打开与 Windows 同 hash 的 fixture；完成首屏、翻页、后台/恢复、旋转或 viewport 改变、关闭再开以及坏图片恢复。
- 同时记录应用和 renderer PSS、`gfxinfo framestats`、decode p50/p95、峰值与三轮后的平台期；检查 logcat 是否有 renderer gone、LMK/OOM、ANR、native alignment 或资源协议错误。
- 另在项目最低支持的 4 KiB 设备回归一次，避免只修 16 KiB 构建而破坏现有 ABI。

### 停止条件

出现以下任一项就停止扩格式或预载优化，先收敛边界：

1. 任何 fixture 可绕过实际读取、成员、像素或路径上限；任何重复名结果随平台变化。
2. Android 或 Windows 出现 renderer 崩溃/重启、OOM/LMK、ANR，或三轮遍历 PSS 持续近似线性上升。
3. 上限附近页面使相邻页预载无法维持稳定内存；先降为只保留当前页，再依据同设备测量调整 20 MP/8,192 阈值，不先引入缩放/转码栈。
4. 正版目标语料中大量页面落在 JPEG/PNG 之外，或大量书依赖 RTL/DoublePage/ComicBookInfo 才能正确阅读；以 corpus 统计开启下一份 change，不在首片顺手扩张。
5. 16 KiB release-equivalent APK 未通过对齐检查、无法安装/启动，或只测了主进程未测 renderer；不得把 Windows 或 4 KiB 结果代称 Android 16 KiB 验收。

## 决策摘要

| 主题 | 采用 | 延期/拒绝 | 证据性质 |
| --- | --- | --- | --- |
| 图片 | JPEG、PNG；扩展名 + `imagesize` 魔数/尺寸 + WebView 完整 decode | GIF/WebP/BMP/SVG/JXL/AVIF | 建议；浏览器 decode 行为为规范保证 |
| ZIP | Rust `zip 8.6`，复用 EPUB 防线，不落盘 | zip.js、解压目录 | 建议；API 为库事实 |
| XML | `quick-xml 0.41` event parser | 新 XML 库 | 建议与源码事实 |
| 元数据 | ComicInfo Title、Writer、FrontCover | ComicBookInfo、Series、Manga、DoublePage 等 | 建议；schema 为事实标准 |
| 排序 | 自定义确定性路径分段 ASCII 数字自然序 | `.sort()`、默认 locale Collator、排序依赖 | 建议；两上游行为为源码事实 |
| 依赖 | 唯一新增 `imagesize 0.15`，仅 jpeg/png features | 完整 Rust decoder、codec registry | 建议；crate 清单为库事实 |
| 显示 | 一图一 section、LTR、当前 + 至多一邻页 | 自动 spread、整书预解码 | 建议；当前 manifest 为源码事实 |

这是一份实施前研究，不是已完成的 Windows、Android 或生产等价验收。所有阈值均需在后续 accepted change 中落地为常量、动态 fixture 和真实目标证据后，才能升级为项目契约。

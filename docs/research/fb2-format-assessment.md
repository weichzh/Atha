---
description: 固定 FB2 权威 schema、Readest 与 foliate-js 源码，并确定 Atha Android FB2 最小切片。
---

# FB2 格式一手研究与 Android 最小切片

## 结论

本文用三种标签区分证据强度：

- **规范保证**：格式所有者的 schema 或平台规范明确给出的行为；
- **源码事实**：固定版本、固定提交或 Atha 当前代码实际存在的行为，不外推到其他版本；
- **建议**：后续 change 需要批准、实现和验证的 Atha 选择。

结论如下。

1. **规范保证**：FB2 2.0 本体是一份 XML 文档，不是 ZIP 容器。根元素是命名空间 `http://www.gribuser.ru/xml/fictionbook/2.0` 下的 `FictionBook`；正文、附加 notes body、base64 `binary`、任意 stylesheet 和 XLink 都位于同一 XML 中。[格式所有者的固定 schema](https://github.com/gribuser/fb2/blob/4d3740e319039911c30d291abb0c8b26ec99703b/FictionBook.xsd#L31-L230)
2. **建议**：第一片只承诺普通 `.fb2`。Readest 另外识别 `.fb.zip`、`.fb2.zip` 和 `.fbz`，但这些是兼容容器，不是 FB2 schema；本机没有对应正向样本，不把用户的 `.fb2` 重新打包成派生 fixture，也不按 ZIP 首成员猜书。用户放入真实 FBZ 后，再复用现有 ZIP 信任边界增加一个小兼容片。
3. **建议**：直接用 Atha 已锁定的 `quick-xml 0.41.0` event reader，把 FB2 投影为现有 `ReaderManifest` / `BookRoot` / `Locator`；启用其 `encoding` feature，并使用 `DecodingReader` 处理 BOM、XML 声明、UTF-16 和 Windows-1251 等编码。不要先转成临时 EPUB，不引入第二套 reader、format factory 或 trait hierarchy。
4. **建议**：不用 `fb2 0.4.4` 作为运行时模型。它是宽松的 serde model，内部仍依赖旧的 `quick-xml 0.30`，并把正文树和每个 base64 binary 都保存在 `String` / `Vec` 中；这会复制 XML 栈并放大 Android 峰值内存。它的容错清单可作为测试线索，不是 Atha 的信任边界。
5. **建议**：第一 body 的前置标题/题记生成一个可选前言 section，每个顶层 `section` 生成一个 ReaderManifest section，嵌套 section 保留在同一 XHTML 中；所有 section title 按文档顺序压平成现有平面 TOC。附加 body 生成无 TOC 的可链接 section，不扩展格式专属 locator 或 manifest schema。
6. **建议**：只把已知 FB2 结构转换成受控 XHTML。保留段落、标题、强调、删除线、上下标、代码、诗歌、题记、引文和表格语义；`style name` 只保留为有界语义标记。第一片忽略 FB2 内嵌 stylesheet，统一使用 Atha 固定样式和后续用户 CSS 覆盖，绝不直接执行任意 CSS。
7. **建议**：`#id` 链接重写为已验证的 BookRoot 内部锚点；绝对 `http(s)` 链接可以保留可见 label 和安全 URL，继续由现有 reader 显示“外部链接已阻止”；其他 scheme、相对路径、重复 ID 和失效引用不获得活动能力。FB2 不能读取网络、本机路径、XInclude、DTD 或外部实体。
8. **建议**：base64 binary 只解码正文或封面实际引用的 JPEG / PNG，复用现有 `imagesize 0.15.0` 魔数、单边 8,192 和 20 MP 闸门。输出精确 `width` / `height`，再复用 `content.mjs` 已有的可见区图片延迟加载队列；不要让当前 `renderCached()` 在首稳前对整章所有图片并发 `decode()`。
9. **本地证据**：匿名样本只有一个正文 section，却有 249 张有效 JPEG / PNG、合计约 1.49 亿像素。按最坏 RGBA 估算，仅像素面就约 569 MiB；因此“首稳前未解码所有 249 张图”是 Android 功能门，而不是可选优化。先复用现有延迟加载；只有该路径仍无法让 PSS 形成平台期，才按块边界技术分段。
10. **建议**：固定 API 35 x86_64 16 KiB AVD 完成系统 picker、导入、首稳、图片翻页、链接阻止、搜索和强停恢复，并取 10 次 median / P95 与 app / renderer PSS。AVD 只证明目标端功能和同环境回归；至少一台 ARM64 真机用同一 release-like build 完成相同测量后，才能把 FB2 Android 性能标为完成。

## 固定一手来源

### FB2 2.0 schema

本报告把格式所有者 Dmitry Gribov 的 `gribuser/fb2` 仓库固定在提交 [`4d3740e3`](https://github.com/gribuser/fb2/commit/4d3740e319039911c30d291abb0c8b26ec99703b)。仓库包含 FB2、genre、language、XLink 和 notes schema，并以 BSD-2-Clause 发布；本报告只引用 schema，不把它 vendoring 到产品。[仓库与许可证](https://github.com/gribuser/fb2/tree/4d3740e319039911c30d291abb0c8b26ec99703b)

- **规范保证**：第一 `body` 是默认主流程，其他 body 用于脚注等附加信息并通过链接访问；body 由可选 image、title、epigraph 和任意多个 section 组成。[`bodyType` 与 notes body](https://github.com/gribuser/fb2/blob/4d3740e319039911c30d291abb0c8b26ec99703b/FictionBook.xsd#L31-L70)
- **规范保证**：根元素依次容纳任意 stylesheet、description、主 body、可选 notes body 和任意多个 binary；每个 binary 是带必填 `id` 与 `content-type` 的 `xs:base64Binary`。[根结构与 binary](https://github.com/gribuser/fb2/blob/4d3740e319039911c30d291abb0c8b26ec99703b/FictionBook.xsd#L71-L230)
- **规范保证**：section 可以嵌套，或包含 paragraph、image、poem、subtitle、cite、empty-line 和 table；内联标记还包括 strong、emphasis、named style、link、strikethrough、sub、sup、code 和 image。[section 与内联结构](https://github.com/gribuser/fb2/blob/4d3740e319039911c30d291abb0c8b26ec99703b/FictionBook.xsd#L273-L520)
- **规范保证**：书籍元数据位于 `title-info`，包括 genre、author、book-title、annotation、coverpage、lang、translator 和 sequence。[`title-infoType`](https://github.com/gribuser/fb2/blob/4d3740e319039911c30d291abb0c8b26ec99703b/FictionBook.xsd#L570-L647)

schema 本身允许“任意 stylesheet”，也允许通用 XLink；这只是数据模型，不等于浏览器可以安全执行 stylesheet 或导航。Atha 仍必须在导入时收窄能力。

### Readest v0.11.20 与固定 foliate-js

对照基线固定为 Readest tag `v0.11.20` 的提交 [`1df1505f`](https://github.com/readest/readest/commit/1df1505fc5033fc949463c9908f2d53bd0fbdfa6)，及其 submodule 指向的 `readest/foliate-js` 提交 [`dd71f2be`](https://github.com/readest/foliate-js/commit/dd71f2be356563c16a23272686189fcfb45d0b82)。以下都是源码事实，不能外推到更新版本。

- Readest 把 MIME `application/x-fictionbook+xml` 或 `.fb2` 当普通 FB2；把 `.fb.zip`、`.fb2.zip`、`.fbz` 当 FBZ。FBZ 路径优先取第一个后缀为 `.fb2` 的成员，否则回退到 ZIP 第一个成员，再交给同一个 `makeFB2()`。[格式识别与 dispatch](https://github.com/readest/readest/blob/1df1505fc5033fc949463c9908f2d53bd0fbdfa6/apps/readest-app/src/libs/document.ts#L324-L443)
- 固定 fork 会先把整个 Blob 读成 `ArrayBuffer` 和字符串，再由 `DOMParser` 建完整 XML DOM；若 XML 声明不是 UTF-8，会再用声明编码解码整份 buffer。[`parseXML`](https://github.com/readest/foliate-js/blob/dd71f2be356563c16a23272686189fcfb45d0b82/fb2.js#L142-L156)
- adapter 建立所有 binary 的 DOM map；图片被转成含完整 base64 的 data URL。它复制结构 ID，递归转换一组白名单元素，却把 XLink `href` 原样复制到 XHTML。[converter](https://github.com/readest/foliate-js/blob/dd71f2be356563c16a23272686189fcfb45d0b82/fb2.js#L74-L140)
- 第一 body 的每个顶层转换结果成为一个 section；嵌套标题成为子目录。附加 body 每个只生成一个 `linear: no` section，所有内容先序列化为 Blob / object URL；书籍关闭时统一 revoke。[section、TOC 与链接解析](https://github.com/readest/foliate-js/blob/dd71f2be356563c16a23272686189fcfb45d0b82/fb2.js#L217-L355)

Readest 证明了“FB2 结构映射到统一 section / TOC / href 模型”可行，但其整书 DOM、data URL 和 object URL 路径不适合 Atha 的 Android 内存与 BookRoot 信任边界。Atha 只学习格式映射，不迁移 foliate Book 对象或 JS 解析器。

## 本地样本匿名预检

只读预检扫描了 Git 忽略的 `fixtures/local`，输出仅保留格式、字节数、编码和结构计数；没有记录或输出书名、作者、路径、正文、URL、哈希或任何派生文件。

| 指标 | 匿名聚合 |
| --- | ---: |
| 普通 FB2 / FBZ 样本 | 1 / 0 |
| 源文件字节 | 12,476,541 |
| XML 编码 / 命名空间 | UTF-8 / FB2 2.0 命名空间有效 |
| body / section / 最大 section 深度 | 1 / 1 / 1 |
| paragraph / title / stylesheet | 11,290 / 0 / 0 |
| binary / image | 249 / 249 |
| binary 类型 | JPEG 159；PNG 90 |
| binary 解码后总字节 / 单项最大字节 | 8,730,746 / 299,693 |
| binary base64 总字符 / 单项最大字符 | 11,801,748 / 405,141 |
| 非 binary XML 文本字符 | 506,048 |
| 图片总像素 / 单图最大像素 | 149,136,347 / 17,547,400 |
| 图片最大宽 / 高 | 6,749 / 2,600 |
| 超出现有 8,192 单边或 20 MP 闸门 | 0 |
| link / `#fragment` / `http(s)` | 764 / 0 / 764 |

这份样本直接覆盖 UTF-8、长文本、JPEG / PNG base64、图片尺寸探测和大量外部链接阻止，但不覆盖多 body、嵌套目录、脚注跳转、source stylesheet、非 UTF-8 或 FBZ。不能由一个样本外推完整互操作性；也不能用新造正向书籍掩盖缺口。

## 成熟库与许可证

| 候选 | 一手事实 | 许可证 | Atha 决策 |
| --- | --- | --- | --- |
| `quick-xml 0.41.0` | 已是 backend 直接依赖；`Reader` / `NsReader` 流式产生 event。`encoding` feature 使用 `encoding_rs`；`DecodingReader` 从 BOM / XML 声明检测并转成 UTF-8。[Reader](https://docs.rs/quick-xml/0.41.0/quick_xml/reader/struct.Reader.html) · [DecodingReader 源码与示例](https://docs.rs/crate/quick-xml/0.41.0/source/src/encoding.rs) · [发布清单](https://github.com/tafia/quick-xml/blob/v0.41.0/Cargo.toml) | MIT | **采用**；启用 `encoding`，不再加 XML parser |
| `base64 0.22.1` | 已在当前 Cargo.lock；提供严格内存解码和 `DecoderReader` 流式接口。[DecoderReader](https://docs.rs/base64/0.22.1/base64/read/struct.DecoderReader.html) · [发布清单](https://github.com/marshallpierce/rust-base64/blob/v0.22.1/Cargo.toml) | MIT OR Apache-2.0 | **采用为直接依赖**；只解码引用资源，不自写 base64 codec |
| `imagesize 0.15.0` | 已直接依赖并只启用 JPEG / PNG；现有 CBZ 路径用它校验魔数、宽高和像素预算。它不做完整像素解码。[`ImageType`](https://docs.rs/imagesize/0.15.0/imagesize/enum.ImageType.html) · [`blob_size`](https://docs.rs/imagesize/0.15.0/imagesize/fn.blob_size.html) | MIT | **复用**；浏览器 `decode()` 仍是最终可显示信号 |
| `fb2 0.4.4` | 上游把自己描述为可让 quick-xml serde 容错反序列化的 FB2 model，并列出不校验 ID 唯一性、min/max、顺序等偏差；其发布清单依赖 quick-xml 0.30、chrono、language-tags、serde。[README](https://github.com/r-glazkov/fb2/blob/v0.4.4/README.md) · [Cargo.toml](https://github.com/r-glazkov/fb2/blob/v0.4.4/Cargo.toml) · [完整 model](https://github.com/r-glazkov/fb2/blob/023f17b4268f71f27734234ab841e24d0b0abdd4/src/lib.rs) | MIT | **不采用运行时**；完整 model 与 base64 `String` 不满足本切片的有界内存目标 |
| `readest/foliate-js fb2.js` | 产品实践覆盖元数据、正文、TOC、附加 body 和链接，但使用整书 DOM、data URL、Blob / object URL，并形成第二套 Book 接口。 | MIT | **只学习行为**；不复制或引入 |

Atha 第一方代码是 `AGPL-3.0-or-later`。上述拟采用依赖都是宽松许可证，没有发现阻止组合的许可证冲突；实际接入仍须把精确版本和许可写入 package manifest / `THIRD_PARTY_NOTICES.md`。Readest 本身是 AGPL-3.0-or-later，但本方案不复制其实现；项目采用同类许可证也不取消来源、版权和修改说明义务。

## Atha 最小复用方案

### 导入边界

新增一个具体的 `reader::fb2` 模块和一个 `LocalLibrary` 后缀分支即可；不建立 importer registry、factory 或 trait。缓存 ID 使用固定 domain（例如 `atha-fb2-import-v1`）加原始源字节，避免同一字节以其他格式导入时碰撞。事务继续复用现有 staging、source-changed 复核、manifest 校验和同文件系统原子发布。

采用两次有界 XML pass：

1. **index pass**：校验命名空间、XML 编码、深度、元素和 ID 唯一性；只收集有界 metadata、顶层 section / TOC 结构、内部 ID 到目标 section 的映射、图片引用和 cover 引用。进入 root 尾部 binary 后，只解码被引用的 JPEG / PNG，验证魔数、尺寸和像素预算并直接写 staging；不保留完整正文树或全部 binary。
2. **render pass**：再次读取到最后一个 body，按 index 结果把已知元素写成受控 XHTML，随后停止，不再二次扫描尾部 base64。每个输出文件和写缓冲受 16 MiB 限制；完成后写现有 schema 1 manifest 和 metadata，再走当前发布事务。

两次顺序读取比维护完整 DOM 更省峰值内存，也比为 byte offset、随机 seek 或 XML subtree 建新抽象更短。只有 benchmark 证明第二 pass 是主瓶颈，才研究位置索引；研究阶段不预建。

### ReaderManifest 投影

| FB2 语义 | Atha 投影 |
| --- | --- |
| body 前置 image / title / epigraph | 有可见内容时生成一个前言 section |
| 第一 body 顶层 section | 每项一个 XHTML section；嵌套 section 留在该 XHTML |
| section title | 生成有界 heading 与确定性 anchor；按文档顺序压平进入现有 TOC |
| 附加 body / notes | 每个 body 一个无 TOC section，保留内部链接目标；不新增 `linear` 字段 |
| binary image | 只输出被引用且验证通过的 JPEG / PNG，并登记到 `resources` |
| coverpage | 第一张有效 cover 引用成为现有 `cover_path`；无效时无封面，不猜正文首图 |
| book-title / author | 映射到现有有界 title / authors；不保存其余暂未消费 metadata |
| locator | 继续使用 section id + 既有文本位置；不保存 FB2 XPath 或另建 locator |

第一片接受 real-world 常见的“section 同时含正文和子 section”，因为 event writer 自然能顺序保留它，且 `fb2 0.4.4` 明确把这列为现实兼容偏差。仍要求正确根命名空间、well-formed XML 和唯一有界 ID；未知元素整棵忽略，不把其属性或文本解释为 HTML。

### XHTML、CSS、链接与图片

- 所有 text、attribute 和 metadata 都通过现有 escaping / normalization；不把 CDATA、comment、processing instruction、raw XML 或未知元素直接拼进 XHTML。
- `p`、title、subtitle、strong、emphasis、strikethrough、sub、sup、code、epigraph、cite、poem、stanza、`v`、table 等映射为固定 XHTML 标签和 `fb2-*` class。`style name` 只保留经过长度和字符集限制的 `data-fb2-style`，不成为 inline CSS。
- `<stylesheet>` 第一片直接忽略。它既可能包含资源函数，也通常以 FB2 XML element 为 selector；直接交给浏览器既不安全也不保证转换后语义正确。后续 CSS 编辑器通过 Atha 用户样式覆盖 `fb2-*` class，不需要让书籍 stylesheet 获得额外能力。
- `xlink:href="#id"` 只有在 index pass 找到唯一目标时才改写为同 BookRoot 的 section + anchor。`http:` / `https:` 交给现有 `describeLink()` / `content-actions.mjs` 阻止实际打开；其他值只保留可读内容。
- 图片引用只指向 importer 生成的 BookRoot 相对资源。输出精确宽高和 alt / title；不接受 data URL、外部 URL、本机路径、SVG、GIF、WebP 或 MIME 与魔数不符的数据。
- 当前 [`content.mjs`](../../reader/web/content.mjs) 已有 `pendingImages`、可见区 / 下一页 bounds、`loadVisible()` 和逐图 `decode()`，但普通图片仍在 `renderCached()` 中 `Promise.all` 急切解码。FB2 只需让带可信宽高的 importer 图片进入这条既有延迟队列；不要再造图片缓存器。若 source XHTML 伪造同一标记，最坏只改变加载时机，不得扩大资源访问权限。

## 不可信输入边界

下表是建议的首片边界；复用现有常量时不要复制第三个匿名数字。

| 风险 | 首片边界 / 失败行为 |
| --- | --- |
| 源文件 | 普通文件、最大 512 MiB；fingerprint 与发布前复核都按真实读取计数 |
| XML 编码 | `DecodingReader` 识别 BOM / XML 声明；未知 label、malformed sequence 或声明切换失败稳定拒绝 |
| XML 主动能力 | `DocType`、DTD、实体声明、XInclude、外部 schema / stylesheet 读取和未知 processing instruction 全部拒绝或忽略，绝不调用网络 / 文件 resolver |
| 命名空间 / 深度 | 根必须是 FB2 2.0 `FictionBook`；最大深度复用 ComicInfo 的 64，越界拒绝 |
| 数量 | ReaderManifest sections / TOC 各不超过现有 2,000；binary 数不超过现有 archive 10,000 量级；ID 必须唯一且有界 |
| 生成资源 | 单个 XHTML / 图片不超过 16 MiB；所有落盘资源总量不超过 512 MiB；实际写入再次计数 |
| 图片 | 只收 JPEG / PNG；`imagesize` 类型必须与声明一致；宽高非零、单边不超过 8,192、像素不超过 20,000,000 |
| base64 | 忽略 XML whitespace 后用 `base64 0.22.1` 严格解码；先按编码长度做 checked 上限，再按实际 decoded bytes 二次限流 |
| metadata | title / author 继续用现有 512 Unicode scalar、16 authors 与控制字符清理；不记录未消费字段 |
| href | ID / href 长度有界；仅唯一 `#id` 与绝对 `http(s)` 保留活动 link 结构，后者仍由 reader 阻止 |
| CSS | 不导入 FB2 stylesheet，不输出源 inline style；只生成固定 class / data marker |
| 日志 | 只写 format、input bytes、encoding enum、body / section / binary / image / link 数、固定 stage / error code 和耗时；禁止标题、作者、路径、正文、URL、hash 和探测字节 |

建议稳定错误类别覆盖 source、encoding、XML、namespace、depth、ID、manifest count、base64、image、resource size、write 和 source-changed；UI 映射固定 code，不暴露底层 parser 英文串。

## 测试与 Android benchmark

### 最小测试

正向功能只使用用户放在 `fixtures/local` 的真实 FB2；测试以 opt-in 方式匿名发现，不把名称、路径、正文、URL 或哈希写进输出。当前样本应锁定为：1 section、0 TOC、249 个有效图片资源、764 个外部链接被阻止，并能搜索、翻页和恢复；这些计数是无内容聚合，不提交派生样本或截图。

小型 inline XML 只用于信任边界，不冒充正向书籍：错误 namespace、DTD / entity、重复 ID、失效内部链接、非法 base64、MIME / magic 不符、UTF-16 / Windows-1251 declaration、深度 / 数量 / 大小边界。测试运行时不打包用户书，也不生成临时 EPUB / FBZ。

### Android 功能门

扩展现有 `scripts/check-android-reader.ps1`，不要另建 UI runner。release-like APK 在固定 `Atha_API_35_16K` 上完成：

1. 系统 picker 选择 `.fb2`、导入、书架元数据与封面、打开到 reader ready；
2. 首 / 中 / 末页导航、文本选择、零命中随机 UUID 全书搜索；只回传耗时、结果数和 truncated boolean；
3. 随机抽取一个 `http(s)` link，验证固定“外部链接已阻止”状态且网络计数为零，不回传 host / URL；
4. 强停、重启、书架存在、同 content version 恢复 locator；
5. 数字诊断确认 249 张图片没有在首稳前全部 decode，可见页与下一页按需进入既有 pending queue；
6. logcat / AppLog 无路径、书名、URL、正文、hash、panic、ANR、OOM、renderer gone 或网络请求。

本地私有书不截图；视觉验收只由人工确认“正文 / 图片 / 翻页可见”，正式 evidence 保存匿名数值。

### 性能口径

固定 build、固定 AVD 与 WebView 版本，预热后每个阶段取 10 次并报告 median / nearest-rank P95：

- fingerprint、XML index、base64 decode + image probe、XHTML write、publish、total import；
- 缓存打开、section fetch / parse、pagination、首稳、首 / 中 / 末页跳转、单页 visible image decode、全书搜索、强停恢复；
- 书架、导入后、首稳、首 / 中 / 末页、连续往返三轮和返回书架后的 app PSS 与可归属 WebView renderer PSS。

样本的 249 张图片合计 149,136,347 像素，若全部常驻，单份 RGBA 理论值约为 `149,136,347 × 4` 字节，尚未计入压缩字节、DOM、缩放 surface 和 GPU 纹理。首轮不先发明绝对 PSS 阈值；先冻结 AVD 与 ARM64 真机基线，再在事实所有者中设回归门槛。但以下情况立即 no-go：

1. 首稳前急切 decode 249 张图，或单次打开触发 OOM / LMK / ANR / renderer 重启；
2. 10 次任一阶段越过正式 gate timeout，或三轮首→中→末往返后 PSS 仍近似线性增长而不进入平台期；
3. offscreen 图片产生资源请求 / decode，或外部链接触发任何网络请求；
4. 为通过 AVD 而扩大 heap、降低图片安全上限、缓存整书 DOM / base64 或并行解码全部资源；
5. 只测 app 主进程、只测 Windows / AVD，或缺少 ARM64 真机同 build 数据，却宣称 Android 性能完成。

若既有可见区延迟队列仍 no-go，第二选择才是按完整 block 边界把一个源 section 技术拆分，TOC 只指向首片、内部 ID 重写到对应片；不要先引入虚拟 DOM、图片转码、专用分页器或格式 locator。

## FBZ、notes 与停止条件

### FBZ 后续兼容

用户提供真实 `.fb2.zip` / `.fbz` 后，可在独立小片复用 [`reader::archive`](../../backend/atha-backend/src/reader/archive.rs)：先执行重叠、加密、符号链接、路径、成员数、单成员和总解压预算，再要求**恰好一个**普通 `.fb2` 成员。不要复制 Readest 的“没有 `.fb2` 就取第一个成员”fallback，不把 ZIP 展开到任意目录，也不接受多候选 first-wins。

### 需要停止并重新定界的情况

1. 真实样本依赖多个 notes body 的非线性阅读顺序，而现有 manifest 无法避免把脚注当顺序正文；先在通用 schema 评审可选 `linear`，不要在 FB2 模块藏状态。
2. 大量目标书依赖 source stylesheet 才可读；先设计经过 CSS parser、selector 重写和现有资源函数拒绝的统一 source-CSS 管线，不把原 CSS 直通 WebView。
3. 真实样本主要图片不是 JPEG / PNG，或有效图片超过现有像素边界；以匿名 corpus 计数和 Android decode 证据开启下一片，不按 MIME 名称顺手扩格式。
4. `quick-xml` 两 pass 的 `xml_index_ms` / `render_write_ms` 被 benchmark 证明为主瓶颈；再研究 byte offset 或单 pass spool。未证明前不建随机访问索引。
5. FBZ 没有真实正向 fixture、ARM64 真机不可用，或私有内容会进入日志 / 截图 / snapshot；对应范围不得标记完成。

## 决策摘要

| 主题 | 首片采用 | 延期 / 拒绝 |
| --- | --- | --- |
| 输入 | 普通 `.fb2` XML | `.fbz` / `.fb2.zip` 等待真实样本 |
| parser | `quick-xml 0.41` + `encoding` / `DecodingReader` | `fb2` 完整 model、浏览器 DOM parser |
| binary | `base64 0.22.1`，只解码引用的 JPEG / PNG | data URL、全部 binary 常驻、SVG / 其他图片 |
| 文档模型 | 两 pass → 现有 ReaderManifest / BookRoot / Locator | 临时 EPUB、foliate Book、format factory / trait |
| section / TOC | 前言 + 顶层 section；嵌套 title 压平 | 格式专属 locator、预建 `linear` schema |
| CSS | 固定 `fb2-*` 语义 class + Atha 用户 CSS | 直通 source stylesheet / inline style |
| link | 验证后的 `#id`；`http(s)` 可见但阻止 | 文件、data、javascript、相对路径与未知 scheme |
| 图片运行时 | 复用既有 visible / next-page pending queue | 首稳前 `Promise.all` 解码整章、另造缓存器 |
| 性能完成 | AVD 功能 / 回归 + ARM64 真机同 build 基线 | 用 Windows 或 AVD 代称真机性能 |

这是一份实施前研究，不是已完成的 FB2、Android 或生产等价验收。建议值只有进入 accepted change、变成常量 / 检查并取得对应证据后，才升级为项目契约。

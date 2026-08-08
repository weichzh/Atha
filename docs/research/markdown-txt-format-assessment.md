---
description: 评估 Markdown 与 TXT 导入的上游实现、依赖、安全边界和 Android 验收方案。
---

# Markdown 与 TXT 格式评估

> 研究快照：2026-08-08。Readest 是对标对象；文中的“当前”只指下述固定提交，不随其默认分支继续漂移。
>
> 范围：Markdown、纯文本和既有 `ReaderManifest` 投影。本文不设计多格式 factory，不改变 EPUB/CBZ 导入，也不保存本地书籍的名称、正文、路径或内容哈希。

## 结论

1. Markdown 采用 `pulldown-cmark 0.13.4`。它是成熟的 CommonMark pull parser，事件流让导入器能在生成 XHTML **之前** 把原始 HTML 转成普通文本，并逐个处理链接和图片；不需要再引入一套 DOM 或 HTML sanitizer。[`pulldown-cmark` 文档](https://docs.rs/pulldown-cmark/0.13.4/pulldown_cmark/)列出其 pull-parser API 和扩展选项，[`Event`](https://docs.rs/pulldown-cmark/0.13.4/pulldown_cmark/enum.Event.html)把 `Html`/`InlineHtml` 与普通文本分开，[`Tag`](https://docs.rs/pulldown-cmark/0.13.4/pulldown_cmark/enum.Tag.html)暴露链接和图片的目标 URL。
2. TXT 采用 `encoding_rs 0.8.35` 与 `chardetng 1.0.0`。BOM 和严格 UTF-8 产生确定结果；无 BOM 的遗留编码只能诚实标为 best effort。不得移植 Readest 自写的高字节比例和语言模式猜测。[`encoding_rs`](https://docs.rs/encoding_rs/0.8.35/encoding_rs/)提供 WHATWG 编码及流式解码，[`chardetng`](https://docs.rs/chardetng/1.0.0/chardetng/)提供增量检测。
3. 两种格式都直接生成现有 BookRoot。Markdown 以一级标题为篇章边界；TXT 只识别重复出现的高置信整行章节标题。优先把统一 ReaderManifest 上限从 1000 有界提高到 2000，让真实样本约 1,134 个语义 section 继续保持“一章一 section”；Android 的启动、全书搜索和 PSS 是该取舍的 go/no-go。只有实测不达标时才升级为多章分组与 fragment label 改造。不要先生成临时 EPUB，也不要增加另一套 reader。
4. Markdown 原始 HTML 一律按文本显示；第一版所有作者链接都保持可读但不可点击，图片只显示替代文本或占位。单文件 Markdown 没有可信资源根，不能据相对路径读取本机文件，也不能请求网络资源。
5. Android 先做功能与相对性能门。导入阶段记录固定数字字段，阅读阶段复用既有首稳、翻页、恢复和 PSS 口径；x86_64 AVD 只能证明目标端功能和同环境回归，ARM 真机数据到位前不作跨设备性能承诺。

## Readest 固定基线

2026-08-08 查询 Readest 默认分支时，锚点是提交 [`629ab2919a5812156af6152015ddfd0c34c6843b`](https://github.com/readest/readest/commit/629ab2919a5812156af6152015ddfd0c34c6843b)，其 app 包版本为 `0.11.20`。[固定提交的 `package.json`](https://github.com/readest/readest/blob/629ab2919a5812156af6152015ddfd0c34c6843b/apps/readest-app/package.json)同时固定了下述 JavaScript 依赖版本范围，避免把后续主线行为误写成当前事实。

Readest 的格式分派有两个不同适配器：

- Markdown 由 `makeMarkdownBook()` 直接打开；
- TXT 由 `TxtToEpubConverter` 生成 EPUB，再交给通用文档加载器。

扩展名、MIME 识别和分派可在固定提交的 [`document.ts`](https://github.com/readest/readest/blob/629ab2919a5812156af6152015ddfd0c34c6843b/apps/readest-app/src/libs/document.ts#L119-L142)及[打开分支](https://github.com/readest/readest/blob/629ab2919a5812156af6152015ddfd0c34c6843b/apps/readest-app/src/libs/document.ts#L338-L379)核对。

### Readest Markdown

固定实现使用 `marked` 的 GFM 选项和 `marked-footnote`，随后调用项目的 `sanitizeHtml()`，再以 DOM 处理标题和章节。[初始化与清理链路](https://github.com/readest/readest/blob/629ab2919a5812156af6152015ddfd0c34c6843b/apps/readest-app/src/utils/md.ts#L1-L75)可直接核对。

章节规则很简单：第一个一级标题之前的非空内容成为前言；每个一级标题开启一个 section；没有一级标题时整篇只有一个 section。各级标题同时组成嵌套目录，最后序列化为 XHTML。[章节和目录实现](https://github.com/readest/readest/blob/629ab2919a5812156af6152015ddfd0c34c6843b/apps/readest-app/src/utils/md.ts#L94-L163)还说明它不是按文件夹或 front matter 分章。Readest 为这些内存 section 构造自己的 locator/CFI 适配层，[book 对象和 URI 处理](https://github.com/readest/readest/blob/629ab2919a5812156af6152015ddfd0c34c6843b/apps/readest-app/src/utils/md.ts#L199-L264)不应原样移植到 Atha。

可借鉴的是“H1 分章、无 H1 单章”；不借鉴的是 JavaScript DOM 管线、全书 XHTML 常驻内存和第二套 locator。

### Readest TXT

Readest 对较小输入读取完整字节，对较大输入改用 `File.stream()`；固定阈值、转换入口和分段读取见 [`txt.ts` 的输入路径](https://github.com/readest/readest/blob/629ab2919a5812156af6152015ddfd0c34c6843b/apps/readest-app/src/utils/txt.ts#L99-L231)及[流式段落路径](https://github.com/readest/readest/blob/629ab2919a5812156af6152015ddfd0c34c6843b/apps/readest-app/src/utils/txt.ts#L288-L495)。它的编码选择由 BOM、严格/宽松 UTF-8、高字节比例以及 GBK/GB18030/Shift-JIS 等规则共同决定，[编码启发式](https://github.com/readest/readest/blob/629ab2919a5812156af6152015ddfd0c34c6843b/apps/readest-app/src/utils/txt.ts#L871-L1008)是产品经验，不是标准检测器。

章节识别先尝试中文 `第…章/节/回/讲/篇/话`、卷部标题、固定前后记词和英文 `Chapter`，再尝试更宽的裸数字标题；只有分出多个有效部分且单部分不过大才接受，否则按固定段落数回退。[固定实现的章节规则](https://github.com/readest/readest/blob/629ab2919a5812156af6152015ddfd0c34c6843b/apps/readest-app/src/utils/txt.ts#L514-L737)明确包含这些启发式。输出则是合成的 EPUB2，每章一个 XHTML/spine 项，[EPUB 生成](https://github.com/readest/readest/blob/629ab2919a5812156af6152015ddfd0c34c6843b/apps/readest-app/src/utils/txt.ts#L740-L868)只是适配 Readest 现有加载器的选择。

Atha 可借鉴“流式处理、章节边界必须经过有效性检查”；不复制宽松裸数字规则、自写编码检测和临时 EPUB 包装。

## 本地 TXT 预检

2026-08-08 对用户提供的唯一正向 TXT 样本做了只读、无内容预检。下面只保存聚合数字，不保存书名、文件名、正文、路径或哈希：

| 探针 | 结果 | 含义 |
| --- | --- | --- |
| 输入规模 | 7,362,028 bytes | 适合首轮 Android 流式解码和内存基线 |
| BOM/UTF-8 | 无 BOM；严格 UTF-8 失败 | 不能把无 BOM TXT 默认当成 UTF-8 |
| 遗留编码 | 严格 GB18030 解码成功 | 必须实际验收 `chardetng` + `encoding_rs` 中文路径 |
| 行结构 | 92,533 行，CRLF | decoder 与章节扫描必须跨块保留行状态并统一换行 |
| 高置信章节候选 | 1,134 | 当前 1000 上限不足，但仍低于既有 2000 TOC 上限 |
| 局部规模 | 最大章约 10,234 bytes；首章前正文 421 bytes | 无需拆章；前置正文可与第一组同 section 保存 |

这些是当前环境的真实样本探针，不是所有 TXT 的格式保证。尤其“严格 GB18030 成功”只证明候选 decoder 能完整消费该样本；最终选码仍须用锁定版本的 `chardetng` 在实现测试中复现。`chardetng` 明确把 GB18030 检测为 GBK，而 WHATWG [GBK decoder](https://encoding.spec.whatwg.org/#gbk-decoder)和 [`encoding_rs::GBK`](https://docs.rs/encoding_rs/0.8.35/encoding_rs/static.GBK.html)都说明其 decoder 与 gb18030 decoder 相同，所以这不会丢失四字节解码能力。

样本还给出了一个更简单的结构选择：每章最大约 10 KiB，单章远低于 16 MiB 资源上限；真正未知的是约 1,134 个 section 对 Android 启动、DOM 目录、全书搜索时约 1,134 次资源读取和 PSS 的影响。因此先只提高有界数量上限并测量，不应先引入章节分组、fragment 定位和各功能的 offset-aware label。

## 依赖与许可证

| 组件 | 固定研究版本 | 一手事实 | 决策 |
| --- | --- | --- | --- |
| `pulldown-cmark` | 0.13.4 | CommonMark pull parser，扩展显式开启；crate 标注 MIT。[API](https://docs.rs/pulldown-cmark/0.13.4/pulldown_cmark/) · [crate 元数据](https://docs.rs/crate/pulldown-cmark/0.13.4) | 采用；事件流足够完成分章、目录和安全改写 |
| `comrak` | 0.54.0 | CommonMark/GFM AST；默认 `unsafe = false` 会过滤 raw HTML 和危险链接，`escape = true` 可转义 HTML；BSD-2-Clause。[API](https://docs.rs/comrak/0.54.0/comrak/) · [`RenderOptions`](https://docs.rs/comrak/0.54.0/comrak/options/struct.Render.html#structfield.unsafe) · [crate 元数据](https://docs.rs/crate/comrak/0.54.0) | 可行备选；当前不需要完整 AST 和更大的功能面 |
| `encoding_rs` | 0.8.35 | WHATWG 编码实现；支持增量 `Decoder`，`for_bom()` 识别 UTF-8、UTF-16LE、UTF-16BE BOM；MIT OR Apache-2.0。[API](https://docs.rs/encoding_rs/0.8.35/encoding_rs/) · [`for_bom`](https://docs.rs/encoding_rs/0.8.35/encoding_rs/struct.Encoding.html#method.for_bom) · [`Decoder`](https://docs.rs/encoding_rs/0.8.35/encoding_rs/struct.Decoder.html) | 采用；不自写 decoder |
| `chardetng` | 1.0.0 | `feed()` 增量收样，`guess()` 给出编码；不检测无 BOM UTF-16，并把 GB18030 检测为 GBK；MIT OR Apache-2.0。[API](https://docs.rs/chardetng/1.0.0/chardetng/struct.EncodingDetector.html) · [上游 README](https://docs.rs/crate/chardetng/1.0.0/source/README.md) · [crate 元数据](https://docs.rs/crate/chardetng/1.0.0) | 采用；结果明确是遗留编码的 best effort |
| `regex` | 1.13.1 | 有限自动机实现，对固定 pattern 的搜索为输入线性时间；MIT OR Apache-2.0。[API](https://docs.rs/regex/1.13.1/regex/) · [crate 元数据](https://docs.rs/crate/regex/1.13.1) | 采用一个预编译、首尾锚定的章节 pattern；该版本已在当前 [`Cargo.lock`](../../Cargo.lock) 中 |

项目许可证是 [`AGPL-3.0-or-later`](../../CONTEXT.md)。上述候选都是宽松许可证；GNU 的[许可证兼容性说明](https://www.gnu.org/licenses/license-compatibility.en.html)说明 GPLv3 可组合 Apache License 2.0 等兼容代码，GNU [GPL FAQ](https://www.gnu.org/licenses/gpl-faq.en.html#AllCompatibility)说明 GPLv3 与 AGPLv3 的组合规则。研究未发现阻止采用的许可证冲突；发版时仍须以实际 `Cargo.lock`、crate LICENSE 文件和第三方 notices 审计为准。

## Markdown 投影

### 输入与章节

`pulldown-cmark::Parser` 接受一段 UTF-8 `&str`，事件多数借用输入；上游建议把 HTML 写入预分配的 `String`/`Vec` 以减少复制。[parser 和性能说明](https://docs.rs/pulldown-cmark/0.13.4/pulldown_cmark/)意味着它不是任意大文件的分块 parser。第一版应把 Markdown 原始输入限制在现有单资源上限 16 MiB，而不是把 512 MiB 外层书籍上限等同于安全内存预算；该限制复用 [`resources.rs`](../../backend/atha-backend/src/reader/resources.rs) 的既有资源边界，扩大前须先取得 Android 证据。

分章规则与 Readest 对齐，但输出 Atha 原生结构：

1. 第一个 H1 之前有可见内容时生成前言 section；
2. 每个 H1 开启新 section；
3. 没有 H1 时整篇生成一个 section；
4. 各级标题生成目录节点，内部 `id` 使用解析顺序派生的稳定编号，不实现 GitHub slug 方言；
5. YAML 风格 metadata block 若通过 parser 选项识别，只丢弃，不另引入 YAML 解释器，也不把任意 metadata 变成应用元数据。

Parser 选项从 `Options::empty()` 明确加入 `ENABLE_TABLES`、`ENABLE_FOOTNOTES`、`ENABLE_STRIKETHROUGH`、`ENABLE_TASKLISTS`、`ENABLE_GFM` 和 `ENABLE_YAML_STYLE_METADATA_BLOCKS`，覆盖 Readest 当前 GFM/footnote 范围与仓库文档 front matter；不要使用 `Options::all()` 意外开启 wikilink、heading attributes、math 等未验收方言。各 flag 的上游定义见 [`Options`](https://docs.rs/pulldown-cmark/0.13.4/pulldown_cmark/struct.Options.html)。脚注正文保留可读；第一版不承诺跨 section 的活动脚注链接。`Event::TaskListMarker` 必须改写为普通的勾选/未勾选文本，不能让默认 renderer 生成 reader 明确禁用的 `<input>` 元素；脚注引用也改成可读文本标记。

每个 section 写入独立的受控 XHTML 文件并登记到 `ReaderManifest.sections`；`resources` 保持为空，除非将来确有 importer 自产的共享 CSS。XHTML 不能误放进只接受 CSS/图片等附属资源的 `resources` 数组。这直接复用 [`session.mjs`](../../reader/web/session.mjs) 的 schema、1000 sections/2000 toc items 上限和既有 Locator；不建立 Markdown book 对象、格式 factory 或第二套路由。仓库正向 Markdown 文档远低于上限；第一版遇到超过 1000 个 H1 或 2000 个目录项的 Markdown 时稳定拒绝，不为没有真实样本的边界先建分组策略。

### HTML、链接和图片

`pulldown-cmark` 是 parser，不是 sanitizer；安全性来自事件变换：

- `Event::Html` 与 `Event::InlineHtml` 改写为普通 `Text`，交给 renderer 转义后显示源码字面量；不得直接送入 `html::push_html()`；
- `Tag::Link` 保留可见 label 和 title 信息，但第一版不输出可点击的 `href`。CommonMark 的 [ATX heading 规范](https://spec.commonmark.org/0.31.2/#atx-headings)没有定义浏览器 fragment ID，贸然只放行 `#fragment` 也会产生错误跳转；
- `Tag::Image` 只输出 alt 文本或固定占位，不读取相对/绝对本机路径，不加载 `http(s)`、`file:`、`data:`、`blob:` 或未知 scheme；
- parser 输出之后仍经过既有 [`content.mjs`](../../reader/web/content.mjs) XHTML 清理和 WebView 导航拦截，作为纵深防御，而不是弥补导入器放行 raw HTML。

这牺牲了第一版的活动链接和内嵌图片，但完整保留正文可读性，并符合“不可信书籍无网络、无路径越界”的项目边界。若将来要支持 Markdown 资源，先定义可信容器和资源根；不能让单文件 `.md` 隐式获得所在目录的读取权限。

## TXT 解码与章节

### 两遍、有界解码

WHATWG 编码标准规定 BOM sniff 的三种签名及其优先级，[标准算法](https://encoding.spec.whatwg.org/#bom-sniff)可由 `encoding_rs::Encoding::for_bom()` 直接实现。导入流程如下：

1. 第一遍读取有界字节流。开头检查 BOM；同时做严格增量 UTF-8 校验，并把样本喂给 `chardetng`。
2. 有 BOM 时由 BOM 决定编码；无 BOM 且全流是合法 UTF-8 时选择 UTF-8；否则调用 `chardetng::guess(None, Utf8Detection::Allow)`。不伪造站点 TLD 或系统 locale；若返回 GBK，直接使用 `encoding_rs::GBK` 的 gb18030 decoder。
3. 第二遍用 `encoding_rs::Decoder` 增量解码，按 `for_bom()` 返回长度只移除开头 BOM，并检查 without-replacement 的错误结果；解码失败返回稳定错误，不悄悄替换正文。
4. 无 BOM UTF-16 不在自动承诺内，因为 `chardetng` 明确不检测它。单字节遗留编码通常也无法从“可解码”证明选择正确，因此 UI/日志不得显示虚假的置信度。
5. 只有真实正向样本证明自动选择错误时，才增加显式编码覆盖入口；先不要自建候选打分系统。

导入器最多保留 decoder 状态、当前行、当前 section 写缓冲和章节索引；不把完整解码文本或全部 XHTML 留在内存。输入仍受 [`archive.rs`](../../backend/atha-backend/src/reader/archive.rs) 的 512 MiB 外层限制，单个生成资源受 16 MiB 限制。

### 可诚实承诺的中文网文章节规则

IETF 的 `text/plain` 定义不提供格式命令或处理指令，[RFC 2046 §4.1.3](https://www.rfc-editor.org/rfc/rfc2046#section-4.1.3)也没有章节元数据；Readest 的实现只能作为产品先例，不能成为正确性保证。Atha 第一版只承诺以下最小启发式：

- 先把 CRLF/CR 统一为 LF；候选必须占据完整的 trimmed line，长度不超过 80 个 Unicode 标量；
- 主规则是 `第 + 中文或 ASCII 数字 + 章/节/回/篇 + 可选短标题`；可把独立的“楔子、序章、前言、后记、番外”视为附属边界；
- 至少发现两个主规则候选后才启用语义分章。第一个候选之前的非空正文成为前言；单个疑似标题不能把普通段落误切成目录；
- 不识别裸 `1`、裸中文数字、任意全大写行或每固定 100 段一个“章节”；这些规则召回率更高，但会制造无法诚实解释的目录；
- 每个识别出的语义章节对应一个 section 和一个目录项。统一 manifest 上限提高到 2000 后，超过 2000 章或任一生成 section 超过 16 MiB 时稳定拒绝；不要为超限输入静默拆章或并章；
- 不足两个候选时，不声称识别出章节：较小文本生成一个“正文”section；超过单资源上限时只按空行附近切成有界“正文片段”，这些技术片段不是章节。

章节主规则用 `regex 1.13.1` 编译一次并以 `^…$` 锚定；长度检查和固定前后记词用标准库完成。不要每行重新编译 regex，也不要实现回溯或自写数字状态机。正文转 XHTML 时逐行转义，每个非空逻辑行输出一个 `<p>`，连续空行不制造空段；不解释 HTML、Markdown、文件路径或 URL。章节检测只观察解码后的标题行，不执行内容。

## ReaderManifest 与缓存契约

两种适配器应沿用当前 EPUB/CBZ 的导入事务：指纹后写同文件系统 staging、校验源未变化、完成后原子发布；缓存命中只读取已经完整验证的 BookRoot。实现可共享很小的“写 manifest/发布 staging”内部函数，但不要建立带注册表、trait hierarchy 或动态分派的多格式 factory。

| 语义 | Markdown | TXT | 既有契约 |
| --- | --- | --- | --- |
| section | 前言/H1 篇章 | 每个高置信章节一个；无章节时为正文片段 | `sections[].id/href`，拟统一提高到 2000 |
| section 文档 | 每 section 一个受控 XHTML | 每章/片段一个受控 XHTML | 每文件最多 16 MiB，规范化相对路径 |
| resources | 第一版为空 | 第一版为空 | 只登记 CSS/图片等允许的附属资源，不登记 XHTML |
| toc | 标题层级 | 每章一个 section 目标；技术片段只标“正文片段” | 最多 2000，目标必须存在 |
| locator | section id + 既有文本位置 | section id + 既有文本位置 | 不创建格式专属 locator |
| content version | 既有文件指纹与 importer schema 版本 | 同左 | 缓存与书架继续使用现有字段 |

当前 schema 上限由 [`session.mjs`](../../reader/web/session.mjs) 的 `MAX_SECTIONS = 1000` 和 [`resources.rs`](../../backend/atha-backend/src/reader/resources.rs) 的硬编码 `1_000` 分别执行。实施时把 **manifest 全局验证上限**统一改为 2000，并让 TXT 使用该上限；现有 EPUB `MAX_SECTIONS` 与 CBZ `MAX_PAGES` 可以继续保留 1000 的格式专属限制，避免无证据扩大其他格式的输入面。格式适配器不得再复制第三个无名数字。

### 一章一 section 的评估结果

优先结论是保留该模型并把 manifest 全局验证上限有界提高到 2000。真实样本预检得到 1,134 个高置信章节候选，最终 section 与 TOC 精确计数由实现规则的测试锁定；无论前置正文单列还是并入首章都仍低于上限。该模型让现有搜索、注释、书签和阅读进度继续沿用“section 就是章节”的简单语义，无需新增 fragment mapping。

这仍是需要 Android 证据的项目契约变化，而不是仅改常量：[`search.mjs`](../../reader/web/search.mjs) 会逐 section 读取和解析正文，目录控件、session 描述和恢复状态也会持有 section 元数据。对真实样本必须测冷启动/缓存打开、1134 项目录、全书搜索和 PSS；若出现 OOM/ANR、超过正式 gate timeout，或 P95/PSS 相比同设备基线出现不可接受回归，才启用后备方案。

后备方案是把连续完整章节分组进最多 1000 个 section，每章保留 `#chapter-N` TOC。[`navigation.mjs`](../../reader/web/navigation.mjs) 已支持 fragment 跳转，但 `search.mjs` 的结果 label 与 [`annotations.mjs`](../../reader/web/annotations.mjs) 的 section filter 目前只取同 path 的第一个目录项；采用后备方案时必须把它们改为按 locator offset 解析最近 fragment。该复杂度是实测失败后的升级，不是默认实现。

## 样本与测试

正向样本严格遵从当前用户约束：

- Markdown 使用已被版本控制的 [`README`](../../README.md) 和 [EPUB2 研究文档](epub2-ncx-library-assessment.md)，覆盖多级标题、列表、代码和普通链接；不新造 Markdown 书籍 fixture；
- TXT 只使用 `fixtures/local` 中用户提供的网文，验证真实编码、约 1,134 个语义 section、目录和长文本性能；它始终留在忽略目录；
- 报告、测试快照、Android evidence 和日志不记录样本书名、正文、本机路径或内容哈希，只记录格式、输入字节数、固定编码标签、section 数和阶段耗时；
- 不复制、截取、派生或提交本地书籍。人工核对只报告是否通过；章节数可以作为无内容计数进入 evidence，标题仍不保存。

最小合成输入仅用于信任边界单元测试，不冒充正向样书：raw `<script>`/内联 HTML、`javascript:` 链接、远程/本机图片、无 BOM UTF-16、非法 UTF-8、超长标题行和 section 数上限。断言目标是“不可执行、不可联网、不可越界、错误稳定”，不是全文快照。

至少覆盖这些行为：

1. Markdown 无 H1、前言 + 多 H1、重复标题、多级目录、raw HTML、链接、图片和 16 MiB 边界；
2. TXT UTF-8/UTF-8 BOM/UTF-16 BOM、真实样本自动编码、跨读取块的多字节字符和 CRLF；
3. 中文章节至少两个才启用、单个疑似标题不分章、1001 至 2000 章的一章一 section、无章节大文本只生成技术片段、超限稳定失败；
4. manifest 中每个 section/resource/toc 目标都存在，路径规范化，XHTML 经既有清理器后可读；
5. 强停后书架与 locator 恢复，重新导入命中完整缓存，损坏 staging 不可见。

## Android 优先验收与 benchmark

### 功能门

扩展 [`check-android-reader.ps1`](../../scripts/check-android-reader.ps1) 的正式入口，而不是另写临时 UI runner。Markdown 和 TXT 分别执行：系统 picker、导入、打开、目录跳转、首末 section、搜索/选择、翻页、强停恢复、locator 恢复和 picker 缓存清理；同时检查应用与 WebView 日志中没有脚本执行、网络尝试、路径泄漏、panic、ANR 或 OOM。

日志只增加固定字段：

- `format`、`input_bytes`、`encoding`（TXT）、`sections`；
- `fingerprint_ms`、`detect_ms`、`decode_ms`、`chapter_scan_ms` 或 `markdown_parse_ms`、`render_write_ms`、`publish_ms`、`total_ms`；
- `manifest_validate_ms`、`toc_bind_ms`、`search_ms`、`search_results`、`search_truncated`；
- 稳定错误码和所在阶段。

不得记录标题、正文、搜索词、原始/规范化路径、URL、内容哈希或探测字节。编码标签来自固定枚举，不把 detector 内部细节写入日志。

### 性能口径

既有本机 reader gate 每阶段取 10 个样本、报告 median 和 nearest-rank P95；冷启动、首稳、热打开、翻页、重排的本机门槛分别是 2000/750/120/50/150ms，内存只记录不设失败阈值。[`check-reader-slice.ps1`](../../scripts/check-reader-slice.ps1)和 [`READER-CORE.md`](../architecture/READER-CORE.md)是该口径的事实所有者。

Markdown/TXT 的 Android 测量应复用“固定 build + 固定设备 + 预热后 10 次 + median/P95”统计方式，但**不能**直接套用 Windows/WebView2 数值：

1. 先在固定 16 KiB x86_64 AVD 上分别记录冷导入、缓存打开、首稳、首/中/末章目录跳转、翻页、强停恢复和全书搜索；
2. 在书架、导入完成、首 section、末 section 和恢复后采集 app PSS；若 WebView renderer 无法可靠归属，继续明确记录为空，不把系统级进程混入；
3. TXT 以 7,362,028-byte、92,533-line 的用户本地网文作为主要解码/分章压力样本；Markdown 使用仓库 [`README`](../../README.md) 和既有 [EPUB2 研究文档](epub2-ncx-library-assessment.md)，只对其当前尺寸范围作声明；
4. 调优只针对实测瓶颈。TXT 的首要不变量是“不让完整解码文本与全部 XHTML 同时驻留”；Markdown 的首要边界是 16 MiB 输入上限和逐 section 落盘；
5. AVD 结果达到功能门后，再在至少一台 ARM64 Android 真机以同一 release-like build 重跑。只有真机数据才能支持“Android 性能达标”措辞；AVD 只支持功能与同环境回归结论；
6. 第一轮真机数据形成基线后，在事实所有者中冻结具体 P95/内存回归门槛。研究阶段不凭桌面强机或单次结果发明阈值。

全书搜索使用只存在于本机、不会写入脚本或 evidence 的查询词；gate 只记录耗时、结果数和是否截断。提高到 2000 sections 的 go/no-go 至少要求 10 次流程无 OOM/ANR、均在正式 gate timeout 内完成，并把启动、搜索 P95 与各阶段 PSS 交给当前 change 验收；若这些数据不能接受，回退常量并实施前述分组方案，不在高上限上叠加并行搜索来掩盖结构问题。

测试证据按等级报告：Rust 单元/集成和桌面 reader gate 是本地证据；固定 AVD 是 Android 目标端功能与相对性能证据；ARM64 真机是目标端性能证据。三者不得互相代称。

## 实施顺序与停止条件

1. 在现有 reader importer 边界增加两个显式入口，先接 `pulldown-cmark`、`encoding_rs`、`chardetng` 和稳定错误码；不建 factory。
2. 先完成 raw HTML/链接/图片、编码和路径边界单元测试，再投影 XHTML/manifest。
3. Markdown 用仓库 [`README`](../../README.md) 与既有 [EPUB2 研究文档](epub2-ncx-library-assessment.md)通过桌面 reader gate；TXT 用用户本地网文通过 GBK/gb18030 decoder、约 1,134 个语义 section 和对应 TOC 检查。
4. 把 manifest 验证上限有界提高到 2000，再接入 Android picker、日志字段、强停恢复、真实全书搜索和 10 样本 benchmark；根据阶段耗时与 PSS 定位瓶颈。
5. 只有 `decode_ms`/PSS 证明 decoder 路径是瓶颈时才优化缓冲大小；只有 `chapter_scan_ms` 证明规则扫描是瓶颈时才优化匹配器。不要先增加并行解析、缓存层或自写 SIMD。
6. 固定 AVD 功能通过、真机性能基线完成、日志无内容泄漏后，才把 Markdown/TXT 标为 Android 完成。若 1134 sections 的启动/搜索/PSS 不达标，再实施章节分组与 offset-aware label；若真实 TXT 样本编码猜错或章节规则无法达到可解释结果，停止并记录证据，再决定显式编码选择或规则扩展。不得用更宽泛的猜测掩盖失败。

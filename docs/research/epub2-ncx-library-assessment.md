---
description: 评估 Atha 在 Android 优先路线中兼容 EPUB 2.0.1 与 NCX 的最小实现、成熟库取舍、安全边界和验证门禁。
---

# EPUB2 / NCX 最小兼容与成熟库评估

## 结论先行

1. **继续扩展现有 `zip 8.6` + `quick-xml 0.41` 管线，不增加 EPUB 运行时依赖。** Atha 已经在同一 deep module 内完成 ZIP 重叠、加密、路径、条目数、单成员和总解压量边界；`quick-xml` 的 `NsReader` 又能流式解析并解析命名空间。一个有界的 OPF2 分支和 NCX 状态机足以投影到既有 `ReaderManifest`，换库反而会重建这些信任边界（[Atha archive](../../backend/atha-backend/src/reader/epub/archive.rs)、[`NsReader` 文档](https://docs.rs/quick-xml/0.41.0/quick_xml/reader/struct.NsReader.html)、[`ZipArchive` 文档](https://docs.rs/zip/8.6.0/zip/read/struct.ZipArchive.html)）。
2. **对外应称为“EPUB2 XHTML / NCX 子集”，不能称为完整 EPUB 2.0.1 Reading System。** EPUB2 spine 还允许 DTBook、废弃 OEBPS 文档、XML Island 和 fallback 链；Atha 当前 ReaderManifest 与内容边界只接受 XHTML。保留这个窄边界比为了格式名引入另一套渲染模型更安全（[OPF 2.0.1 spine](https://idpf.org/epub/20/spec/OPF_2.0_latest.htm#Section2.4)、[OPS 2.0.1 内容类型](https://idpf.org/epub/20/spec/OPS_2.0.1_draft.htm#Section1.4.1)）。
3. **EPUB2 必须从 `spine@toc` 找 NCX，不猜测任意 NCX 文件；EPUB3 仍必须有且只有一个 `properties="nav"` 项。** EPUB2 规定 `spine@toc` 引用 manifest 中 `application/x-dtbncx+xml` 的 NCX；EPUB3 把 NCX / guide 保留为给 EPUB2 阅读器的兼容数据，并明确 EPUB3 阅读器不使用这些 legacy feature。因此不能用 NCX 给缺少 nav 的损坏 EPUB3 兜底（[OPF 2.0.1 NCX requirements](https://idpf.org/epub/20/spec/OPF_2.0_latest.htm#Section2.4.1.2)、[EPUB 3.3 legacy support](https://www.w3.org/TR/epub-33/#sec-package-legacy-support)、[EPUB 3.3 nav item](https://www.w3.org/TR/epub-33/#sec-item-elem)）。
4. **只把 `navMap` 按前序拍平成现有 flat TOC；首版不为 `guide`、`pageList`、`navList` 建模。** `navMap` 是 NCX 必选的层级主导航；`guide` 可选且 Reading System 无须使用，`pageList` 仅在有纸页语义时要求，`navList` 可选。拍平是对当前契约的最小、确定性投影，不要求改变 Locator、ReaderManifest 或 UI（[NCX DTD](https://www.daisy.org/z3986/2005/ncx-2005-1.dtd)、[OPF 2.0.1 NCX rules](https://idpf.org/epub/20/spec/OPF_2.0_latest.htm#Section2.4.1.2)、[OPF 2.0.1 guide](https://idpf.org/epub/20/spec/OPF_2.0_latest.htm#Section2.6)）。
5. **文档声明必须是惰性精确白名单，绝不解析外部 DTD 或通用实体。** OPF 没有一个需要放行的“标准固定 DOCTYPE”；NCX 可以无 DOCTYPE，或使用 canonical NCX declaration；EPUB2 XHTML 的标准依据是 XHTML 1.1 模块，而不是 XHTML 1.0 Strict。允许 XHTML 1.0 Strict 只能算产品兼容扩展（[OPF 2.0.1 package identity](https://idpf.org/epub/20/spec/OPF_2.0_latest.htm#Section1.3.2)、[OPS 2.0.1 XHTML profile](https://idpf.org/epub/20/spec/OPS_2.0.1_draft.htm#Section2.1)、[XHTML 1.1 conformance](https://www.w3.org/TR/xhtml11/conformance.html)）。
6. **Readest / foliate-js 是架构参照，不是本切片的依赖。** Readest 的 Rust 快路只打开 ZIP、预取 OPF / nav / NCX 字节及条目大小，完整 package、TOC、CFI 和资源语义仍由 foliate-js 处理；foliate-js 自述 API 不稳定并建议固定 git submodule。Atha 已有不同的 BookRoot、Locator 和内容安全模型，迁移只会扩大边界（[Readest native parser](https://github.com/readest/readest/blob/2b719600c27b4c9c91bef7b2bb148b3251338ea7/apps/readest-app/src-tauri/src/epub_parser.rs)、[foliate-js EPUB parser](https://github.com/readest/foliate-js/blob/f65836f77e8b66b84baacd54bfc92096578e7a84/epub.js)、[foliate-js README](https://github.com/readest/foliate-js/blob/f65836f77e8b66b84baacd54bfc92096578e7a84/README.md)）。
7. **公开 fixture 应由 Atha 动态生成原创最小书，再用 EPUBCheck 5.3.0 作开发期规范 oracle。** 这不复制真实书籍，随 Atha 的 `AGPL-3.0-or-later` 公开最清楚。W3C EPUBCheck 自带 EPUB2 package / XHTML / NCX 测试且仓库是 BSD-3-Clause；如果以后复制其具体资源，必须保留声明，但本切片没有必要 vendor 上游文件（[EPUBCheck test suite](https://www.w3.org/publishing/epubcheck/docs/test-suite/)、[EPUBCheck license](https://github.com/w3c/epubcheck/blob/main/LICENSE.md)、[EPUBCheck 5.3.0](https://github.com/w3c/epubcheck/releases/tag/v5.3.0)）。
8. **现有 Android 脚本是功能链路证据，不是性能验收。** 它已覆盖系统 picker、首次打开、first-stable / ready、强停重开和日志隐私，但没有目录点击、目标 Locator 断言、峰值 PSS 或稳定性能阈值。功能门禁应补目录跳转与恢复；性能门禁应在 release-like arm64 真机上比较等价 EPUB2 / EPUB3 fixture，并保留 AVD 只做快速回归（[Android gate](../../scripts/check-android-reader.ps1)、[已接受 change](../changes/android-epub2-ncx-compatibility.md)）。

## 研究范围与证据锚点

本报告检索日期为 **2026-08-08**，只采用 EPUB 一手规范、Readest / foliate-js 固定源码、其官方依赖源码或文档，以及 Atha 当前源码。Readest 固定在 commit [`2b719600`](https://github.com/readest/readest/commit/2b719600c27b4c9c91bef7b2bb148b3251338ea7)，其 foliate-js submodule 固定在 [`f65836f7`](https://github.com/readest/foliate-js/commit/f65836f77e8b66b84baacd54bfc92096578e7a84)。

“规范事实”表示标准直接规定；“源码事实”表示固定提交或当前 Atha 文件可验证；“建议”是将这些事实投影到 Atha 现有边界的工程判断，不表示已经在 Android 真机测得性能结果。

## 规范兼容边界

### OPF2 与 OPF3 分流

OPF2 的 package 根应按命名空间解析，固定值是：

```xml
<package xmlns="http://www.idpf.org/2007/opf" version="2.0">
```

OPF2 没有需要 Atha 加入白名单的固定 package DOCTYPE；规范示例只给 XML declaration 和上述根元素。OPF 文件仍应拒绝所有 DOCTYPE、internal subset 和通用实体，这是比完整 OPF2 更窄、但与 Atha 不可信书籍模型一致的安全选择（[OPF 2.0.1 namespaces](https://idpf.org/epub/20/spec/OPF_2.0_latest.htm#Section1.3.2)、[Atha package parser](../../backend/atha-backend/src/reader/epub/package.rs)）。

EPUB2 路径只接受以下关系同时成立：

1. `package@version == "2.0"`；
2. 恰好一个 spine，且 `spine@toc` 存在；
3. 该 IDREF 唯一解析到 manifest item；
4. item 的 media type 精确为 `application/x-dtbncx+xml`；
5. href 经既有安全 resolver 后位于书根内。

这些关系来自 OPF2 对 NCX 和 spine 的直接要求；缺失 `spine@toc` 时返回 unsupported 比 foliate-js 搜索“第一个 NCX MIME”更确定（[OPF 2.0.1 spine / NCX](https://idpf.org/epub/20/spec/OPF_2.0_latest.htm#Section2.4)、[foliate-js fallback behavior](https://github.com/readest/foliate-js/blob/f65836f77e8b66b84baacd54bfc92096578e7a84/epub.js)）。

EPUB3 路径保持现状：`version="3.0"`、恰好一个 XHTML nav item、恰好一个 `toc nav`。即使 package 同时带 NCX / guide，也不读取它们，因为 EPUB3 规范明确 legacy feature 只服务 EPUB2 阅读器（[EPUB 3.3 package version](https://www.w3.org/TR/epub-33/#sec-package-elem)、[EPUB 3.3 navigation requirements](https://www.w3.org/TR/epub-33/#sec-nav-doc)、[EPUB 3.3 legacy support](https://www.w3.org/TR/epub-33/#sec-package-legacy-support)）。

### Spine、guide 与 cover

OPF2 的 `linear` 缺省为 `yes`，但规范允许 Reading System 忽略 `linear="no"` 并把全部 itemref 当作主阅读顺序。Atha 可继续保留所有 spine itemref，既不新增辅助内容状态，也保证 NCX 可以指向这些 section（[OPF 2.0.1 linear example](https://idpf.org/epub/20/spec/OPF_2.0_latest.htm#Section2.4)）。

首版只接受最终 spine 资源为 `application/xhtml+xml`；DTBook、`text/x-oeb1-document`、XML Island 和 manifest fallback chain 返回 unsupported。它们是规范允许面，但 Atha 没有对应渲染 / 清洗契约，不能仅靠 package 库变成安全可读内容（[OPF 2.0.1 spine media](https://idpf.org/epub/20/spec/OPF_2.0_latest.htm#Section2.4)、[OPS 2.0.1 core media types](https://idpf.org/epub/20/spec/OPS_2.0.1_draft.htm#Section1.3.7)）。

`guide` 最多一个且 Reading System 无须使用；首版应有界跳过，不能把它当 NCX 的替代 TOC。EPUB2 cover 只识别 legacy `<meta name="cover" content="manifest-id">`，并要求 ID 唯一解析到已声明、安全路径、受支持图片 item（[OPF 2.0.1 guide](https://idpf.org/epub/20/spec/OPF_2.0_latest.htm#Section2.6)、[EPUB 3.3 legacy cover note](https://www.w3.org/TR/epub-33/#sec-opf2-meta)）。

### NCX 结构与投影

canonical NCX 根和 DTD 标识是：

```xml
<!DOCTYPE ncx PUBLIC "-//NISO//DTD ncx 2005-1//EN"
  "http://www.daisy.org/z3986/2005/ncx-2005-1.dtd">
<ncx xmlns="http://www.daisy.org/z3986/2005/ncx/" version="2005-1">
```

这些值由 canonical DTD 固定；Atha 只把完整声明当作允许出现的字节模式，绝不访问 system identifier（[canonical NCX DTD](https://www.daisy.org/z3986/2005/ncx-2005-1.dtd)）。

规范结构是 `navMap (navInfo*, navLabel*, navPoint+)`，每个 `navPoint` 是 `navLabel+, content, navPoint*`。Atha 的投影规则应是：只消费每个 navPoint 的第一个可用文本标签和唯一 `content@src`，按文档前序写入 flat TOC；有界忽略 navMap 级 label / info 和额外 navLabel。若实现改为直接拒绝这些规范允许结构，必须把它们列为已知兼容缺口，而不能声称一般 NCX 支持（[NCX DTD content model](https://www.daisy.org/z3986/2005/ncx-2005-1.dtd)）。

`content@src` 必须经过既有 resolver，并最终指向 ReaderManifest 中的 XHTML section，可带一个 fragment。外部 URL、绝对路径、反斜杠、冒号、查询串、百分号、NUL、控制字符、多 fragment、越界路径和不存在 / 非 spine 目标均拒绝；这延续 Atha 当前比一般 URL resolver 更窄的安全契约（[OPF 2.0.1 NCX targets](https://idpf.org/epub/20/spec/OPF_2.0_latest.htm#Section2.4.2)、[Atha resolver](../../backend/atha-backend/src/reader/epub/archive.rs)）。

`pageList` 在出版物具有纸页导航时才必须提供，`navList` 可选，`guide` 可选。首版可以解析到关闭标签并有界跳过，但不能把其中条目计入主 TOC；以后产品真的展示页码 / landmarks 时再扩展 ReaderManifest（[OPF 2.0.1 NCX structures](https://idpf.org/epub/20/spec/OPF_2.0_latest.htm#Section2.4.1.2)、[OPF 2.0.1 guide](https://idpf.org/epub/20/spec/OPF_2.0_latest.htm#Section2.6)）。

### DOCTYPE、实体与 `playOrder`

声明策略应按文档类型分开：

| 文档 | 可接受声明 | 必须拒绝 | 依据 |
| --- | --- | --- | --- |
| `container.xml` / OPF | 无 DOCTYPE | 任意 DOCTYPE、internal subset、实体声明 | [OPF 2.0.1 package identity](https://idpf.org/epub/20/spec/OPF_2.0_latest.htm#Section1.3.2) |
| EPUB2 NCX | 无 DOCTYPE；或上面的精确 canonical declaration | 其他 public / system ID、internal subset、ENTITY | [OPF 2.0.1 NCX exception](https://idpf.org/epub/20/spec/OPF_2.0_latest.htm#Section2.4.1.2)、[NCX DTD](https://www.daisy.org/z3986/2005/ncx-2005-1.dtd) |
| EPUB2 XHTML | 无 DOCTYPE；精确 XHTML 1.1 declaration | internal subset、ENTITY、未知 declaration | [OPS 2.0.1 XHTML profile](https://idpf.org/epub/20/spec/OPS_2.0.1_draft.htm#Section2.1)、[XHTML 1.1 declaration](https://www.w3.org/TR/xhtml11/conformance.html) |
| EPUB2 XHTML 兼容扩展 | 可选精确 XHTML 1.0 Strict declaration | Transitional / Frameset 及其他声明 | [XHTML 1.0 Strict declaration](https://www.w3.org/TR/xhtml1/#strict) |
| EPUB3 nav | 无 DOCTYPE；精确 `<!DOCTYPE html>` | 其他声明 | [EPUB 3.3 XHTML syntax](https://www.w3.org/TR/epub-33/#sec-xhtml) |

EPUB2 OPS 2.0.1 以 XHTML 1.1 模块和 NVDL 定义内容 profile，并没有把 XHTML 1.0 Strict 声明规定为 EPUB2 固定值；因此 change 中的 XHTML 1.0 支持应标记为现实兼容白名单，而不是规范要求（[OPS 2.0.1 modules](https://idpf.org/epub/20/spec/OPS_2.0.1_draft.htm#Section2.2)）。

`playOrder` 有一个容易误实现的版本差异：带 canonical DOCTYPE 的 NCX 必须遵守 DTD，所以 navPoint 必须有 `playOrder`；没有 DOCTYPE 的 EPUB NCX 可以省略它。最小正确实现应只在 `doctype_seen` 时强制存在和合法，在无声明时把文档顺序作为唯一顺序；若无条件要求该属性，就属于明确的更窄子集（[OPF 2.0.1 playOrder exception](https://idpf.org/epub/20/spec/OPF_2.0_latest.htm#Section2.4.1.2)、[NCX DTD navPoint](https://www.daisy.org/z3986/2005/ncx-2005-1.dtd)）。

`quick-xml` 是 pull parser，不进行 DTD / NVDL / schema validation；`unescape` 也不应启用 HTML 全实体表。Atha 必须自己校验根、命名空间、层级、基数、必选属性和唯一性，只解析 XML 五个预定义实体与 numeric character reference（[`quick-xml` reader](https://docs.rs/quick-xml/0.41.0/quick_xml/reader/)、[`quick-xml` escape](https://docs.rs/quick-xml/0.41.0/quick_xml/escape/)）。

### UTF-16 是单独的兼容门槛

OPF2 要求 Reading System 正确解析 UTF-8 与 UTF-16 的 OPF、NCX 和内容文档。Atha 当前 `quick-xml` 未开启 `encoding` feature，并直接用原始字节创建 reader，实质只可靠支持 UTF-8；这必须在产品兼容声明中公开，不能悄悄算作完整 EPUB2（[OPF 2.0.1 Unicode](https://idpf.org/epub/20/spec/OPF_2.0_latest.htm#Section1.3.4)、[OPS 2.0.1 Unicode](https://idpf.org/epub/20/spec/OPS_2.0.1_draft.htm#Section1.3.6)、[Atha Cargo](../../backend/atha-backend/Cargo.toml)）。

若要覆盖 UTF-16，不需要换 EPUB 库：开启 quick-xml 的 `encoding` feature，并在现有成员字节上使用 `DecodingReader` 有界转成 UTF-8，再进入同一状态机；UTF-16LE / BE 不能直接交给普通 reader。需要用 UTF-16LE、UTF-16BE、奇数尾字节和伪造 declaration fixture 单独验收（[`quick-xml` encoding guidance](https://docs.rs/quick-xml/0.41.0/quick_xml/#features)、[`DecodingReader` source](https://docs.rs/crate/quick-xml/0.41.0/source/src/encoding.rs)）。

## 成熟实现与依赖取舍

| 方案 | 一手事实 | 对 Atha 的判断 |
| --- | --- | --- |
| 现有 `zip` + `quick-xml` | `ZipArchive` 提供条目元数据、重叠检查和流式读取；`NsReader` 提供流式事件与命名空间。Atha 已在其上叠加完整书根边界（[`zip 8.6`](https://docs.rs/zip/8.6.0/zip/read/struct.ZipArchive.html)、[`quick-xml 0.41`](https://docs.rs/quick-xml/0.41.0/quick_xml/reader/struct.NsReader.html)、[Atha archive](../../backend/atha-backend/src/reader/epub/archive.rs)） | **本切片采用。** 新增一个 version 分流和一个非递归 NCX 状态机；共享现有 caps、resolver、manifest 与错误类型。 |
| Readest Rust + foliate-js | Rust `parse_epub_full` 预取 container / OPF / nav / NCX 和 sizes，阻塞 ZIP 工作放到 blocking task；JS 再用 DOMParser / foliate 完成语义。foliate 支持 nav、NCX、guide、EPUB2 cover 与 CFI，但 README 明示 API 不稳定并要求 CSP（[Readest parser](https://github.com/readest/readest/blob/2b719600c27b4c9c91bef7b2bb148b3251338ea7/apps/readest-app/src-tauri/src/epub_parser.rs)、[Readest Cargo](https://github.com/readest/readest/blob/2b719600c27b4c9c91bef7b2bb148b3251338ea7/apps/readest-app/src-tauri/Cargo.toml)、[foliate EPUB](https://github.com/readest/foliate-js/blob/f65836f77e8b66b84baacd54bfc92096578e7a84/epub.js)、[foliate README](https://github.com/readest/foliate-js/blob/f65836f77e8b66b84baacd54bfc92096578e7a84/README.md)） | **只学习，不引入。** Readest 证明“native ZIP 快路 + 单一 JS 阅读模型”可行，但 Atha 的单一模型已经是 Rust importer → BookRoot / ReaderManifest；迁移会产生第二套定位与信任事实。 |
| `epub 2.1.5` crate | 提供 EPUB 导航 API，依赖 `xml-rs 1` 与 `zip 3`；archive source 对条目使用 `read_to_end`，没有 Atha 的成员 / 总量 / 路径 / 条目数边界；license metadata 为 GPL-3.0（[`epub` crate](https://docs.rs/crate/epub/2.1.5)、[`Cargo.toml`](https://docs.rs/crate/epub/2.1.5/source/Cargo.toml)、[`archive.rs`](https://docs.rs/crate/epub/2.1.5/source/src/archive.rs)） | **拒绝。** 不仅重复依赖，还弱化不可信 ZIP 边界；Atha 采用 AGPL-3.0-or-later 也不等于可以跳过精确依赖许可与 notice 检查。 |
| `rbook 0.7.10` | Apache-2.0；支持 EPUB2 / 3、lazy resource、manifest、spine、TOC、guide / landmarks 与读写。默认 feature 还包含 write / prelude / threadsafe；`strict(true)` 官方明确不验证完整规范（[`rbook`](https://docs.rs/rbook/0.7.10/rbook/)、[`EpubOpenOptions`](https://docs.rs/rbook/0.7.10/rbook/epub/struct.EpubOpenOptions.html)、[`Cargo.toml`](https://docs.rs/crate/rbook/0.7.10/source/Cargo.toml)） | **当前不引入；未来首选重评对象。** 只有当范围扩大到 fallback、guide / landmarks、层级 TOC 保真、写回 / 编辑或多 rendition，且自建状态机明显超过一个 deep-module adapter 时，再关闭默认 feature 做 P0；仍须把 Atha archive caps 放在它之前。 |

因此“何时需要 foliate / EPUB 库”的触发条件不是又发现一本有 NCX 的书，而是 ReaderManifest 本身需要表达当前没有的阅读模型：CFI、复杂 fallback、DTBook / XML Island、完整层级多导航、写回或多格式共享 parser。触发前继续使用现有深模块，避免同时维护 Rust 与 JS 两份 EPUB 真相（[foliate Book interface](https://github.com/readest/foliate-js/blob/f65836f77e8b66b84baacd54bfc92096578e7a84/README.md)、[`rbook` feature set](https://docs.rs/rbook/0.7.10/rbook/)）。

## 安全与资源上限

NCX parser 必须是基于事件和显式栈 / 深度计数的线性状态机，不使用 Rust 递归遍历；时间复杂度应为 `O(XML bytes + navPoint count)`，额外内存为 flat TOC、唯一性集合和当前标签。所有 XML 类型复用同一个 `MAX_XML_DEPTH`，所有 TOC 类型复用现有 `MAX_TOC_ITEMS` 与标签上限，不能为 EPUB2 悄悄开更大的第二套 caps（[`quick-xml` streaming reader](https://docs.rs/quick-xml/0.41.0/quick_xml/reader/struct.NsReader.html)、[Atha limits](../../backend/atha-backend/src/reader/epub/mod.rs)）。

必须保持以下拒绝顺序和不变量：

1. ZIP 中央目录、重叠 / 加密 / symlink、名称规范化和解压大小先于 XML 语义；
2. XML 根、版本、命名空间、深度、基数先于路径解析；
3. ID / href / playOrder 唯一性和 label 大小在加入结果前检查；
4. href 只通过既有 resolver，并要求目标属于 section 集合；
5. DTD 字符串永不触发文件、网络或 entity resolver；
6. 任一失败发生在 publish 受控书根之前，staging 可回滚。

这些顺序复用 Atha 已有 archive → package → plan → publish 边界；`zip` 或 `quick-xml` 的通用 API 本身不会替项目自动提供这些策略（[Atha importer](../../backend/atha-backend/src/reader/epub/mod.rs)、[Atha archive](../../backend/atha-backend/src/reader/epub/archive.rs)、[`ZipArchive`](https://docs.rs/zip/8.6.0/zip/read/struct.ZipArchive.html)）。

脚本、表单、网络资源和 CSS 不因为 EPUB2 获得新权限；导入成功后的 XHTML 仍走既有内容 / 搜索清洗和 CSP。DOCTYPE 兼容只应移除“已知安全声明导致无法打开”的假阴性，不得成为保留实体或主动内容的理由（[foliate CSP warning](https://github.com/readest/foliate-js/blob/f65836f77e8b66b84baacd54bfc92096578e7a84/README.md)、[Atha accepted boundary](../changes/android-epub2-ncx-compatibility.md)）。

## Fixture 与公开分发门禁

### 正向 fixture

测试代码动态生成一份原创、无第三方正文的最小 EPUB2，至少包含：stored 且首项的 `mimetype`、container、OPF2 metadata、legacy cover、两节 XHTML、`linear="no"` 节、CSS / image、嵌套 navPoint 和 fragment。它直接属于 Atha 仓库内容，可随项目 `AGPL-3.0-or-later` 公开；不提交用户本机书籍或抽取段落（[EPUBCheck EPUB2 test organization](https://www.w3.org/publishing/epubcheck/docs/test-suite/)、[Atha change](../changes/android-epub2-ncx-compatibility.md)）。

至少保留两个规范正向变体：

- canonical NCX DOCTYPE + 每个 navPoint 有合法 `playOrder`；
- 无 NCX DOCTYPE + 省略 `playOrder`。

另以 `guide`、navMap 级 `navLabel` / `navInfo`、多个 navLabel、`pageList` / `navList` 构造“有效但首版忽略”的变体，防止 parser 把可忽略的规范内容误判成恶意输入（[OPF 2.0.1 NCX exception](https://idpf.org/epub/20/spec/OPF_2.0_latest.htm#Section2.4.1.2)、[NCX DTD](https://www.daisy.org/z3986/2005/ncx-2005-1.dtd)）。

每次更改 fixture 结构时，用固定的 EPUBCheck 5.3.0 验证正向样本零 error；EPUBCheck 是开发 oracle，不进入 Android APK 或运行时依赖（[EPUBCheck 5.3.0 release](https://github.com/w3c/epubcheck/releases/tag/v5.3.0)、[EPUBCheck purpose](https://www.w3.org/publishing/epubcheck/docs/test-suite/)）。

### 负向与边界 fixture

测试矩阵至少覆盖：错误 package / NCX namespace 或 version，缺失 / 错误 `spine@toc`，错误 NCX media type，截断 XML，未知 DOCTYPE，internal subset / ENTITY，外部 / 绝对 / query / percent / traversal href，目标不在 spine，重复 id / href / playOrder，空 / 超长标签，当前最大深度与 `+1`，当前最大 TOC 条目与 `+1`，单成员最大字节与 `+1`，以及 DTBook / fallback 明确 unsupported（[OPF 2.0.1 structural requirements](https://idpf.org/epub/20/spec/OPF_2.0_latest.htm)、[Atha bounds](../../backend/atha-backend/src/reader/epub/mod.rs)、[Atha resolver](../../backend/atha-backend/src/reader/epub/archive.rs)）。

UTF-16LE / BE 是单独正向门禁：如果本切片不启用 `DecodingReader`，fixture 必须稳定返回 documented unsupported；如果启用，则两种字节序都要通过，并拒绝错误 BOM、奇数尾字节与声明 / 实际编码不一致（[`quick-xml` encoding source](https://docs.rs/crate/quick-xml/0.41.0/source/src/encoding.rs)、[OPF 2.0.1 encoding requirement](https://idpf.org/epub/20/spec/OPF_2.0_latest.htm#Section1.4.1.1)）。

W3C EPUBCheck 的测试资源可按 BSD-3-Clause 条件再分发，但最小 fixture 没有复用它们的必要。若以后选取具体上游文件，需固定 commit、核对该文件额外 notice 并保留仓库 license；不能仅凭“测试文件”三个字假设无版权义务（[EPUBCheck license](https://github.com/w3c/epubcheck/blob/main/LICENSE.md)、[test resource layout](https://www.w3.org/publishing/epubcheck/docs/test-suite/#test-files-organization)）。

## Android 功能与性能门禁

### AVD 功能门禁

将同一动态 EPUB2 fixture 写到 ignored artifact 后，交给正式入口：

```powershell
scripts/check-android-reader.ps1 -EpubPath <generated-epub2.epub> -CleanAppData
```

现有脚本已自动完成 picker、导入、打开、first-stable / ready、强停重开、cache 清理和日志隐私。完成 EPUB2 change 前还要增加：展开目录、点击嵌套 fragment 项、断言 section / Locator 或进度改变、强停后断言恢复到同一目标；否则只能证明“可打开”，不能证明 NCX 投影可用（[Android gate](../../scripts/check-android-reader.ps1)、[change acceptance](../changes/android-epub2-ncx-compatibility.md)）。

AVD 再跑现有 EPUB3 fixture，确保 version 分流没有改变 EPUB3 nav、Locator 和日志结果。外部 / traversal / entity 负向 fixture 应在导入期失败，且 AppLog 不出现书名、路径、URI、TOC 标签或正文（[Android observability boundary](../changes/android-observability-foundation.md)、[EPUB3 nav requirement](https://www.w3.org/TR/epub-33/#sec-nav-doc)）。

### 性能门禁

先生成内容、资源、spine 完全相同的一对 fixture，只让一个使用 OPF2 + NCX，另一个使用 OPF3 + XHTML nav。这样差值主要来自 package / navigation parser，而不是书籍内容。每轮只记录格式、字节数、section / TOC 数和阶段耗时，不记录私人文本（[Readest native prefetch design](https://github.com/readest/readest/blob/2b719600c27b4c9c91bef7b2bb148b3251338ea7/apps/readest-app/src-tauri/src/epub_parser.rs)、[Atha observability boundary](../changes/android-observability-foundation.md)）。

推荐记录 `package_parse_ms`、`navigation_parse_ms`、总 import、open、first-stable、ready 和峰值 PSS；release-like 构建在同一 arm64 设备、同一 Android / WebView、同一提交上先 warm up，再分别执行至少十轮冷导入 / 打开并报告 median / p95。十轮统计方式可沿用现有 reader benchmark，但 Windows 的 first-stable 750 ms 门槛不能直接当 Android 门槛（[reader benchmark](../../scripts/check-reader-slice.ps1)、[Android gate evidence](../../scripts/check-android-reader.ps1)）。

当前没有 arm64 基线，不应先编造绝对数字。第一次真实设备运行建立 baseline，随后把相同 fixture、设备档位、构建模式和允许回归幅度冻结在 change；x86_64 AVD 只负责功能 / 崩溃回归，不能被描述成 ARM 性能证据（[accepted performance boundary](../changes/android-epub2-ncx-compatibility.md)）。

另建压力 fixture：达到当前最大 TOC 条目、最大 XML 深度和接近单成员字节上限；它在 Android 上必须有界完成或稳定拒绝，无 ANR、崩溃、持续内存增长或跨轮累积。若 NCX stage 相对等价 EPUB3 持续成为主耗时，再做 profile；在出现证据前不引入缓存、DOM、线程池或新 parser 库（[Atha caps](../../backend/atha-backend/src/reader/epub/mod.rs)、[`quick-xml` streaming model](https://docs.rs/quick-xml/0.41.0/quick_xml/reader/struct.NsReader.html)）。

## 实施决策

1. 在 `backend::reader::epub::package` 内按 package version 分流，并把 XHTML nav / NCX 都归一为同一 `Vec<TocItem>`；外部 `import_epub`、ReaderManifest、BookRoot、Locator 和前端消息接口不变（[Atha package module](../../backend/atha-backend/src/reader/epub/package.rs)、[change architecture](../changes/android-epub2-ncx-compatibility.md)）。
2. EPUB2 严格使用 `spine@toc`；解析 NCX navMap，前序拍平；有界忽略 guide、pageList、navList 和替代 label；保留全部 spine itemref（[OPF 2.0.1 spine / NCX](https://idpf.org/epub/20/spec/OPF_2.0_latest.htm#Section2.4)、[NCX DTD](https://www.daisy.org/z3986/2005/ncx-2005-1.dtd)）。
3. package / container 继续拒绝 DOCTYPE；NCX 与 XHTML 只接受本文固定白名单，DTD 永不解析；无 NCX DOCTYPE 时允许省略 playOrder（[OPF 2.0.1 NCX exception](https://idpf.org/epub/20/spec/OPF_2.0_latest.htm#Section2.4.1.2)、[XHTML 1.1 declaration](https://www.w3.org/TR/xhtml11/conformance.html)）。
4. 首版明确为 UTF-8 EPUB2 XHTML 子集；若产品验收要求 UTF-16，就使用 quick-xml `DecodingReader` 扩展同一管线，不换 EPUB 库（[`quick-xml` encoding guidance](https://docs.rs/quick-xml/0.41.0/quick_xml/#features)、[OPF 2.0.1 Unicode](https://idpf.org/epub/20/spec/OPF_2.0_latest.htm#Section1.3.4)）。
5. 动态 fixture + EPUBCheck 负责规范与恶意输入门禁；AVD 负责正式 Android 功能链路；arm64 真机负责性能与峰值内存。三种证据分开陈述（[EPUBCheck suite](https://www.w3.org/publishing/epubcheck/docs/test-suite/)、[Android gate](../../scripts/check-android-reader.ps1)、[accepted change](../changes/android-epub2-ncx-compatibility.md)）。

重新评估成熟库的停止条件只有三个：现有 parser 必须实现 DTBook / fallback / 多 rendition 等完整 package 语义；ReaderManifest 需要层级多导航或 CFI；或 Android profile 证明当前状态机无法在既有 caps 内达标。届时优先用关闭默认 feature 的 `rbook` 做隔离 P0，foliate-js 只在 Atha 决定采用其完整 Book / CFI 模型时进入；`epub` crate 不作为候选（[`rbook` scope](https://docs.rs/rbook/0.7.10/rbook/)、[foliate scope](https://github.com/readest/foliate-js/blob/f65836f77e8b66b84baacd54bfc92096578e7a84/README.md)、[`epub` archive source](https://docs.rs/crate/epub/2.1.5/source/src/archive.rs)）。

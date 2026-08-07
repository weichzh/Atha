---
description: 评估 Atha 离线词典引擎、样本兼容性、Android 性能验证与版权边界。
---

# 词典引擎评估

> 调研日期：2026-08-07。本文只处理离线词典导入、查词与性能边界，不决定通用阅读格式解析方案。

## 结论

1. 不应从零实现 MOBI 的 HUFF/CDIC、INDX/INFL，也不应先建立统一转换框架。MOBI 首选用 `libmobi 0.12` 做 Android 实机 P0：它覆盖经典 MOBI、KF8、HUFF/CDIC 和词典索引，且明确支持 Android。Atha 只保留一层很薄的 Rust/FFI 边界。是否进入产品取决于样本上的内存、随机查词延迟、崩溃隔离和 LGPL 分发审查。
2. Merriam-Webster 样本不是 MOBI，而是 MDict v2 的 `.mdx + .mdd` 配对。技术上最贴近 Atha 后端的是 Rust 的 `mdict-rs`，但它在 2026 年 4 月才进入早期公开版本，许可证是 AGPL-3.0-only 或商业许可。Readest 已用其 `js-mdict` 分支证明“按需范围读取 + MDict encrypt=2 + MDD 资源”路线可行，但该代码同样受 AGPL 约束。Atha 在许可证明确前只能做隔离 P0，不能复制或链接这些实现进入产品。
3. Android 应先于桌面成为性能门槛。查词热路径必须保持“原生索引定位 → 只读目标压缩块 → 解析一个词条 → 按需取资源”，不能把约 204 MiB 的 Bing 解压文本常驻内存。优先利用操作系统页缓存、单个已打开的 reader 和有上限的块缓存；只有实测证明必要时再增加转换索引或缓存依赖。
4. 两个本地样本都有较高版权风险，且没有随文件提供的可再分发许可证。它们可以作为用户本机、显式启用、不输出正文的兼容性与性能样本；不能加入 Git、公共 CI、测试产物或日志，也不能从中截取“小样本”提交。公开测试应使用自制的结构型词条或来源及数据许可证都明确的开放词典。
5. Bing 样本只覆盖 MOBI6 经典词典，不覆盖 KF8 词典；Merriam-Webster 样本覆盖 MDict v2、encrypt=2 和配套资源。两者通过不等于“所有 MOBI/KF8/MDict 均受支持”。

本文中的“事实”来自官方文档、项目源码/文档或本机只读探针；“推断”是由这些证据得到但尚未经过 Atha 运行验证的判断；“建议”是下一阶段的实现选择。

## 本地样本证据

本节探针只读取文件头、记录表、公开元数据、大小与哈希；没有转换、解包、复制或读取词条正文。

### 样本清单

| 样本 | 实际形态 | 大小 | SHA-256 | 本机只读探针结论 |
| --- | --- | ---: | --- | --- |
| `fixtures/local/concise-bing.mobi` | 单个 Palm Database / `BOOKMOBI` 文件 | 99,081,920 B（94.49 MiB） | `BDCCCE6252118D65873BBDC02578647A7A8C16108B6062FC368004E2E8F66DC7` | MOBI6、HUFF/CDIC、Windows-1252、无 DRM、经典词典索引 |
| `Webster's Third New International Dictionary of the English Language.mdx` | MDict v2 主词典 | 39,801,145 B（37.96 MiB） | `4BC4391042D2401AF8DEFC965BDB6F21BCE807E515A0343C68C9260A432F1777` | HTML、UTF-8、`Encrypted=2`、`StripKey=Yes` |
| 同名 `.mdd` | MDict v2 资源包 | 18,472,829 B（17.62 MiB） | `E41284757741BE37043E52CACD274D9CBAD0CE8176B119853CD14AD8BB81B0D6` | 与 MDX 配套的资源容器、`Encrypted=2` |

Merriam-Webster 目录共两个文件，合计 58,273,974 B（55.57 MiB）。标题元数据显示为 *Webster's Third New International Dictionary, Unabridged*，生成/要求引擎版本均为 2.0，创建日期为 2009-12-28。这些值只是文件自报元数据，不证明来源、正版状态或授权范围。

### Bing MOBI 的结构证据

事实：

- PalmDB 名称为 `Concise_Bing_Dictionary`，类型/创建者为 `BOOKMOBI`，记录数为 53,467。
- Record 0 声明 MOBI 格式版本 6、压缩类型 HUFF/CDIC、52,436 个文本记录、每个文本记录 4,096 B，未压缩文本总长 214,196,224 B（204.27 MiB），加密类型为 0。
- 正字索引记录号为 52,437；现代 `infl_index` 字段为 `0xFFFFFFFF`，但 `names` 与 `keys` 索引均存在。
- 词典输入 LCID 为 `0x0409`（en-US），输出 LCID 为 `0x0004`（中文中性区域）；封面/资源区域与 HUFF 数据记录也存在。

推断：这是经典 MOBI6 词典，可能使用 `libmobi` 所称的旧版 v1 屈折变化重建路径，而不是现代显式 `infl_index`。它很适合暴露大记录表、HUFF/CDIC 解压、Windows-1252 解码、正字索引与旧式屈折索引问题；但它不是 KF8 覆盖证据。

### Merriam-Webster MDict 的结构证据

事实：

- MDX 与 MDD 都以 MDict v2 的 UTF-16LE XML 头开始；MDX 声明 `Format=Html`、`Encoding=UTF-8`、`Encrypted=2`，MDD 也声明 `Encrypted=2`。
- 两个文件同名配对。MDX 是键与词条数据，MDD 是图片、样式等资源的伴随容器。
- `Encrypted=2` 是候选实现所称的关键字索引混淆/加密模式，不等同于要求用户口令的词条记录加密（通常称 encrypt=1）。

推断：该样本适合验证 MDict v2、加密关键字索引、HTML 词条和 MDD 懒加载资源；目前没有在 Atha 或候选库中执行实际查词，因此不能声称已经兼容。

## 格式事实与成熟实现

### MOBI/KF8 词典

Amazon 的官方词典制作文档定义的是源内容标记：每个词条由 `<idx:entry>` 包围，查找标签用 `<idx:orth>`，不可见的屈折形式用嵌套的 `<idx:infl>` 与 `<idx:iform>`；旧的 `<idx:key>` 已被弃用。Amazon 还明确提醒，给所有词都增加大量屈折形式会扩大索引并降低查找速度。[Amazon KDP 词典指南](https://kdp.amazon.com/en_US/help/topic/G2HXJS944GL88DNV)、[Kindle Publishing Guidelines](https://s3.amazonaws.com/kindlegen/AmazonKindlePublishingGuidelines.pdf)

官方文档没有公开描述最终 MOBI 二进制中的 INDX/TAGX/ORDT 编排。对二进制兼容的判断必须以成熟开源解析器和真实样本测试为准，不能把源标记规范直接当作二进制解析规范。

`libmobi` 是当前首选候选：

- C 实现，许可证为 LGPL-3.0-or-later；0.12 版本发布于 2024-06-17。
- 项目声明支持 PalmDoc、MOBI、KF8/AZW3/AZW4、HUFF/CDIC、索引以及 Linux、macOS、Windows、Android 等目标。
- 源码/生成文档提供词典正字和屈折索引检查、词条偏移/长度导出，以及经典旧式词典屈折结构的重建路径。
- 它处理不可信二进制的边界仍在 C 内，项目历史也包含损坏输入的整数、空指针与泄漏修复。Atha 必须在 FFI 前做文件大小/记录边界限制，并把崩溃、越界与峰值内存列入 P0，而不是只验证“能打开”。

来源：[libmobi 仓库](https://github.com/bfabiszewski/libmobi)、[MOBI header 文档](https://www.fabiszewski.net/libmobi/structMOBIMobiHeader.html)、[索引实现文档](https://www.fabiszewski.net/libmobi/index_8c.html)、[导出 API](https://www.fabiszewski.net/libmobi/group__mobi__export.html)、[RawML/词典重建](https://www.fabiszewski.net/libmobi/parse__rawml_8c.html)

需要特别验证 `libmobi` 的加载模型。其 API 以 `MOBIData` 原始记录和 `MOBIRawml` 为中心，但仅凭 API 不能断言运行时只做范围读取。P0 必须记录打开阶段的实际读取字节数与 PSS；如果打开一个 94.49 MiB 文件就复制或展开绝大部分数据，则不能直接作为 Android 常驻 reader。

### MDict 与 Readest

本次没有找到格式所有者发布的 MDict v2 二进制规范。候选库的兼容性来自开源实现、测试语料和 Readest 的产品实践，因此 Atha 必须用本地样本验证，不能按扩展名承诺兼容。

Readest 的自定义词典实现提供了值得复用的架构证据：

- MDict 使用 Readest 的 `js-mdict` 分支；文件扫描器通过 `Blob.slice(...).arrayBuffer()` 做按需范围读取，并补全 encrypt=2 的 RIPEMD-128 解密路径。
- StarDict 不把整个 `.idx` 展成大量 JavaScript 字符串，而是保存紧凑字节偏移数组并二分查找；DictZip 也只解压目标块。
- Readest 随后在移动端增加系统词典集成；Android 路径使用只读的 `ACTION_PROCESS_TEXT`，可作为可选降级，但它不能替代用户导入的离线文件。

来源：[Readest 自定义 MDict/StarDict PR](https://github.com/readest/readest/pull/4012)、[Readest 系统词典 PR](https://github.com/readest/readest/pull/4219)、[Android `ACTION_PROCESS_TEXT`](https://developer.android.com/reference/android/content/Intent#ACTION_PROCESS_TEXT)

这些是“架构证据”，不是可直接复制的代码。Atha 已采用 `AGPL-3.0-or-later`，但 Readest 与上游 `js-mdict` 的精确 `-only` / `-or-later`、版权与修改说明仍需单独核对；项目采用 AGPL 不等于可以无出处复制。

`mdict-rs` 是更符合 Rust 后端的候选。项目文档声明其为 clean-room、按需读取的 MDX/MDD v2 实现，支持加密关键字索引、MDD 资源、zlib、可选 LZO、多种文本编码、边界校验和模糊测试；但 0.1.0 到 0.1.4 都发布于 2026 年 4 月，项目自称早期公开版本，许可证为 AGPL-3.0-only 或商业许可。[mdict-rs 0.1.4](https://docs.rs/crate/mdict-rs/0.1.4)、[mdict-rs API](https://docs.rs/mdict-rs/latest/mdict_rs/)

因此对 MDict 的结论是：技术方向明确，生产依赖尚未批准。`mdict-rs` 的 `AGPL-3.0-only` 与 Atha 的 `AGPL-3.0-or-later` 不能仅凭名称相近自动判定兼容；生产接入前必须完成精确组合许可判断，否则询价商业许可或重新检索可兼容且能通过本样本的实现。不能为了绕开许可证而照着 AGPL 源码重写。

### 候选矩阵

| 候选 | 覆盖与优势 | 主要限制 | 决策 |
| --- | --- | --- | --- |
| [`libmobi`](https://github.com/bfabiszewski/libmobi) | MOBI/KF8、HUFF/CDIC、词典索引、Android；项目成熟度最高 | C FFI；LGPL 分发义务；范围读取和峰值内存需实测 | MOBI Android P0 首选 |
| [`kindling`](https://github.com/ciscoriordan/kindling) | MIT Rust；可构建词典、检查 MOBI 二进制和模拟查词；覆盖 INDX/ORDT/屈折结构 | 2026 年新项目；更偏构建/验证；未证明能解析本样本的 HUFF/CDIC | 用作自制 fixture 生成器和结构 oracle，不作首版运行时 |
| [`mobi` crate](https://docs.rs/mobi/latest/mobi/) | MIT、纯 Rust、基础 MOBI 读取 | 公开实现只看到 PalmDOC 解压，未提供完整 HUFF/CDIC 词典索引查找 | 不用于 Bing 样本运行时 |
| [`KindleUnpack`](https://github.com/kevinhendricks/KindleUnpack) | 长期使用的 Python/GPL 反向工程与解包工具 | 桌面 Python 工具，不适合 Android 运行时；GPL | 仅作开发期交叉验证 |
| [`mdict-rs`](https://docs.rs/crate/mdict-rs/0.1.4) | 纯 Rust、懒读取、MDX/MDD v2、encrypt=2、安全边界 | 很新；AGPL-3.0-only 或商业许可 | 许可证获批后的 MDict P0 首选 |
| [Readest `js-mdict` 路线](https://github.com/readest/readest/pull/4012) | 已在同类阅读器实现懒读取、encrypt=2 与资源解析 | JS/Blob 边界；AGPL；不符合 Atha 后端事实所有权 | 学习架构与行为，不复制实现 |

`Calibre`/`ebook-meta` 与 KindleUnpack 可以在开发机上做元数据和导出结果的交叉检查，但不能作为移动端依赖或性能基线。工具“能完整导出”也不代表它适合低延迟、低内存的交互式查词。

## 建议的首版架构

### 导入与存储

1. Android 通过 Storage Access Framework 选择文件；后台流式复制到应用私有目录并同时计算内容哈希。复制一次可以获得稳定的随机访问和生命周期，不在每次查词时依赖外部 URI 权限或云盘提供者延迟。
2. 先检查魔数、头部、记录边界与声明大小，再相信扩展名。MDX 可以独立导入；发现同名 MDD 时作为可选资源配对，缺失资源不能让纯文本词条完全不可用。
3. SQLite 首版只保存词典注册信息：内容哈希、应用内相对路径、格式、标题、输入/输出语言、启用状态、排序、文件大小、解析器版本和导入状态。不要预先把所有正文复制到统一表，也不要先建立通用 provider 插件框架。
4. 运行时按已支持的具体格式分派，例如 `Mobi` 与 `MdxMdd`。等 StarDict 真正进入范围时再加第三个分支；此时一个薄的查词结果类型已经足够，不需要提前设计动态插件 ABI。
5. DRM MOBI、MDict encrypt=1、越界记录、解压炸弹和未知脚本/外链必须明确拒绝。失败应保留原导入文件或安全回滚临时副本，不能产生半注册词典。

若 `libmobi` 的打开阶段内存不合格，第二选择才是“导入时用成熟解析器转换”：SQLite B-tree 保存规范化词头与词条位置/正文，别名表保存屈折映射。FTS5 只在确实要做全文搜索时启用；精确词头查找不需要 FTS。是否复制或压缩正文由 Android 磁盘、导入时间和 PSS benchmark 决定。

### 查词热路径

建议的最短路径是：

1. 保留原查询，仅执行格式要求的 Unicode/大小写规范化，不做语言无关的激进词干化。
2. 使用格式原生索引做精确命中。
3. 精确未命中时查询格式原生别名/屈折结构：MOBI 的 orth/infl 或旧 names/keys、MDict 的链接/键规则、未来 StarDict 的 `.syn`。
4. 只读取并解压包含目标词条的块；解析一个结果，而不是完整词典。
5. 资源在定义真正引用时再从 MDD/MOBI 资源记录读取。
6. 返回最小结果：词典 ID、命中词头、命中类型、隔离后的定义 HTML 和受控资源定位符。

缓存按下面顺序增加：

- 首先依赖操作系统文件页缓存，并保持当前启用词典的 reader 句柄打开。
- 然后增加按“压缩块字节数”限制的解压块缓存；上限必须能响应 Android 内存压力和词典移除/版本变化。
- 最后才考虑最近结果缓存。结果缓存也按字节而不是固定条数计量，避免一篇含大量 HTML 的词条挤爆内存。
- 首版不为 LRU 单独引入依赖；小型、可测的固定容量缓存足够。只有命中率和 CPU/IO 证据表明需要时才升级策略。

禁止缓存完整 204.27 MiB 解压语料。多个词典并行查找应有取消与并发上限，翻页时产生的新选词应取消已经过期的查找。

### 定义渲染的信任边界

MDX 与 MOBI 定义都是不可信 HTML。不能把原始正文直接插入阅读器 DOM。建议复用 Atha 对不可信书籍的边界：

- 用没有 `allow-scripts` 的 sandboxed iframe 或独立受控文档渲染；设置 `default-src 'none'` 的 CSP。
- 移除脚本、表单、事件处理器、弹窗与导航；禁止网络请求。
- 把图片、CSS、音频等 URL 重写到受控的词典资源协议；后端检查词典 ID、规范化路径、MIME、单资源大小和总解压大小，拒绝 `..`、绝对路径与协议跳转。
- CSS 作用域限制在定义容器内，不能污染阅读器主题。资源失败只影响该资源，不应使词条或阅读进度崩溃。

## Android 性能验证

AndroidX Benchmark 官方文档要求在物理设备上测量；Microbenchmark 会处理 warmup、降温/重试并输出带设备信息的 JSON，Macrobenchmark 可覆盖完整用户流、帧时序和自定义 trace section。当前稳定线为 1.4.x。[Microbenchmark 概览](https://developer.android.com/topic/performance/benchmarking/microbenchmark-overview)、[编写 Microbenchmark](https://developer.android.com/topic/performance/benchmarking/microbenchmark-write)、[Macrobenchmark metrics](https://developer.android.com/topic/performance/benchmarking/macrobenchmark-metrics)、[AndroidX Benchmark releases](https://developer.android.com/jetpack/androidx/releases/benchmark)

### 设备与构建

- 至少一台 4–6 GB 内存的中低端 arm64 真机作为合并门槛；高端电脑和模拟器只用于快速回归。
- 使用 release-like、不可调试、与发布一致的 ABI/AOT 设置；记录提交、构建变体、Android、WebView、ABI、电量和温度。
- 冷启动场景在每轮前终止进程；热路径由同一进程重复查找。不要把首次复制/导入与普通查词混成一个数字。

### 场景矩阵

两个本地样本都执行以下场景，查询词由本机私有清单提供，报告只保留类别和命中布尔值：

| 场景 | 目的 |
| --- | --- |
| 首次导入/建索引 | 总耗时、吞吐、临时/最终磁盘、峰值 PSS、取消与恢复 |
| 进程冷启动后的第一次精确查词 | reader 打开、索引初始化、首次 IO 与解压 |
| 热精确查词 | 交互主路径与缓存收益 |
| 屈折/别名命中 | MOBI 旧式 inflection、MDict 链接/键规则 |
| 不存在的词 | 最坏索引扫描与错误路径 |
| 含图片/CSS 的词条 | MDD/MOBI 资源按需读取和渲染 |
| 词典头部/中部/尾部命中 | 排除只对局部记录快速的假象 |
| 两本词典交替查找 | reader 保活策略与缓存互相挤压 |

每个场景记录 p50/p95/max 墙钟时间、CPU 时间、读取字节数、分配量、PSS/RSS、缓存命中、UI frame timing，以及导入后的索引/副本大小。用 Macrobenchmark 驱动“选词 → 弹出定义”的用户流，并在 `open/index/read/decompress/resolve/sanitize/render` 阶段放自定义 trace section；Microbenchmark 只测可重复调用的解析/索引热循环。Rust/C 代码通过很薄的 JNI 测试入口进入；性能退化后再用 [Simpleperf](https://developer.android.com/ndk/guides/simpleperf.html) 定位 native CPU，用 [Perfetto](https://developer.android.com/tools/perfetto) 查看 IO、调度和跨线程等待。

### 首轮建议门槛

这些是产品预算，不是现有实现已达到的事实；第一轮基线后可在 change 中收紧，但不能因为高端设备很快而放宽：

- 中低端真机热精确查词 p95 不高于 100 ms，屈折/别名查词 p95 不高于 150 ms。
- 冷进程第一次查词 p95 不高于 500 ms；耗时更长时必须有可取消的加载状态，且不阻塞阅读器滚动。
- 单本词典打开后的额外 PSS 目标不高于 64 MiB，且不能随查询次数持续增长；任何接近完整 204.27 MiB 解压大小的常驻增长直接判定失败。
- 查词期间不得产生可感知卡顿；Macrobenchmark 的慢帧应与无查词基线相当。导入必须在后台运行并可取消。
- 损坏、截断和超大声明记录必须返回结构化错误，不能崩溃、越界读取或写到应用私有词典目录之外。

停止条件：如果 `libmobi` 在 Bing 样本上无法满足峰值内存或只为一次查词构建完整 RawML，就停止直接运行时集成，评估“导入时转换为 SQLite 偏移索引”；如果 `mdict-rs`/许可路径不能在 Merriam 样本上完成 encrypt=2 键查询和 MDD 资源读取，就停止 UI 集成并重新检索实现，不自行补写完整 MDict 解析器。

### 可观测性

发布构建默认只记录格式、匿名词典 ID/哈希短前缀、阶段耗时、读取字节、缓存命中、结果数量和结构化错误码。不要记录查询词、定义正文、完整本机路径、资源内容或用户选中的原文。开发 trace 可按显式开关增加阶段细节，但仍不得把受版权保护的正文写入 benchmark JSON、截图或 CI artifact。

## Fixture 与版权边界

### 技术可用性

| 样本 | 本地自动化 | 公共仓库/CI | 覆盖缺口 |
| --- | --- | --- | --- |
| Bing MOBI | 可做 opt-in 兼容、损坏输入衍生测试之外的性能测试；测试前核对大小/哈希 | 不可提交原文件、解包内容、派生数据库、词头/定义 dump | 不覆盖 KF8、DRM、现代显式 INFL |
| Merriam MDX/MDD | 可做 opt-in encrypt=2、HTML 与资源查找测试；两文件独立核对哈希 | 不可提交原文件、正文切片、资源、截图或派生索引 | 不覆盖 encrypt=1、MDict v1、无资源词典 |

测试入口应从未提交的本机配置或环境变量接收路径，文件不存在时明确 `skip`，不能硬编码 `C:\Users\nick\...`。日志用上表的哈希核对样本身份即可，不输出内容。即使只抽取几个词条，仍然是受保护内容的复制，不能把它伪装成“小型 fixture”。

公开的 MOBI 结构测试可以用 Kindling 从 Atha 自己编写的无版权词头/定义生成；MDict 测试优先使用所选解析库明确授权且数据来源清楚的测试资产，或使用独立许可的开放词典。上游代码的开源许可证不自动证明其仓库中第三方词典文件也可再分发，每个数据文件仍需单独确认来源。

### 法律判断的限制

以下只是工程风险边界，不是法律意见：

- 微软服务协议把 Bing Dictionary 列为涵盖服务，并限制未经授权的下载、复制、再分发和用其内容构建产品；但本机文件的真实来源与具体许可未知，因此只能把该协议作为风险信号，不能据此证明文件合法或非法。[Microsoft Services Agreement](https://www.microsoft.com/en-us/servicesagreement)
- Merriam-Webster 的站点条款说明其内容受版权保护并限制发布/分发；该网站条款也不能证明本机 2009 年 MDX 的来源或授权。[Merriam-Webster Terms of Use](https://www.merriam-webster.com/i/terms-of-use)
- 拥有一个文件副本不等于拥有内容版权；向公共仓库上传还会产生复制与分发。GitHub 要求上传者拥有发布内容所需权利，且公开仓库会授予 GitHub 与其他用户查看、复制和 fork 所需权利。[美国版权法第 202 条](https://www.copyright.gov/title17/92chap2.html)、[美国版权局数字版权 FAQ](https://www.copyright.gov/help/faq/faq-digital.html)、[GitHub Terms of Service](https://docs.github.com/en/site-policy/github-terms/github-terms-of-service)

因此，在没有来源证明和明确测试/再分发授权前，两个样本都只能保留在用户控制的本机位置并由测试显式引用。允许提交的只有不还原内容的结构元数据、哈希、错误类别和聚合性能数字。

## 实施顺序与未决项

1. Atha 项目许可证已定为 `AGPL-3.0-or-later`；下一许可门槛是 LGPL 动态 / 静态链接材料与 `AGPL-3.0-only` / 商业依赖的精确组合边界，未解决时只做隔离实验，不把候选源码并入主产品。
2. 为 `libmobi` 建立最薄 Android P0：只做打开、精确查词、旧式屈折查词和词条范围读取；用 Bing 样本执行上述 benchmark，并对损坏头/截断记录做无正文的合成测试。
3. 为获准的 MDict 候选建立独立 P0：验证 Merriam 样本的 encrypt=2 键索引、精确/不存在查询、一个 MDD 资源和内存曲线。没有许可证或样本失败时不进入 UI。
4. 两个 P0 达标后再接入统一的最小查词结果和安全定义视图；最后增加字典排序、启停和可选 Android 系统词典动作。
5. 另行取得可公开再分发的 KF8 词典 fixture，或用自有内容生成；没有该证据时产品声明必须写“经典 MOBI6 词典”，不能笼统写“MOBI/KF8 全支持”。

仍需真实验证的事项包括：`libmobi` 在本样本上的 RSS/读取模型、其 Android NDK 构建与异常边界、Merriam 样本是否包含候选库不支持的压缩块、MDD 的实际资源 MIME 与大小分布、以及 Android WebView 中定义 CSS 的隔离效果。代码阅读、桌面元数据读取和 Readest 的成功经验都不能替代这些真机结果。

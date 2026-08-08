---
description: 评估 Atha 离线词典引擎候选、样本兼容性、模块边界与性能验证方案。
---

# 离线词典引擎评估

> 调研更新：2026-08-09。本文只处理 Kindle 词典与 MDict 的技术选型，不决定在线词典、系统词典或通用阅读格式解析方案。

## 执行结论

1. **MDict 首个真实切片采用 `mdict-rs 0.1.4`。** 本机 P0 已覆盖匿名 MDict v2、`encrypt=2`、三次精确查词、miss 和 MDD 范围流式读取，性能与内存明显低于首轮预算；实施时直接使用这一个固定依赖，不再增加 provider 接口、工厂或 sidecar。
2. **Kindle 词典没有一个可以原样接入且同时满足成熟度、随机访问和移动端内存目标的现成库。** `boko 0.5.0` 已能解析普通 MOBI/KF8 和 HUFF/CDIC，但没有实现词典 `orth/infl/names/keys` 语义；`libmobi 0.12` 的词典覆盖最成熟，却以完整 RawML 重建为中心，已不符合当前样本的交互式查词热路径。首个 Kindle 切片应在 Atha 中建立独立、最小的解析模块，复用既有 PalmDB 边界检查，并只借鉴成熟实现中的 INDX/TAGX 与 HUFF/CDIC 算法。
3. **成熟度问题通过窄范围与真实预算控制，不通过抽象层掩盖。** MDict 先只承诺当前样本证明的 v2 路径；Kindle 先只承诺经典 MOBI6 词典的精确查词。Readest、GoldenDict-ng 和 `libmobi` 是固定版本的行为 oracle，不作为新的运行时栈。许可证只记录，不作为本项目的淘汰条件；决定因素是正确性、范围读取、内存和维护风险。
4. **日常回归以 Linux GUI 为主，PCT-AL10 是后续真实设备性能门槛。** 2026-08-09 已恢复该机的 ADB 访问，当前只证明 USB/ADB 链路可用，尚没有应用安装、查词或 PSS 通过结论。Linux 结果仍不能代替 ARM 真机验收。
5. **本地词典样本只作为匿名、显式启用的私有 fixture。** 文档、日志、截图、benchmark 产物和公共 CI 不记录原文件名、路径、哈希、查询词、正文或资源内容。

本文区分四类证据：**官方事实**来自项目固定版本的源码、包元数据或官方文档；**源码观察**是对固定源码的直接阅读；**当前实测**来自 Atha 本机探针；**建议**是尚待实现和验证的工程选择。

## 当前样本与证据边界

| 匿名样本 | 当前实测 | 能证明什么 | 不能证明什么 |
| --- | --- | --- | --- |
| `KINDLE-D` | MOBI6、HUFF/CDIC、Windows-1252，存在正字索引 | 经典 Kindle 词典头部探测和旧式索引路径需要被覆盖 | KF8、DRM、现代显式 INFL 或实际查词已兼容 |
| `MDX-A` / `MDD-A` | MDict v2，`encrypt=2`，主词典与资源包配对 | v2 加密关键字索引和资源解析是首个 MDict 验收面 | MDict v1、口令/记录加密或所有压缩组合 |

已有性能证据也必须正确解读：

- `KINDLE-D` 的格式早期拒绝路径做过 10 次运行，P95 为 3.1 ms、RSS 为 6148 KiB。这只证明分派和拒绝足够便宜，不是查词性能。
- 既有普通书籍模型曾把该词典展开为约 130 万节并达到约 1.7 GiB 内存；`libmobi` 完整源码重建在 120 秒观察窗口内没有完成。两项结果都支持“不能先展开整本词典”，但不代表最小随机访问解析一定达不到目标。
- `mdict-rs 0.1.4` 的 Linux x86_64 release P0 做了 10 次 open 和 20 次匿名查询：MDX open P50/P95 为 0.127/0.462 ms，三次精确查词 P50/P95 为 1.583/1.653 ms；MDD open P50/P95 为 0.027/0.074 ms，三次资源流式读取 P50/P95 为 0.901/1.452 ms；进程峰值 RSS 为 3308 KiB。该结果足以进入产品实现，但不是 Linux GUI 或 Android 验收。
- PCT-AL10 的 ADB 链路已经可用，尚未执行安装、查词、PSS、Perfetto 或 UI 链路验证。

## Readest 现状

调研固定在 Readest `v0.12.1` 对应提交 `f3e1df7`，不把浮动主分支当作可复现证据。[Readest 固定源码](https://github.com/readest/readest/tree/f3e1df7e0572c0119cbb420e1e27ca9af859f91c)

### 现有能力

**官方事实：** Readest 当前词典服务已包括内置网络词典、Wikipedia、Wiktionary、系统词典，以及用户导入的 StarDict、DICT、SLOB、BGL 和 MDict。它不是只做 MDX 的单一实现。[词典服务](https://github.com/readest/readest/blob/f3e1df7e0572c0119cbb420e1e27ca9af859f91c/apps/readest-app/src/services/dictionaries/dictionaryService.ts)

**源码观察：** MDict provider 使用 Readest 自己的 `js-mdict` 分支，并以 `MDX.create(blob, { lazy: true })` 打开主词典。该分支固定在提交 `d01bf62`，包版本为 `7.0.0`，核心依赖很少。[Readest MDict provider](https://github.com/readest/readest/blob/f3e1df7e0572c0119cbb420e1e27ca9af859f91c/apps/readest-app/src/services/dictionaries/providers/mdictProvider.ts)、[`js-mdict` 固定源码](https://github.com/readest/js-mdict/tree/d01bf62af872b1fbeacb2f18446460960e7400de)

其最值得 Atha 借鉴的是读取模型，而不是语言或框架：

- 文件扫描器通过 `Blob.slice(start, end).arrayBuffer()` 做有界范围读取；MDX lazy 模式避免打开时解码和排序所有 key block。[文件扫描器](https://github.com/readest/js-mdict/blob/d01bf62af872b1fbeacb2f18446460960e7400de/src/file-scanner.ts)、[MDict 基础实现](https://github.com/readest/js-mdict/blob/d01bf62af872b1fbeacb2f18446460960e7400de/src/mdict-base.ts)
- lazy 模式只适合精确定位；前缀、包含、模糊、建议和全量枚举需要 eager 数据。Atha 首版因此不应承诺模糊检索或全文检索。[MDX API](https://github.com/readest/js-mdict/blob/d01bf62af872b1fbeacb2f18446460960e7400de/src/mdict.ts)
- Readest 对 MDX 使用 lazy，但 MDD 保持 eager。源码说明这是为避免 MDD 键排序和 JavaScript `localeCompare` 不一致导致资源漏查。该选择会把 MDD 初始化时间与内存重新带回风险面，必须在 PCT-AL10 上单独测量。[Readest MDict provider](https://github.com/readest/readest/blob/f3e1df7e0572c0119cbb420e1e27ca9af859f91c/apps/readest-app/src/services/dictionaries/providers/mdictProvider.ts)
- Readest 支持多个 MDD、CSS、图片、常见音频以及 `entry://`、`bword://` 链接重写。Atha 首版只需精确查词、条目链接和实际引用的图片/CSS，不要一次复制全部能力。
- Readest 有 MDict 初始化诊断测试，但诊断结果本身不是跨设备性能门槛。[初始化诊断测试](https://github.com/readest/readest/blob/f3e1df7e0572c0119cbb420e1e27ca9af859f91c/apps/readest-app/src/__tests__/services/dictionaries/mdict-init-diagnostic.test.ts)

Readest 的 HTML 边界不应照搬。它会把定义写入 `innerHTML`，再修正部分资源和点击行为；Atha 应在后端或独立净化层删除脚本、事件属性、表单和外部网络地址，并只暴露受控资源定位符。格式解析成熟不等于词条 HTML 可信。

## Kindle 词典候选

### `boko 0.5.0`

**官方事实：** Atha 当前固定的 `boko 0.5.0` 源码提交为 `8f412fb`。它包含 PalmDB/MOBI header、通用 KF8 `INDX/TAGX/CNCX` 和 HUFF/CDIC 实现。[固定源码](https://github.com/zacharydenton/boko/tree/8f412fb1a507399bce320d591feb517467cdb5f7)、[crates.io 元数据](https://crates.io/crates/boko/0.5.0)

**源码观察：** `MobiHeader` 没有暴露词典 `orth/infl/names/keys` 字段，索引代码服务于 skeleton、division、NCX 和 guide，并没有词典正字与屈折语义。[MOBI header](https://github.com/zacharydenton/boko/blob/8f412fb1a507399bce320d591feb517467cdb5f7/src/mobi/headers.rs)、[索引实现](https://github.com/zacharydenton/boko/blob/8f412fb1a507399bce320d591feb517467cdb5f7/src/mobi/index.rs)、[HUFF/CDIC](https://github.com/zacharydenton/boko/blob/8f412fb1a507399bce320d591feb517467cdb5f7/src/mobi/huffcdic.rs)

**决策：** 继续让 `boko` 负责普通书籍，不向其普通章节模型塞入词典。后续词典适配器可以复用 Atha 已有的文件边界和 HUFF/CDIC 能力，但词典索引必须走独立路径。

### `libmobi 0.12`

**官方事实：** `libmobi 0.12` 固定提交为 `85dcfe8`。它的 `index.c` 明确覆盖 INDX/TAGX/ORDT、orth 位置与长度、现代和旧式屈折结构以及屈折规则重建，并暴露词典词条偏移与长度。[固定源码](https://github.com/bfabiszewski/libmobi/tree/85dcfe803fc2a21020ddcf15c3eb66b93d388add)、[词典索引实现](https://github.com/bfabiszewski/libmobi/blob/85dcfe803fc2a21020ddcf15c3eb66b93d388add/src/index.c)、[索引结构](https://github.com/bfabiszewski/libmobi/blob/85dcfe803fc2a21020ddcf15c3eb66b93d388add/src/index.h)、[RawML 解析](https://github.com/bfabiszewski/libmobi/blob/85dcfe803fc2a21020ddcf15c3eb66b93d388add/src/parse_rawml.c)

**判断：** 它是当前最成熟的 Kindle 二进制行为 oracle，但不是首选运行时依赖。现有全量重建实验已经触发停止条件；直接 FFI 还会引入 C 内存安全、崩溃隔离和跨平台构建面。更稳妥的路线是以其固定源码为主要算法参考，只实现样本要求的窄路径。

### KindleUnpack 与 Kindling

KindleUnpack 是长期使用的 Python 解包实现，适合交叉验证 INDX、ORDT 和现代 inflection；但其源码明确拒绝旧式 tag `0x07` 屈折方案，不能单独覆盖 `KINDLE-D` 暗示的旧 `names/keys` 路径。[KindleUnpack 固定源码](https://github.com/kevinhendricks/KindleUnpack/tree/bf0ca6ece4e73494625e7950be3e259b6260774c)、[词典实现](https://github.com/kevinhendricks/KindleUnpack/blob/bf0ca6ece4e73494625e7950be3e259b6260774c/lib/mobi_dict.py)

Kindling 是较新的 Rust 词典构建器和 Kindle 查词模拟器。它的 ORDT、排序和多文字测试很有价值，也适合生成 Atha 自有的开放 fixture；但它构造的是正字别名路线，查词模拟读取整文件并收集索引，不是通用旧 Kindle 词典运行时。[Kindling 固定源码](https://github.com/ciscoriordan/kindling/tree/828e32ec18c3e9e25864b38ac219ca6cc2b5a57b)、[查词模拟](https://github.com/ciscoriordan/kindling/blob/828e32ec18c3e9e25864b38ac219ca6cc2b5a57b/src/lookup.rs)

### Kindle 决策矩阵

| 候选 | 格式成熟度 | 运行时模型 | 移动端风险 | 用途 |
| --- | --- | --- | --- | --- |
| `boko 0.5.0` | 普通 MOBI/KF8 成熟，词典语义缺失 | 普通书籍展开 | 对大词典内存不合格 | 复用已有基础能力，不直接查词 |
| `libmobi 0.12` | 词典覆盖最完整，含旧式 inflection | 偏完整解析/RawML | 全量重建、C FFI | 主行为 oracle 和窄算法来源 |
| KindleUnpack | 解包与现代 inflection 成熟 | 桌面 Python 全量工具 | 不适合嵌入，旧 tag 7 缺失 | 交叉验证 |
| Kindling | 构建、排序和结构测试有价值 | 构建侧/模拟器 | 不是通用解析器 | 开放 fixture 与排序 oracle |

**后续实施建议：** 只有在真正开始词典引擎 change 时，才新增 `KindleDictionary`。首个实现范围限制为 `KINDLE-D` 已证明的 MOBI6、orth、旧 names/keys、HUFF/CDIC 和 Windows-1252；一次查词只定位索引、读取覆盖目标定义的文本记录并按需解压。不要先实现 KF8、DRM、全文搜索、通用词形学或整本转换。

## MDict 候选

### `mdict-rs 0.1.4`

**官方事实：** 候选是 `Initsnow/mdict-rs`，固定提交 `d4bc67d`，不是同名的旧 Web 项目。0.1.4 是纯 Rust、禁止 unsafe 的 MDict v2 reader，支持 MDX/MDD、`encrypt=2`、按需查找、zlib、可选 LZO、常见文本编码和边界校验；明确不支持 v1、写入器、HTML/CSS 重写或持久化 sidecar。[固定源码](https://github.com/Initsnow/mdict-rs/tree/d4bc67d1128e9561a27b714f085ad970dfed6c09)、[包元数据](https://docs.rs/crate/mdict-rs/0.1.4)、[解析核心](https://github.com/Initsnow/mdict-rs/blob/d4bc67d1128e9561a27b714f085ad970dfed6c09/src/core.rs)、[资源限制](https://github.com/Initsnow/mdict-rs/blob/d4bc67d1128e9561a27b714f085ad970dfed6c09/src/limits.rs)

**当前实测与决策：** 它与当前匿名样本的功能交集最好，接入成本最低。独立 release P0 已通过 MDict v2、`encrypt=2`、精确查词、miss 和 MDD 资源范围读取，并以 1.653 ms 的三次查词 P95 和 3308 KiB 峰值 RSS 留出足够预算。因此首个真实切片锁定 `mdict-rs 0.1.4`；适配层只补 Atha 更紧的文件、压缩块、解压块、条目和资源预算，不 fork、不建 sidecar。

### Readest `js-mdict`

Readest 的固定分支已经在同类阅读器中采用 lazy MDX、`encrypt=2`、多 MDD 与资源重写，是最接近 Atha 产品场景的行为基线。它的问题不是“JavaScript 一定慢”，而是 MDD eager 策略、主线程调度和 Blob/JS 对象分配必须在目标设备实测。Atha 没有必要为了复用它而引入第二套 JS 后端；它更适合作为同一匿名查询清单的交叉结果 oracle。

### GoldenDict-ng 与 Medict

GoldenDict-ng 固定在 `v26.9.0_alpha` 对应提交 `8e1079d`。其 MDict 代码长期维护，覆盖 v1/v2、`encrypt=2`、无压缩/LZO/zlib、Adler32 校验和多个 MDD，并在导入时建立持久 B-tree sidecar，查询时只解压目标 record block。[GoldenDict-ng 固定源码](https://github.com/xiaoyifang/goldendict-ng/tree/8e1079d781c41c5efab64c304109504aecb2b3a4)、[MDict parser](https://github.com/xiaoyifang/goldendict-ng/blob/8e1079d781c41c5efab64c304109504aecb2b3a4/src/dict/mdictparser.cc)、[索引与资源实现](https://github.com/xiaoyifang/goldendict-ng/blob/8e1079d781c41c5efab64c304109504aecb2b3a4/src/dict/mdx.cc)

它是最成熟的 MDict 兼容性与索引架构参考，但代码深度依赖 Qt、GoldenDict B-tree、折叠规则和自身存储层。直接嵌入或大规模移植都会扩大 Atha 的维护面。只有 lazy 原文件读取在真机上不合格时，才借鉴其“导入时生成紧凑 sidecar，正文仍留在原块”的模式。

Medict 固定提交 `04f572a`，拥有 Go MDict parser、MDX/MDD 资源服务和 LevelDB 索引，是另一份可交叉验证的成熟产品实现；但 Go runtime、LevelDB 和其内存模型都不适合直接进入 Atha 的 Rust/Tauri 核心。[Medict 固定源码](https://github.com/terasum/medict/tree/04f572a6258997125d6382486598e4c7d5018ea7)、[Go MDict 实现](https://github.com/terasum/medict/tree/04f572a6258997125d6382486598e4c7d5018ea7/internal/libs/go-mdict)

### MDict 决策矩阵

| 候选 | 成熟度 | 性能架构 | 接入代价 | 决策 |
| --- | --- | --- | --- | --- |
| `mdict-rs 0.1.4` | 新、实现窄、安全边界清楚 | lazy，单 key/record block 缓存 | 最低，纯 Rust | P0 已通过，首个产品适配器 |
| Readest `js-mdict 7.0.0` | 已有同类产品使用 | MDX lazy、MDD eager | 需要 JS 后端与额外隔离 | 行为和结果 oracle |
| GoldenDict-ng | 长期维护，覆盖最广 | 持久 B-tree + 目标块解压 | Qt/C++ 依赖很深 | 兼容性和 sidecar 架构 oracle |
| Medict | 产品级 Go 实现 | LevelDB 索引 | Go/LevelDB 运行时过重 | 辅助交叉验证 |

**实施顺序：** 直接把 `mdict-rs 0.1.4` 接入后端，完成精确查词、miss、链接深度限制、定义净化和实际引用的 MDD 资源；Linux GUI 通过后再跑 PCT-AL10。只有真机证据显示原文件 lazy 读取不合格时，才重新评估 GoldenDict 风格 sidecar。

## 最小产品边界

首个产品切片只实现已经有真实需求和样本证据的行为：

- 不返回原始文件路径，不让前端自行读取 MDX/MDD/MOBI。
- 查词结果只包含词典 ID、命中词头、命中类型、净化后的定义和受控资源引用。
- MDict 直接调用 `mdict-rs`；Kindle 使用独立解析模块。上层用一个静态 `match` 分派即可，不定义只有一个实现的 trait。
- 打开、查词和资源读取都有明确预算；实现不得隐藏全量预加载。
- 首版不做动态插件 ABI、provider 注册表、通用转换流水线、全文索引、模糊查找或跨词典并行搜索。

这已经足以完成“导入词典 → 阅读器选词 → 显示安全定义”的端到端链路。未来格式出现时再增加静态枚举分支，不为未知需求预留框架。

## 性能与验证计划

### Linux GUI 日常回归

Linux 是开发主入口。真实引擎开始实施后，每次变更先跑 release 构建的微基准，再在 Linux Tauri/WebKitGTK 中验证“选词 → 弹出定义 → 打开条目链接/资源”。记录匿名场景，不记录查询内容。

| 场景 | 记录指标 |
| --- | --- |
| open | 耗时、读取字节、RSS 增量、是否建立 sidecar |
| cold exact | 第一次精确命中的 p50/p95、CPU、块读取 |
| warm exact | 重复与随机词头的 p50/p95、缓存命中 |
| inflection/alias | 命中类型、p50/p95、额外索引读取 |
| miss | 最坏索引查找时间与读取量 |
| resource | 图片/CSS 单资源延迟、字节预算、MIME |
| alternating | 两本词典交替查询后的缓存与 RSS |

GUI 检查同时覆盖定义净化、禁网、主题隔离、滚动、键盘/鼠标选择、窗口窄宽状态、控制台与网络错误。Linux 的结果用于快速发现回归，不作为 Android 性能结论。

### PCT-AL10 真机门槛

PCT-AL10 已经恢复 ADB 访问。使用 release arm64 构建，在固定电量、温度和屏幕状态下执行同一匿名场景清单。至少记录冷/热精确查词、旧式 inflection、miss、MDD 资源和词典交替，并使用 `dumpsys meminfo`；出现退化时再用 Perfetto 或 Simpleperf 定位，不先堆测量依赖。

首轮预算是待实测校准的产品目标：

- 热精确查词 P95 不高于 100 ms；inflection/alias P95 不高于 150 ms。
- 冷进程第一次查词 P95 不高于 500 ms。
- 打开单本词典后的额外 PSS 不高于 64 MiB，连续查词不得单调增长。
- 查词只读取目标索引和压缩块；任何接近整本解压文本的常驻展开都直接失败。
- 选词到弹窗不得阻塞阅读滚动，过期请求可取消，定义渲染不得发起网络请求。

停止条件也应提前固定：

- Kindle 适配器若需要完整 RawML、完整正文转换或无法有界读取，则停止当前运行时方案。
- `mdict-rs` 若无法正确覆盖 `MDX-A/MDD-A` 的 `encrypt=2`、资源与连续内存曲线，先限定修复；连续两轮仍不达标再评估 sidecar。
- 任一候选若需要在同一 change 中扩展到未取样的 MDict v1、DRM、记录加密、KF8 或全文搜索，则缩回范围并重新立项。

## 安全、隐私与 fixture

词典是外部不可信输入，解析和渲染边界必须同时成立：

- 文件头、记录表、索引偏移、压缩/解压大小、词条大小、资源大小和递归链接深度都有显式上限。
- MDX/MOBI 定义先净化，再进入无脚本、无表单、无外网的隔离视图；CSS 只能作用于定义容器。
- `entry://` 和资源引用只解析到当前词典或明确配对的资源包，拒绝绝对路径、`..`、协议跳转和未知 MIME。
- 日志只记录格式、匿名 source ID、阶段耗时、读取字节、缓存命中和错误码，不记录查询词、定义正文、原文件名、完整路径或资源内容。

私有 fixture 通过未提交的本机配置显式启用，缺失时测试应明确 `skip`。公共测试使用 Atha 自有文本生成的结构型 Kindle 词典，以及数据许可明确的 MDict fixture；不能从私有样本切出词条、图片、CSS 或派生数据库提交到仓库。

## 最终选择

当前路线的正确收束是：**接入通过 P0 的 `mdict-rs 0.1.4`，并为经典 Kindle 词典实现独立的最小随机访问模块；不造 provider 框架，不启动 Android 模拟器。**

未来实际实施时按以下顺序推进：

1. MDict：直接依赖已经通过样本 P0 的 `mdict-rs 0.1.4`，完成精确查词与 MDD 按需资源链路。
2. Kindle：以 `libmobi 0.12` 为主 oracle，`KindleUnpack` 和 Kindling 交叉验证，在 Atha 独立模块中实现样本驱动的最小随机访问路径。
3. Linux GUI 完成功能与回归验证后，再在 PCT-AL10 做性能和内存验收；ADB 可用不等于 Android 应用已经通过。

这种顺序优先复用成熟知识，避免把不适合移动端的完整库硬塞入运行时，也避免在证据不足时重造两个完整格式。

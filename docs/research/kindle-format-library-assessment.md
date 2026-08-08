---
description: 评估 Atha 导入 MOBI、AZW、KF8 与 AZW3 时可复用的解析器、信任边界、样本与性能门。
---

# Kindle 格式解析库评估

## 结论

截至 2026-08-08，**没有一个现成实现可以单独成为 Atha 的 Kindle 信任边界，但也没有证据支持复制整套 parser**。固定本地样本 benchmark 已经把候选收敛为：

1. [`boko 0.5.0`](https://crates.io/crates/boko/0.5.0)：Rust 原生，已经实现经典 MOBI、PalmDOC、HUFF/CDIC、纯 KF8、MOBI/KF8 combo、FDST/SKEL/FRAG、INDX/CNCX/NCX、CSS、图片和字体。两个普通 KF8 样本都成功重构，10 次热缓存全量投影 P95 分别为 213 ms 和 21 ms，峰值 RSS P95 分别约为 25 MiB 和 13 MiB；但版本年轻、公开 MOBI/KF8 样本覆盖还小，现有总解压预算由整个文件大小放大，且未知压缩会按原始字节继续处理。它适合作为 Atha 专用实现的 Rust 底稿，不能原样进入信任边界。
2. [`libmobi 0.12`](https://github.com/bfabiszewski/libmobi/tree/85dcfe803fc2a21020ddcf15c3eb66b93d388add)：格式覆盖和维护历史更成熟，两个普通 KF8 样本的全量源码重构 P95 分别为 94 ms 和 29 ms；但 99 MiB HUFF 词典压力样本 120 秒仍未完成。它继续作为 parity 与性能 oracle，不承担 Atha 运行时的 C FFI、系统库、内存边界和跨平台成本。

Readest 的真实生产路径也不是 `mobi 0.8.x` 全功能 Rust reader。固定的 [Readest v0.11.18](https://github.com/readest/readest/releases/tag/v0.11.18) 在阅读时调用其生产 fork `foliate-js`；Rust [`mobi = "0.8"`](https://github.com/readest/readest/blob/4af203755d807ae317c9ffe4922f5f1e7989a66b/apps/readest-app/src-tauri/Cargo.toml) 只做部分指纹和封面快速路径。Readest 的采用证明了 foliate 路线具有真实产品覆盖，但固定 fork 的 README 同时明确说 API 尚未稳定，MOBI6 会一次解压全部文本，KF8 的 HUFF/CDIC 实现仍可能慢。因此，**生产采用度不是性能结论，必须在 Atha 的指定本地样本上测量。**

最终采用固定 `boko = 0.5.0`、`default-features = false` 加一个具体的 `reader::kindle` adapter，不复制或 fork 5.9 万行上游源码，不引入通用格式工厂，也不在 WebView 中运行 foliate 或保留第二套书籍模型。Atha 在调用依赖前独立检查源大小、PDB record、MOBI version、compression、encoding、encryption、词典索引和 HUFF 预算，因而消除了本地样本暴露的未知压缩放行与词典膨胀路径；依赖只恢复经过验证的 PalmDOC / MOBI6 与纯 KF8 内容，结果再投影到现有 `ReaderManifest` / `BookRoot` / `Locator`。只有当 `boko` 的公共能力实测阻塞必要保真或性能时才最小 fork；当前唯一确认的限制是 raw API 无法读取 KF8 flow stylesheet，首版删除对应 link 而不发布悬空 CSS。日常功能和性能入口使用 Linux Tauri / WebKitGTK；Android 模拟器保持关闭，Android 只在发布前或移动端专项时使用 ARM64 真机验证。

## 固定本地样本基准

### 方法与证据边界

本轮只登记匿名结构标签，不记录文件名、标题、正文、路径或内容哈希：

- `KF8-A`：17,751,173-byte、PalmDOC 压缩、纯 KF8；
- `KF8-B`：2,990,792-byte、HUFF/CDIC 压缩、纯 KF8；
- `HUFF-D`：99,081,920-byte、HUFF/CDIC、MOBI6 词典压力样本，声明正文 214,196,224 bytes。

普通 KF8 使用同一 Linux x86_64 主机和热页缓存，每个候选先预热一次，再运行 10 次；报告中位数、nearest-rank P95、峰值 RSS 和失败数。`boko` 使用 `default-features = false` 的 release build，顺序读取全部 spine 与 assets；`libmobi` 使用固定 `0.12` release build，关闭外部 zlib/libxml2 后重构全部源码。两者都覆盖全量投影，但输出模型并不相同，因此结构 parity 仍需单独判断。

Readest 对照固定为 `v0.11.18` 对应的 foliate fork `ba57ec8`。本轮只用 Node 24 与 `linkedom` 驱动相同 `mobi.js`，测 open 与首/中/末 section，不是 WebKitGTK，也没有遍历全部资源；它只能定位 JavaScript parser 的数量级，不能与前两路全量投影直接排名，更不能冒充 Linux GUI 验收。

### 普通 KF8 结果

| 样本 | 路径 | 工作量 | 时间 median / P95 | RSS median / P95 | 结构计数 | 失败 |
| --- | --- | --- | --- | --- | --- | --- |
| `KF8-A` | `boko 0.5.0` | 全部 spine + assets | 208 / 213 ms | 25,412 / 25,608 KiB | 1 spine、0 TOC、405 assets | 0/10 |
| `KF8-A` | `libmobi 0.12` | 全部源码重构 | 87 / 94 ms | 33,772 / 33,916 KiB | 410 输出文件 | 0/10 |
| `KF8-A` | Readest foliate | open + 1 个可用 section | 1,037 / 1,058 ms | 333,818 / 344,976 KiB | 1 section、0 TOC | 0/10 |
| `KF8-B` | `boko 0.5.0` | 全部 spine + assets | 19 / 21 ms | 12,646 / 12,912 KiB | 205 spine、204 TOC、98 assets | 0/10 |
| `KF8-B` | `libmobi 0.12` | 全部源码重构 | 26 / 29 ms | 7,536 / 7,616 KiB | 126 输出文件 | 0/10 |
| `KF8-B` | Readest foliate | open + 首/中/末 section | 175 / 179 ms | 118,676 / 136,636 KiB | 25 sections、204 TOC | 0/10 |

`KF8-B` 的 205、126 和 25 不是简单的对错投票：三者分别暴露重构 parts、源码文件和呈现 skeleton。下一步必须比较目录目标、选择章节的文本摘要、资源引用和内部链接图，不能用 section 数相同作为正确性判断。`KF8-A` 的正文集中为一个 spine，三路结构计数没有暴露明显冲突。

### 词典压力结果

`HUFF-D` 只做一次有上限的停止探针，没有重复制造 1 GiB 以上分配：

| 路径 | 结果 | 峰值 RSS | 停止判断 |
| --- | --- | --- | --- |
| `boko 0.5.0` | open 3,161 ms；生成 1,335,577 spine；全量输出 498,864,071 bytes | 1,790,344 KiB | 不能作为词典查词或普通 reader 模型 |
| Readest foliate | open 60,430 ms；生成 1,335,714 sections | 1,548,160 KiB | 不能作为词典查词或 Atha 导入路径 |
| `libmobi 0.12` | 120 秒超时，未产生输出文件 | 未生成可靠数值 | 停止，不扩大超时掩盖结构问题 |

三路都把词典条目边界当成书籍分段，证明 `HUFF-D` 必须进入独立的 MOBI dictionary 索引/查词切片，而不是提高 reader section 上限。Kindle 阅读 importer 应识别词典结构并稳定拒绝作为普通书导入；后续词典引擎必须按索引定位少量候选词条，不能先展开整本词典。

### 被淘汰候选的本地证据

`mobi 0.8.0` 能快速读取三个样本的 header，但两个 KF8 正文都未通过严格解码：`KF8-A` 在 PalmDOC 路径遇到非法 UTF-8，`KF8-B` 的 HUFF 路径发生字典索引越界并向 stderr 输出大段内部字典。它既缺失 KF8 重构，也不满足固定、无内容诊断日志要求，因此不再进入产品或性能候选。

## 研究边界与证据

本评估没有把 README 的格式清单当成实现事实。版本和源码锚点如下：

| 对象 | 固定版本或提交 | 本次检查的主要证据 |
| --- | --- | --- |
| Readest | [`v0.11.18` / `4af2037`](https://github.com/readest/readest/commit/4af203755d807ae317c9ffe4922f5f1e7989a66b) | [文档分派](https://github.com/readest/readest/blob/4af203755d807ae317c9ffe4922f5f1e7989a66b/apps/readest-app/src/libs/document.ts)、[Rust MOBI 快速路径](https://github.com/readest/readest/blob/4af203755d807ae317c9ffe4922f5f1e7989a66b/apps/readest-app/src-tauri/src/mobi_parser.rs)、[Tauri bridge](https://github.com/readest/readest/blob/4af203755d807ae317c9ffe4922f5f1e7989a66b/apps/readest-app/src/utils/tauriMobiBridge.ts) |
| Readest foliate-js fork | [`ba57ec8`](https://github.com/readest/readest/tree/4af203755d807ae317c9ffe4922f5f1e7989a66b/packages/foliate-js) | [MOBI parser](https://github.com/readest/foliate-js/blob/ba57ec8a3f01be5533c1302c5edd3dab3d1b9147/mobi.js)、[README 限制](https://github.com/readest/foliate-js/blob/ba57ec8a3f01be5533c1302c5edd3dab3d1b9147/README.md)、[MIT 许可证](https://github.com/readest/foliate-js/blob/ba57ec8a3f01be5533c1302c5edd3dab3d1b9147/LICENSE) |
| `mobi` | [`0.8.0`](https://crates.io/crates/mobi/0.8.0) | [crate 源码](https://docs.rs/crate/mobi/0.8.0/source/src/lib.rs)、[记录解析](https://docs.rs/crate/mobi/0.8.0/source/src/record.rs)、[PalmDOC header](https://docs.rs/crate/mobi/0.8.0/source/src/headers/palmdoch.rs)、[仓库](https://github.com/vv9k/mobi-rs) |
| `boko` | [`0.5.0` / `8f412fb`](https://github.com/zacharydenton/boko/tree/8f412fb1a507399bce320d591feb517467cdb5f7) | [MOBI importer](https://github.com/zacharydenton/boko/blob/8f412fb1a507399bce320d591feb517467cdb5f7/src/import/mobi.rs)、[AZW3 importer](https://github.com/zacharydenton/boko/blob/8f412fb1a507399bce320d591feb517467cdb5f7/src/import/azw3.rs)、[HUFF/CDIC](https://github.com/zacharydenton/boko/blob/8f412fb1a507399bce320d591feb517467cdb5f7/src/mobi/huffcdic.rs)、[字节源](https://github.com/zacharydenton/boko/blob/8f412fb1a507399bce320d591feb517467cdb5f7/src/io/byte_source.rs)、[崩溃语料测试](https://github.com/zacharydenton/boko/blob/8f412fb1a507399bce320d591feb517467cdb5f7/tests/parser_crash_corpus.rs) |
| `libmobi` | [`0.12` / `85dcfe8`](https://github.com/bfabiszewski/libmobi/tree/85dcfe803fc2a21020ddcf15c3eb66b93d388add) | [公共 API](https://github.com/bfabiszewski/libmobi/blob/85dcfe803fc2a21020ddcf15c3eb66b93d388add/src/mobi.h)、[RawML/KF8 解析](https://github.com/bfabiszewski/libmobi/blob/85dcfe803fc2a21020ddcf15c3eb66b93d388add/src/parse_rawml.c)、[README 支持面](https://github.com/bfabiszewski/libmobi/blob/85dcfe803fc2a21020ddcf15c3eb66b93d388add/README.md) |

Amazon 没有公开足够完整、可直接实现 MOBI/KF8 reader 的正式规范。本评估以 [MobileRead MOBI 结构整理](https://wiki.mobileread.com/wiki/MOBI)、[KF8 结构整理](https://wiki.mobileread.com/wiki/KF8)、[KindleUnpack 反向工程入口](https://wiki.mobileread.com/wiki/KindleUnpack)、固定 Readest/foliate 源码和 `libmobi` 源码交叉验证。它们是可靠的反向工程证据，不应表述为 Amazon 的官方兼容保证。

## Readest 的实际覆盖

### 生产阅读路径

[Readest 的文档分派](https://github.com/readest/readest/blob/4af203755d807ae317c9ffe4922f5f1e7989a66b/apps/readest-app/src/libs/document.ts) 通过 `BOOKMOBI` 魔数选择 foliate 的 `MOBI(...).open(file)`，扩展名只用于把产品格式标为 `MOBI`、`AZW` 或 `AZW3`。固定 foliate fork 的 [`mobi.js`](https://github.com/readest/foliate-js/blob/ba57ec8a3f01be5533c1302c5edd3dab3d1b9147/mobi.js) 实际实现了：

- PalmDOC header、经典 MOBI7 和 MOBI/KF8 combo boundary；
- 无压缩、PalmDOC 和 HUFF/CDIC；
- MOBI6 的 guide、`filepos` 链接和目录投影；
- KF8 的 FDST、SKEL、FRAG、INDX、CNCX、NCX、RESC 与 PAGE；
- 图片、音频、视频和字体资源，zlib 压缩字体需要调用方提供解压函数；
- Windows-1252 与 UTF-8 文本编码。

这比 Readest 的 Rust [`mobi_parser.rs`](https://github.com/readest/readest/blob/4af203755d807ae317c9ffe4922f5f1e7989a66b/apps/readest-app/src-tauri/src/mobi_parser.rs) 更能代表阅读能力。后者调用 `mobi 0.8.x` 读取整本文件，仅负责部分 MD5、封面提取和缩放，源码注释把完整 metadata 与阅读留给 foliate-js。

### 成熟度与性能不能混为一谈

Readest 对固定 fork 的生产采用说明 foliate 已承受真实桌面和移动阅读工作负载；它不是只停留在 README 的概念实现。但同一固定版本的 [README](https://github.com/readest/foliate-js/blob/ba57ec8a3f01be5533c1302c5edd3dab3d1b9147/README.md) 明确说明 API 尚未稳定，KF8 仍可能因为 HUFF/CDIC 实现而慢。源码还显示：

- `MOBI6.init()` 会解压并拼接所有正文记录，再按 `<mbp:pagebreak>` 切分；
- KF8 会缓存从头或从尾解压的文本，随机章节访问在最坏情况下仍可能加载相当大比例的正文；
- 图片和其他资源按需读取，但这不能消除正文的峰值内存；
- parser 读取 encryption 字段，却不负责 DRM 解密或稳定拒绝。

因此不能从“Readest 正在使用”推出“Atha 在 Linux 或 ARM 上足够快”。也不能直接把 foliate 放进书籍 WebView：这会让不可信原始 HTML/CSS 进入前端，绕过 Atha 的原子导入、资源协议、安全校验与统一 Locator，并形成第二套书籍模型。

## Rust 候选比较

### 覆盖总表

| 候选 | PalmDOC / MOBI7 | HUFF/CDIC | KF8 / AZW3 | TOC / 资源 | DRM 行为 | I/O 与内存 | 判断 |
| --- | --- | --- | --- | --- | --- | --- | --- |
| [`mobi 0.8.0`](https://docs.rs/mobi/0.8.0/mobi/) | 有 | 有 | 只读部分 header，无 KF8 重构 | 基础 metadata、raw/image records；无完整 KF8 spine/TOC/CSS | 暴露字段，不解密；未知值会落成 `No` | `Read` 后保留整本 `Vec<u8>`，正文再拼接 | 只适合 metadata/封面辅助，不适合 reader |
| [`boko 0.5.0`](https://docs.rs/boko/0.5.0/boko/) | 有 | 有 | 有，含 pure/combo | 有，含 NCX、内部链接、CSS、图片、字体 | 非零 encryption 返回 `DrmProtected` | 文件随机读；首次内容访问后缓存大量解压正文和 parts | Rust 产品候选，需加固并 benchmark |
| [`libmobi 0.12`](https://github.com/bfabiszewski/libmobi/releases/tag/v0.12) | 有 | 有 | 有，另含 AZW4 等 | 格式覆盖最完整，含目录、链接、资源、字典 | 库含 DRM 相关 API；Atha 只能调用检测/拒绝路径 | C 结构与 malloc；调用方需审计总量和释放 | 成熟 oracle；产品使用取决于实测收益是否覆盖 FFI 成本 |
| [`iepub 1.3.7`](https://crates.io/crates/iepub/1.3.7) | 有 | 没有真实 HUFF 解压 | 没有 KF8 重构 | 基础 metadata/正文 | 检测有限 | Seek/Read，但会缓存全部文本；多处 `unwrap`/索引 | 拒绝 |
| [`zepub 1.3.1`](https://crates.io/crates/zepub/1.3.1) | 与 `iepub` 相近 | 不足 | 不足 | 不足 | 不足 | 同类实现，无明显安全收益 | 拒绝 |
| [`ebook-rs 0.13.4`](https://crates.io/crates/ebook-rs/0.13.4) | 仅无压缩/PalmDOC | 未真实支持 | 未实现 SKEL/FRAG 重构 | fallback 可生成占位正文和合成 TOC | 不足 | 无可信边界 | 拒绝 |
| [`ebook 0.1.3`](https://crates.io/crates/ebook/0.1.3) | 偏移启发式 | 无 | 无 | 启发式 | 探针不可靠 | 整文件内存 | 拒绝 |

### `mobi 0.8.0`

[`Mobi` 源码](https://docs.rs/crate/mobi/0.8.0/source/src/lib.rs) 把文件内容保存为 `Vec<u8>`；`from_read` / `from_path` 会把剩余输入全部读入内存，`content_as_string` 再拼接所有正文。它确实实现无压缩、PalmDOC 和 HUFF/CDIC，也能读取 metadata、封面和记录，因此 Readest 用它做封面快速路径是合理的。

它不实现 KF8 的 FDST/SKEL/FRAG、NCX/INDX/CNCX 和资源/CSS 重构，无法承担 `.azw3` 的高保真导入。更重要的是，[header 枚举](https://docs.rs/crate/mobi/0.8.0/source/src/headers/palmdoch.rs) 把未知 compression / encryption 值映射为“无压缩”或“无加密”，而 [record parser](https://docs.rs/crate/mobi/0.8.0/source/src/record.rs) 对不可信 offset 的边界校验不完整，存在切片 panic 风险。这些行为不适合 Atha 的不可信书籍边界。

维护方面，当前版本发布于 2022 年，仓库最后活动也停留在该时期。它不是“坏库”，但能力边界与维护状态都决定它不能作为下一切片的核心。

### `boko 0.5.0`

`boko` 是目前最接近 Atha 需求的 Rust 原生实现。固定源码的 [`MobiImporter`](https://github.com/zacharydenton/boko/blob/8f412fb1a507399bce320d591feb517467cdb5f7/src/import/mobi.rs) 与 [`Azw3Importer`](https://github.com/zacharydenton/boko/blob/8f412fb1a507399bce320d591feb517467cdb5f7/src/import/azw3.rs) 分别处理经典 MOBI 和 KF8，支持 pure KF8 与 combo boundary，并重构 FDST、SKEL、DIV/FRAG、INDX/CNCX/NCX、TOC、链接、CSS、图片和字体。`Format::from_path` 把 `.mobi` / `.azw` 归入 MOBI，把 `.azw3` 归入 AZW3；Atha 仍应在其前面嗅探真实 header，不信任扩展名。

其安全基础明显优于其他纯 Rust 候选：

- [`ByteSource`](https://github.com/zacharydenton/boko/blob/8f412fb1a507399bce320d591feb517467cdb5f7/src/io/byte_source.rs) 使用定位读取和显式边界检查；
- [`HUFF/CDIC`](https://github.com/zacharydenton/boko/blob/8f412fb1a507399bce320d591feb517467cdb5f7/src/mobi/huffcdic.rs) 有 32 层递归深度、单记录 16 MiB、整书预算和错误返回；
- PDB record 范围、字体解压与多种索引都有上限或 checked arithmetic；
- [崩溃语料测试](https://github.com/zacharydenton/boko/blob/8f412fb1a507399bce320d591feb517467cdb5f7/tests/parser_crash_corpus.rs) 覆盖确定性的 malformed/truncated 输入；
- 非零 encryption 会返回 typed `DrmProtected`，不会走解密路径。

但它还不能原样进入 Atha：

- HUFF 整书预算是 `file_len * 64 + 4 MiB`，`file_len` 包含图片等非正文资源。放在 Atha 512 MiB 源上限下仍可能允许不可接受的分配；
- 未知 compression 当前按原始正文继续处理，而不是稳定拒绝；
- KF8 首次章节加载后会缓存解压正文和构建出的 parts，经典 MOBI 也会一次解压正文，不能把随机文件 I/O 误称为流式常量内存；
- Windows-1252 等编码 fallback 仍有 lossy 路径；
- 当前公开测试的真实 MOBI/KF8 样本数量有限。仓库声称的大规模语料主要是 KFX，不能外推到本切片；
- `0.5.0` 很新、生态采用量小。活跃维护是利好，不等于兼容性已经稳定。

若采用，应固定 `0.5.0` 或经过审核的上游提交，并使用 `default-features = false`，避免 CLI/parallel 等无关 feature。第一步不是围绕它再包一层通用 trait，而是向上游提交或维护一个尽可能小的补丁：允许调用方传入**独立于源文件大小**的总生成正文上限，并对未知 compression/encoding 稳定失败。补丁合入、固定本地样本通过以及 Linux benchmark 达标三者缺一不可。

### `libmobi 0.12`

`libmobi` 的 [API](https://github.com/bfabiszewski/libmobi/blob/85dcfe803fc2a21020ddcf15c3eb66b93d388add/src/mobi.h) 与 [RawML/KF8 解析器](https://github.com/bfabiszewski/libmobi/blob/85dcfe803fc2a21020ddcf15c3eb66b93d388add/src/parse_rawml.c) 显示它不是只解析 metadata：它覆盖 PalmDOC、MOBI/PRC、KF8/AZW/AZW3、AZW4、HUFF/CDIC、目录、链接、图像、字体、音视频、字典与混合文件，维护历史和跨平台使用经验也显著长于当前 Rust 候选。格式完整度上它是本轮最强候选。

代价同样真实：

- Rust 需经 C FFI 调用 malloc 所有权模型，panic 隔离、释放、整数转换和错误映射都要单独审计；
- 未发现与 Atha 预算直接对应的调用方可配置总解压/节点/资源上限，不能把成熟 C parser 等同于现成信任边界；
- Android 需要为 ABI 构建和打包 native library，Linux/Windows 也要处理动态或静态链接、交叉编译和更新；
- [`mobi-sys 0.1.2`](https://crates.io/crates/mobi-sys/0.1.2) 只是 2018 年的 bindgen 系统库包装，使用很旧的构建依赖，不能直接视为维护中的安全 Rust facade；
- `libmobi` 是 `LGPL-3.0-or-later`，Atha 必须按实际链接/分发方式满足替换、源码和许可证义务。GNU 对 [GPL/AGPL 组合的说明](https://www.gnu.org/licenses/gpl-faq.en.html#AGPLGPL) 只能解释兼容性，不能替代 LGPL 分发设计和法律复核。

因此保留它作为同一批样本的 oracle 和性能对照。只有当它在格式正确率、导入 P95 或峰值 RSS 上对 `boko` 有明确且可重复的优势，并且该优势足以抵消 FFI、Android 与分发成本时，才进入产品方案。许可证不是本轮首要决策变量。

### 其余候选

[`iepub` 的 MOBI reader](https://github.com/inkroom/iepub/blob/baaf9db2026de1c089ed04e6c39bb7c283ca2293/lib/src/mobi/reader.rs) 虽然活跃，实际只对 compression `2` 做 PalmDOC 解压；HUFF 字段被读取但没有完整解压，KF8 也没有 SKEL/FRAG 重构，同时存在不可信索引上的 `unwrap` 和整本文本缓存。`zepub` 没有带来可辨识的安全或覆盖提升。

[`ebook-rs` 的 MOBI 实现](https://github.com/SV-stark/ebook-rs/blob/804c9bd4ccfb38ab81a98e535f2226d269330aed/src/mobi.rs) 只真实处理无压缩/PalmDOC，KF8 fallback 会抓取任意文本、生成占位正文和合成目录；[`ebook 0.1.3`](https://github.com/yingkitw/ebook/blob/92376c9b1727b24ed232f6c00118390535f9828d/src/formats/mobi.rs) 依赖猜测偏移和启发式目录。两者的公开格式声明都超过实际源码能力，不能进入下一轮 benchmark。

[`palmdoc-compression 0.3.1`](https://crates.io/crates/palmdoc-compression/0.3.1) 只解决 PalmDOC LZ77，不解决 PDB/MOBI/KF8 容器、HUFF/CDIC、目录或资源。如果采用 `boko` 或 `libmobi`，再单独加入它只会增加重复实现。

## 建议的最小交付

### 形状

新增一个具体的 `reader::kindle` module，复用既有 reader 数据模型、staging、manifest 校验和原子发布，不增加 `FormatImporter` registry、插件接口或第二套缓存抽象。

输入流程固定为：

1. picker 允许 `.mobi`、`.azw`、`.azw3`，但后端始终校验 PDB/MOBI magic、record 范围、MOBI version 和 KF8 boundary；扩展与结构不匹配时稳定拒绝；
2. 在进入第三方 parser 前检查源文件上限、PDB record 数/offset、encryption、compression 和已知 encoding；任何非零 encryption 立即返回稳定 DRM 错误；
3. 使用加固后的 `boko` 解析 spine、TOC、metadata、内部链接、CSS 和资源；每个章节取出后立即进入 Atha 现有 HTML/资源校验并写入 staging，不在 Atha 再维护一份长期正文 cache；
4. 所有章节和资源归一为 `ReaderManifest` / `BookRoot`，复用 1000 sections、2000 TOC items、16 MiB 单 section/resource 等现有边界；发布前复核源文件未变化；
5. 同一字节不因 `.mobi`、`.azw`、`.azw3` 改名而产生不同内容身份，使用单一固定格式域，例如 `atha-kindle-import-v1`；解析出的 MOBI/KF8 变体只是 metadata，不参与身份；
6. 日志只记录固定 operation/stage/code、源字节数、record/section/TOC/resource 数、compression 枚举和耗时，不记录路径、书名、作者、正文、URL 或内容哈希。

第三方 parser 的边界不能靠导入完成后的检查补救。至少要在可能分配前具备：

- 独立的总生成正文预算，初始值由本地样本测量后固定，不使用 `source_len * ratio`；
- 单记录、单章节、总 sections、TOC、图片/字体和 metadata 上限；
- record offsets 单调且在文件内，所有加减乘使用 checked arithmetic；
- HUFF/CDIC 递归、单 phrase、单 record 和全书预算；
- 未知 compression、未知加密、无法可靠解码的正文稳定拒绝；
- parser panic 转换为导入失败只能作为最后隔离，不替代内部边界检查。

### HTML、CSS 与资源

Kindle parser 只负责恢复语义内容，不拥有最终信任决定。输出继续经过 Atha 的白名单 HTML 校验：拒绝 script、事件处理器、表单、iframe、object/embed、远程导航、`file:`、`javascript:`、未登记 fragment 和目录越界资源。MOBI `filepos` 与 KF8 内部链接应转换为受控 section/fragment；无法唯一解析的跳转拒绝，而不是留成原始链接。

书源 CSS 只有通过 Atha 现有 CSS 安全规则后才写入 BookRoot：拒绝 `@import`、`url()`、`src()`、`image()`、`image-set()`、转义绕过和 Shadow DOM 穿透。不要把 boko 使用 `cssparser` 解释内部链接视为完整 sanitizer。第一版可以保留通过白名单的排版声明；需要外部获取或当前无法验证的规则应丢弃或稳定拒绝，不加载网络。

首版资源至少覆盖 JPEG/PNG 和本地 CSS。GIF、SVG、嵌入字体、音频、视频只有在补齐 MIME/magic、尺寸、解压和 CSS 引用边界后才允许；否则对**被正文引用**的资源返回明确的 `KINDLE_UNSUPPORTED_RESOURCE`，不能静默丢失。压缩字体样本必须进入测试矩阵：首版若不支持，就验证确定性拒绝而不是崩溃或超量分配。

### 明确拒绝范围

- 所有 DRM / encryption 非零文件；不接收 PID、密钥、账户凭据，不调用 `libmobi` 的解密 API；
- KFX、AZW4/PDF、Print Replica、固定版式与漫画专用变体；PDF 本来就在本轮之外；
- 第一版不承诺 `.prc`、`.kf8` 独立扩展名。Readest 的内部 fast path 或安装关联偶尔出现这些后缀，不等于完整产品入口；
- 外部 URL、远程资源、脚本、表单、嵌入应用、任意本地路径和路径越界；
- 未知 compression / encryption / encoding，以及无法在现有 section/resource 预算内安全恢复的书；
- 字典的索引与查词语义。MOBI dictionary 结构属于后续离线词典切片，本轮即使 parser 能读也不扩展领域模型；
- 不整库复制 foliate-js、KindleUnpack 或 libmobi，也不在 WebView 中直接打开原始 Kindle 文件；确需补齐格式细节时，只移植经过本地 parity 验证的最小算法，并保留来源、测试和安全上限。

## 测试矩阵

### 正向格式与呈现

| 类别 | 固定样本 | 关键断言 |
| --- | --- | --- |
| 经典 MOBI | 无压缩、UTF-8 | metadata、章节、基础目录、内部链接和文本稳定 |
| PalmDOC | PalmDOC 压缩、Windows-1252 | 解码正确，不出现 replacement 扩散，章节/搜索命中 |
| HUFF/CDIC | 经典 MOBI7 | 递归/预算生效，正文、目录与资源 parity |
| KF8 | 纯 KF8/AZW3 | FDST/SKEL/FRAG 重构、NCX、CSS、跨章节 fragment |
| Combo | MOBI7 + KF8 boundary | 明确优先 KF8，不重复章节/资源，不把旧 MOBI7 目录混入 |
| AZW | 无 DRM 经典内容，后缀 `.azw` | 与同字节 `.mobi` 结构和内容身份相同 |
| 资源 | cover、正文 JPEG/PNG、源 CSS | MIME/magic、尺寸、引用重写、无网络 |
| 复杂导航 | 嵌套 NCX、`filepos`、跨章节链接 | TOC 前序、fragment 唯一、返回/恢复稳定 |
| 大书 | 长正文、多 section、多资源 | 上限内成功，RSS 不随 20 次章节切换持续线性上升 |
| 可选资源 | GIF/SVG、zlib 压缩字体、音视频 | 支持项正常；未支持项返回固定错误且无部分发布 |

### 负向与信任边界

- encryption `1`、`2` 和未知值都必须得到同一稳定 DRM/unsupported 错误；
- 未知 compression、未知 encoding、损坏 PalmDOC/HUFF/CDIC 不得按 raw bytes 继续；
- PDB record 数超限，offset 非单调、越界、重叠或截断；
- EXTH 长度/数量溢出，FDST/SKEL/FRAG、INDX/CNCX/NCX 索引越界或自引用；
- HUFF 递归环、解压 bomb、异常长 phrase、总正文预算前一字节和后一字节；
- 缺失资源、伪造 MIME、图片尺寸/pixel 超限、字体解压超限；
- `javascript:`、`data:`、`file:`、外部 HTTP(S)、CSS `@import` / `url()` / 转义绕过；
- 同字节改名为 `.mobi` / `.azw` / `.azw3` 的身份相同，非 MOBI 字节伪装扩展名稳定拒绝；
- 任一 parser error、panic 隔离、磁盘写失败或源文件并发变化都不得留下可见半成品。

### Oracle 与样本治理

同一批无 DRM 样本离线跑三路：固定 Readest foliate fork、`boko` 候选和 `libmobi 0.12`。比较 metadata、spine 数、TOC 层级与目标、资源 MIME/数量、选择的章节文本摘要和内部链接图；差异必须人工归因，不能以“至少两路一致”自动裁决。KindleUnpack 可作为结构诊断工具，不进入运行时。

公开 fixture 只有在来源、许可和再分发权明确时才提交。指定本地书籍放在已忽略的 `fixtures/local` 或 `.tmp`，由显式环境变量 opt-in；不得输出路径、标题、正文或内容哈希，不进入 Git、公开报告和分发包。动态生成的极小 PDB 可以覆盖边界错误，但不能替代真实 HUFF、纯 KF8、combo 和复杂 CSS 样本。

进入实现前，本地样本集至少满足：

- 经典 MOBI/PalmDOC/HUFF、纯 KF8、combo 各至少一个；
- 至少一个 Windows-1252、一个嵌套 TOC、一个跨章节链接、一个带源 CSS、一个压缩字体；
- 至少一个接近实际大书体量的性能样本；
- 至少一个 DRM 文件只用于拒绝测试，且权利状态允许本机持有；
- 所有私有样本只登记匿名 fixture ID、字节规模、结构能力标签和预期结果。

## Linux GUI 与性能门

### 当前日常入口

日常开发和正式目标检查统一使用 Linux Tauri / WebKitGTK。新入口应沿用现有 `scripts/check-fb2-source.ps1 -VerifyLinuxGui` 的做法：构建真实 Tauri 壳，以隔离 XDG 数据种入测试书架，再用官方 [`tauri-driver`](https://v2.tauri.app/develop/tests/webdriver/) 与系统 WebKitWebDriver 驱动当前 GNOME 会话。Tauri 的 [WebDriver 手动配置](https://v2.tauri.app/develop/tests/webdriver/manual-setup/) 和 [WebView 版本说明](https://v2.tauri.app/reference/webview-versions/) 是环境事实来源。

Linux GUI gate 至少覆盖：

- picker 后缀允许列表由 Rust 测试验证；GUI 可像现有 FB2 gate 一样种入隔离 `LocalLibrary`，不伪装成已自动化原生文件对话框；
- 导入、书架卡片、打开、ready、TOC 嵌套跳转、跨章节链接、全书搜索；
- 连续 20 个 section 前后跳转、主题/字号变化、退出后重启并恢复同一 Locator；
- 原始脚本/外链/资源请求被拒绝，浏览器网络记录为零；
- 截图非空且没有错误页、正文遮挡或无法滚动；
- AppLog 包含固定阶段和计数，不含 fixture 路径、书名、正文、URL 或哈希。

### Benchmark 设计

`boko`、`libmobi` 和固定 foliate fork 必须使用**相同样本、相同 Linux 主机、相同 release-like 构建和相同冷/热定义**。先 warm-up，再至少记录 10 次；报告 median、nearest-rank P95、峰值 host RSS、峰值 WebKit renderer RSS 和失败数，同时固定 CPU、内存、内核、Rust、Tauri、WebKitGTK 与 parser revision。不要用单次最快值做决策。

导入阶段分别计时：

1. 源读取/指纹与 PDB preflight；
2. header/EXTH/combo boundary；
3. PalmDOC 或 HUFF/CDIC 解压；
4. FDST/SKEL/FRAG/TOC/链接重构；
5. HTML/CSS 安全归一；
6. 资源校验与写入；
7. manifest 校验、原子发布与总导入。

GUI 阶段分别记录：冷开至 first stable、缓存热开、TOC 首跳/远跳、搜索首结果、连续 20 section、字号重排和重启恢复。RSS 在导入前、导入峰值、打开峰值、20 section 后以及关闭书籍后稳定窗口采样，区分 Rust host 与 WebKit renderer；无法唯一归因时明确写“未生成数值”。

首轮不凭空规定毫秒和 MiB 门槛。用上述固定矩阵形成 accepted baseline，经审阅后把每个样本的 P95 与 RSS 阈值锁入 gate。无论基线数值如何，以下均立即失败：panic、OOM、hang、renderer crash、DRM/未知压缩被接受、绕过生成正文上限、任何网络请求、部分发布，以及 20 section 后 RSS 持续近线性增长而不平台化。

库选择以本地证据排序：正确覆盖和稳定拒绝先于速度，峰值 RSS 与 P95 其次，最后才是集成/维护成本。若加固后的 `boko` 与 `libmobi` 在正确性相当且性能差异不显著，选择 Rust 原生 `boko`；若 `libmobi` 对关键真实样本有可重复、显著优势，再单独评审 FFI/Android/分发成本。Readest 生产采用度是 foliate 成熟度证据，但固定源码的慢路径声明和本地测量仍拥有最终决策权。

### Android 与 ARM

Android 模拟器不属于日常 gate，保持关闭，也不为 Kindle 切片恢复 AVD 性能流程。Linux GUI 通过后，只有在发布前或移动端专项验收时，才在至少一台指定 ARM64 Android 真机上使用 release-like APK 和真实系统 WebView，重跑导入、冷开、TOC、搜索、20 section、重排、恢复、RSS/PSS、ANR 与 renderer crash 检查，并记录设备、Android、WebView、ABI 和 thermal 状态。

Linux 数字不能表述为 ARM 性能证据；x86_64 模拟器也不能替代真机。没有 ARM 样本证据时可以关闭 Linux 功能切片，但不能声称移动端性能已经验收。

## 实施决策回填

本轮以两个普通纯 KF8、一个词典压力样本和动态 PalmDOC fixture 收敛首版范围，结论如下：

- 普通样本与 `boko` / `libmobi` 的正文、目录和图片结构完成对照；Atha 使用 `boko` 官方 TOC position 修复，并按 ReaderManifest 契约去重相同最终目标；
- Atha 的调用前预检替代上游补丁：未知 compression / encoding、非零 encryption、词典索引、非法 record 和超过固定正文 / HUFF 预算的输入均在依赖展开前拒绝；
- 首版资源支持 JPEG、PNG、GIF 与安全 inline style；KF8 flow stylesheet、SVG、字体、音视频、KFX、AZW4 / PDF、`.prc` / `.kf8` 后缀、DRM 和字典查询语义不在本切片；
- 正式脚本固定 10 次 warm-cache release benchmark、词典早拒绝与 Linux Tauri / WebKitGTK 功能、截图和隐私门；Android 只保留发布前 ARM64 真机门。

未补齐经典 HUFF MOBI7、combo、Windows-1252、压缩字体和复杂跨章节链接真实语料，因此当前交付是 Readest 对应扩展名的安全首版，不外推为全部历史 Kindle 变体兼容。若这些语料证明 `boko` 公共 API 不足，再按用户批准借鉴固定成熟实现做最小 fork，而不是预先复制整库。

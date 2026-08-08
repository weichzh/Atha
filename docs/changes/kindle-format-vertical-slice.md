---
description: MOBI、AZW 与 AZW3 / KF8 的有界导入、性能基准及 Linux GUI 纵切。
---

# Kindle 格式纵切

## Status

`accepted`

## Problem

Atha 已支持 EPUB、CBZ、Markdown、TXT、FB2 与 FBZ，但 Readest 0.11.18 的非 PDF 格式还包含 MOBI、AZW 与 AZW3 / KF8。现成候选不能同时满足当前样本的覆盖、性能和信任边界：`mobi 0.8` 无法完整解码两个本地 KF8 样本，Readest 固定的 `foliate-js` 在 Node 代理环境中明显更慢且更耗内存，`libmobi 0.12` 需要 C FFI 并在词典样本上超过 120 秒，`boko 0.5.0` 覆盖最好但会把未知压缩当原文，并允许词典在拒绝前膨胀到约 1.7 GiB。

用户要求以性能和成熟度为首要标准；现成 Kindle 库若均不满足，则借鉴成熟实现为 Atha 建立最小、可审计的解析路径，而不是继续选择已知失败的依赖。

## Research And Decision

- 采用仓内具体的 `reader::kindle` adapter，只移植 `boko 0.5.0` 中经本地样本验证的 PDB、PalmDOC、HUFF / CDIC、MOBI、KF8、索引与资源读取算法，并以 `libmobi 0.12` 和 Readest 固定 `foliate-js` 作为行为 oracle；保留来源和修改说明，不引入整个 5.9 万行 crate；
- 不建立通用电子书 IR、codec registry、格式 factory 或第二套渲染器。Kindle 内容直接归一为现有 schema 1 ReaderManifest / BookRoot，继续使用同一 Locator、搜索、消息、CSS 安全投影与原子发布；
- 信任边界先于兼容回退：加密、未知压缩 / 编码、缺失 HUFF 记录、非法偏移、超限文本 / 资源 / section / TOC 和字典结构均稳定拒绝；不得把未知记录当正文、用 lossy decoding 掩盖错误或在发布前无界展开；
- `.mobi`、`.azw` 与 `.azw3` 只作为输入后缀，内容身份由受控 Kindle 域和源字节决定，不因改后缀产生副本。KFX、AZW4、PDF、DRM 和词典查询语义不在本切片。

## Scope

- 导入无 DRM 的 PalmDOC / MOBI6 和 AZW3 / KF8 普通书籍，覆盖未压缩、PalmDOC 与 HUFF / CDIC 正文、KF8 skeleton / fragment、INDX / CNCX 目录及书内图片；
- 在解压前识别加密和词典索引；固定正文、记录、section、TOC、单资源和资源总量预算，按顺序 staging 并在全部校验通过后原子发布；
- 接入 LocalLibrary、Linux / Android picker、稳定错误码和固定字段日志；不记录标题、正文、路径、URI、哈希或书内 URL；
- 使用用户指定的本地普通 KF8 与词典样本做 release benchmark。普通书籍与 oracle 对齐正文、目录、图片和内部链接；词典样本必须在大量解压前快速、低内存拒绝；
- 使用 Linux Tauri / WebKitGTK 正式 GUI 验证书架、目录、搜索、书内跳转、翻页、恢复、截图和无外联。Android 模拟器保持关闭，ARM64 真机只作为发布前专项门。

## Out Of Scope

- Kindle 词典的索引查询、词形变化和弹窗体验；它们进入后续“本地词典”切片，而不是伪装成百万 section 的普通书；
- KFX、AZW4 / Print Replica、PDF、DRM、网络资源、Kindle 导出 / 写回和自定义 CSS cascade；
- 复制 `boko` 的 KFX、导出、写入、CLI、通用书籍模型或未被当前格式读取需要的代码；
- Android 模拟器日常验收，以及尚无 ARM64 真机数据时的跨设备性能承诺。

## Architecture Impact

present

- Design purpose: 在具体格式边界内复用成熟解析算法，同时把安全预算、错误语义和发布事务收回 Atha 现有 ReaderManifest / BookRoot 契约。
- Drivers / quality scenarios: `A-KINDLE-01` 要求两个本地普通 KF8 样本可完整阅读；`A-KINDLE-SEC-01` 要求恶意或损坏 PDB / MOBI 在原子发布前稳定拒绝；`A-KINDLE-PERF-01` 要求词典在正文展开前识别，普通样本的导入时间和峰值 RSS 不劣于已测 `boko` 基线的 2 倍，且无 OOM、panic 或超时。
- Modules / interfaces: 第一个 public seam 是 `LocalLibrary::import`，新实现只增加一个具体 `reader::kindle` adapter；ReaderManifest、BookRoot、LibraryBook、Locator、Search、MessageStore 和 WebView reader API 不变。
- Candidate tradeoffs: `mobi 0.8` 因本地样本功能失败淘汰；`foliate-js` 因第二运行时、内存和不稳定 API 淘汰；`libmobi` 因 unsafe C FFI、部署面和词典超时只保留 oracle；完整 `boko` 因无界 / 宽松回退及大量无关功能淘汰。最小移植比维护完整 fork 少，但必须保留上游 provenance、差异测试和安全边界测试。
- Review trigger: 若最小移植无法在普通样本上同时达到内容对齐与 2 倍性能门，则停止扩展格式覆盖并复评经过补丁的 `libmobi` FFI；不得靠提高内存 / section 上限通过。词典样本不参与普通阅读成功率。

## Acceptance Criteria

- [ ] `.mobi`、`.azw` 与 `.azw3` 经同一 importer 生成稳定 manifest、书目、封面、正文、目录、内部链接和图片资源；相同源字节跨后缀共享内容身份；
- [ ] 两个指定本地 KF8 样本与至少一个最小 PalmDOC / MOBI6 fixture 导入成功；正文片段、目录目标、资源数量和内部链接与至少一个成熟 oracle 的可比输出对齐；
- [ ] 指定词典样本在正文展开前以稳定 `kindle-dictionary-unsupported` 拒绝；release 运行不超过 2 秒且峰值 RSS 不超过 128 MiB；
- [ ] 加密、未知压缩 / 编码、缺失 HUFF / CDIC、非法 section / record / offset、损坏 INDX / CNCX、超限正文 / 资源 / section / TOC 和源文件变化返回稳定错误且不留下可打开书根；
- [ ] 10 次 warm-cache release benchmark 记录 median、nearest-rank P95 和 peak RSS；普通样本导入无功能失败，且时间和 RSS 均不超过既有 `boko` 基线的 2 倍；
- [ ] picker、LocalLibrary、ReaderManifest、BookRoot、Locator、搜索、消息和 CSS 安全层不分叉，既有 EPUB、CBZ、Markdown、TXT、FB2 与 FBZ 不回归；
- [ ] Linux Tauri / WebKitGTK 正式 gate 完成真实本地普通样本的书架、打开、目录、搜索、内部跳转、翻页、重启恢复、非空截图和日志隐私检查；
- [ ] Rust fmt / Clippy / tests、Svelte / Tauri check / build、AutoCorrect、required docs gate 与独立 Spec / Standards review 通过。

## Files And Steps

1. 以 `LocalLibrary::import` 建立 PalmDOC / MOBI6、两个私有 KF8 样本和词典早拒绝的 public-seam red tests；私有样本不复制、不派生、不提交。
2. 从 `boko 0.5.0` 最小移植只读解析算法，加入严格 header、compression、encoding、offset、record、正文、资源和计数预算，直接生成现有 manifest / book root staging。
3. 接入严格后缀分派、picker、日志和内容身份；补齐 malformed、DRM、未知值、预算、源变化和原子发布测试。
4. 新增正式 Kindle source / Linux GUI gate，对本地样本执行 oracle parity、10 次 release benchmark、词典早拒绝、真实 Tauri 交互和隐私扫描。
5. 更新第三方来源与事实所有者，完成双轴独立 review、提交和 task closure。

## Checks

- `cargo test --locked -p atha-backend --test kindle_import` 与 workspace fmt / Clippy / tests；
- `scripts/check-kindle-source.ps1` 的本地样本 parity、10 次 release benchmark、词典早拒绝与 `-VerifyLinuxGui`；
- `pnpm --dir reader/app check`、`pnpm --dir reader/app build` 与 Tauri tests；
- provenance / notices、AutoCorrect、`git diff --check`、required docs gate 与独立 Spec / Standards review。

## Rollback

删除具体 Kindle adapter、picker 分派和 gate，恢复 LocalLibrary 的既有允许列表。ReaderManifest、BookRoot、LibraryBook、Locator、消息数据库和其他格式缓存 schema 不迁移；已导入 Kindle 书根只会成为未引用 cache，源书不改写。

## Approval

用户已批准按 Atha 路线图持续完成 Readest 支持的非 PDF 格式，明确要求性能和成熟度优先；在现成 Kindle 库均不满足时，允许借鉴成熟实现为 Atha 自建最小解析路径。本 change 是该批准下的下一最小纵切。

## Result

待实施。

## Review

待实施后由独立 agent 完成 Spec / Standards 双轴 review。

## Evidence And Residual Risks

待实施。当前基准与候选淘汰证据见 `docs/research/kindle-format-library-assessment.md`。

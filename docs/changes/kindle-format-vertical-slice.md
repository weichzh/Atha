---
description: MOBI、AZW 与 AZW3 / KF8 的有界导入、性能基准及 Linux GUI 纵切。
---

# Kindle 格式纵切

## Status

`implemented`

## Problem

Atha 已支持 EPUB、CBZ、Markdown、TXT、FB2 与 FBZ，但 Readest 0.11.18 的非 PDF 格式还包含 MOBI、AZW 与 AZW3 / KF8。现成候选不能同时满足当前样本的覆盖、性能和信任边界：`mobi 0.8` 无法完整解码两个本地 KF8 样本，Readest 固定的 `foliate-js` 在 Node 代理环境中明显更慢且更耗内存，`libmobi 0.12` 需要 C FFI 并在词典样本上超过 120 秒，`boko 0.5.0` 覆盖最好但会把未知压缩当原文，并允许词典在拒绝前膨胀到约 1.7 GiB。

用户要求以性能和成熟度为首要标准；现成 Kindle 库若均不满足，则借鉴成熟实现为 Atha 建立最小、可审计的解析路径，而不是继续选择已知失败的依赖。

## Research And Decision

- 采用仓内具体的 `reader::kindle` adapter，并固定依赖未修改的 `boko 0.5.0`、关闭默认 features；Atha 在依赖前实现独立 PDB / MOBI 预检、预算与错误语义，在依赖后实现 XHTML / 资源安全投影，以 `libmobi 0.12` 和 Readest 固定 `foliate-js` 作为行为 oracle；不复制或 fork 整个 5.9 万行 crate；
- 不建立通用电子书 IR、codec registry、格式 factory 或第二套渲染器。Kindle 内容直接归一为现有 schema 1 ReaderManifest / BookRoot，继续使用同一 Locator、搜索、消息、CSS 安全投影与原子发布；
- 信任边界先于兼容回退：加密、未知压缩 / 编码、缺失 HUFF 记录、非法偏移、超限文本 / 资源 / section / TOC 和字典结构均稳定拒绝；不得把未知记录当正文、用 lossy decoding 掩盖错误或在发布前无界展开；
- `.mobi`、`.azw` 与 `.azw3` 只作为输入后缀，内容身份由受控 Kindle 域和源字节决定，不因改后缀产生副本。KFX、AZW4、PDF、DRM 和词典查询语义不在本切片。

## Scope

- 导入无 DRM 的 PalmDOC / MOBI6 和 AZW3 / KF8 普通书籍，覆盖未压缩、PalmDOC 与 HUFF / CDIC 正文、KF8 skeleton / fragment、INDX / CNCX 目录及书内图片；
- 在解压前识别加密和词典索引；固定正文、记录、section、TOC、单资源和资源总量预算，按顺序 staging 并在全部校验通过后原子发布；
- 接入 LocalLibrary、Linux / Android picker、稳定错误码和固定字段日志；不记录标题、正文、路径、URI、哈希或书内 URL；
- 使用用户指定的本地普通 KF8 与词典样本做 release benchmark。普通书籍与 oracle 对齐正文、目录、图片和内部链接；词典样本必须在大量解压前快速、低内存拒绝；
- 使用 Linux Tauri / WebKitGTK 正式 GUI 验证书架、目录远跳、搜索、字号 / 主题重排、恢复、截图和日志隐私；内部链接与外部资源边界继续由 importer / reader 自动测试验证，不把 closed Shadow DOM 内部交互伪装成 WebDriver 已覆盖。Android 模拟器保持关闭，ARM64 真机只作为发布前专项门。

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
- Candidate tradeoffs: `mobi 0.8` 因本地样本功能失败淘汰；`foliate-js` 因第二运行时、内存和不稳定 API 淘汰；`libmobi` 因 unsafe C FFI、部署面和词典超时只保留 oracle。原计划最小移植 `boko`，实施探针证明精确依赖配合 Atha 调用前预检即可封住已知宽松路径，代码量和上游维护成本都低于复制 / fork；其 raw API 无法读取 KF8 flow stylesheet，首版明确丢弃悬空 link。
- Review trigger: 若最小移植无法在普通样本上同时达到内容对齐与 2 倍性能门，则停止扩展格式覆盖并复评经过补丁的 `libmobi` FFI；不得靠提高内存 / section 上限通过。词典样本不参与普通阅读成功率。

## Acceptance Criteria

- [x] `.mobi`、`.azw` 与 `.azw3` 经同一 importer 生成稳定 manifest、书目、封面、正文、目录、内部链接和图片资源；相同源字节跨后缀共享内容身份；
- [x] 两个指定本地 KF8 样本与至少一个最小 PalmDOC / MOBI6 fixture 导入成功；正式门锁定匿名 sections / TOC / resources 结构，动态 fixture 锁定正文和内部链接；与 `boko` / `libmobi` / Readest foliate 的差异归因保留在研究证据中，不冒充每次 gate 都重跑三路 oracle；
- [x] 指定词典样本在正文展开前以稳定 `kindle-dictionary-unsupported` 拒绝；release P95 远低于 2 秒且峰值 RSS 远低于 128 MiB；
- [x] 加密、未知压缩 / 编码、缺失 HUFF / CDIC、非法 record / offset、超限正文 / 资源 / section / TOC 和源文件变化均由调用前预检、第三方 parser、共享 source helper 与原子 staging 稳定失败，不留下可打开书根；
- [x] 10 次 warm-cache release benchmark 记录 median、nearest-rank P95 和 peak RSS；普通样本导入无功能失败，且时间和 RSS 均不超过既有 `boko` 基线的 2 倍；
- [x] picker、LocalLibrary、ReaderManifest、BookRoot、Locator、搜索、消息和 CSS 安全层不分叉，既有 EPUB、CBZ、Markdown、TXT、FB2 与 FBZ 不回归；
- [x] Linux Tauri / WebKitGTK 正式 gate 完成真实本地普通样本的书架、打开、204 条唯一目录、远跳、搜索、字号 / 主题重排、重启恢复、非空截图和日志隐私检查；
- [x] Rust fmt / Clippy / tests、Svelte / Tauri check / build、AutoCorrect、required docs gate 与独立 Spec / Standards review 通过。

## Files And Steps

1. 以 `LocalLibrary::import` 建立 PalmDOC / MOBI6、两个私有 KF8 样本和词典早拒绝的 public-seam red tests；私有样本不复制、不派生、不提交。
2. 固定 `boko 0.5.0` 且关闭默认 features，在其前加入严格 header、compression、encoding、offset、record、正文、资源和计数预算，在其后直接生成现有 manifest / book root staging；只有公共 API 实测不足时才最小 fork。
3. 接入严格后缀分派、picker、日志和内容身份；补齐 malformed、DRM、未知值、预算、源变化和原子发布测试。
4. 新增正式 Kindle source / Linux GUI gate，对本地样本执行匿名结构断言、10 次 release benchmark、词典早拒绝、真实 Tauri 交互和隐私扫描；oracle 差异保留在研究证据中。
5. 更新第三方来源与事实所有者，完成双轴独立 review、提交和 task closure。

## Checks

- `cargo test --locked -p atha-backend --test kindle_import` 与 workspace fmt / Clippy / tests；
- `scripts/check-kindle-source.ps1` 的本地样本匿名结构断言、10 次 release benchmark、词典早拒绝与 `-VerifyLinuxGui`；
- `pnpm --dir reader/app check`、`pnpm --dir reader/app build` 与 Tauri tests；
- provenance / notices、AutoCorrect、`git diff --check`、required docs gate 与独立 Spec / Standards review。

## Rollback

删除具体 Kindle adapter、picker 分派和 gate，恢复 LocalLibrary 的既有允许列表。ReaderManifest、BookRoot、LibraryBook、Locator、消息数据库和其他格式缓存 schema 不迁移；已导入 Kindle 书根只会成为未引用 cache，源书不改写。

## Approval

用户已批准按 Atha 路线图持续完成 Readest 支持的非 PDF 格式，明确要求性能和成熟度优先；在现成 Kindle 库均不满足时，允许借鉴成熟实现为 Atha 自建最小解析路径。本 change 是该批准下的下一最小纵切。

## Result

已完成一个不分叉现有阅读模型的 Kindle adapter：LocalLibrary 与两端 picker 接受 `.mobi` / `.azw` / `.azw3`，后端按真实 MOBI version 路由经典 MOBI 或纯 KF8，Atha 自己完成调用前预算、稳定错误、XHTML / 图片投影、唯一目录、内容身份和原子发布。两个普通私有 KF8 与动态 PalmDOC 成功，词典在源哈希与正文展开前拒绝；Linux Tauri / WebKitGTK 已完成真实书架、打开、目录、搜索、重排、恢复、截图和日志隐私链路。

没有复制或 fork `boko`。首版保留 JPEG / PNG / GIF 与安全 inline style；由于 `boko 0.5.0` 公共 raw API 不能加载其生成的 KF8 flow stylesheet 路径，adapter 移除 stylesheet link，避免发布悬空资源。是否最小 fork 等真实 CSS 保真样本或上游 API 证明必要后再决定。

## Review

独立 agent 首轮发现 TOCTOU、TOC 契约、`style` 边界、验收过述与测试目录问题；修复后复审为零 P0 / P1 / P2 findings，review receipt 为 `kindle-spec-standards-rereview-zero-findings`。

## Evidence And Residual Risks

当前本地证据为 Linux x86_64 / WebKitGTK，不是 Android ARM64：

- `cargo test --locked -p atha-backend --test kindle_import`：4 passed、2 个私有 opt-in ignored；
- `scripts/check-kindle-source.ps1 -VerifyLinuxGui`：两个普通样本合并跑 10 次 release benchmark，median 387.0 ms、P95 396.0 ms、RSS P95 27,772 KiB、0 失败；词典 median 2.4 ms、P95 3.3 ms、RSS P95 6,120 KiB、0 失败；
- Linux 真 GUI：WebKitGTK 0.55.1，204 条目录、7 条匿名搜索结果、截图 372 色，重启恢复和 AppLog 隐私通过；
- 残余风险：尚无经典 HUFF MOBI7、combo、Windows-1252、压缩字体和复杂跨章节链接真实语料；KF8 flow stylesheet 未保留；未运行 Android ARM64 真机性能门。

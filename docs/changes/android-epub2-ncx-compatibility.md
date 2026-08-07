# Android EPUB2 / NCX 兼容

## Status

implemented

## Problem

Atha 当前 EPUB importer 只接受 OPF `version="3.0"`，并强制 manifest 中存在 EPUB3 XHTML navigation document；标准 EPUB2 使用 OPF 2.0、`spine toc` 和 `application/x-dtbncx+xml` NCX，因此会在进入现有 ReaderManifest 与 Android 主循环前被拒绝。Readest 的多格式路线已经确认 EPUB2 / NCX 是下一个最低风险兼容切片。

## Scope

- 在现有 `backend::reader::epub` deep module 内同时接受受控的 OPF 2.0 与既有 EPUB3，不建立第二套 importer；
- 解析 OPF2 `spine toc` 指向的 NCX manifest item，把 `navMap` 按文档顺序投影为现有扁平 `ReaderManifest.toc`，继续复用 sections、Locator、资源白名单和 reader kernel；
- 支持 EPUB2 标准 title / creator 与 `meta name="cover"` 封面引用；保留现有字段数、文本、条目、章节、TOC、压缩和解压大小上限；
- 只接受 NCX 固定结构、受控文档声明和指向已声明 section 的相对引用；继续拒绝实体、外部资源、路径越界、重复标识、空标签、无界深度与超量条目；
- 在 reader 内容与搜索边界只放行标准 XHTML 1.1 与兼容扩展 XHTML 1.0 Strict 固定文档声明，仍拒绝未知声明、实体与主动内容，避免出现“可导入但不可打开”的半兼容；
- 在既有 document title / Android 可访问名称中加入不含正文的 section / page 序号，供读屏上下文与正式位置恢复 gate 复用；
- 用测试代码动态生成最小 EPUB2 / NCX 与恶意变体，不把受版权保护的本机书籍提交到 Git；在 Android 正式入口复用同一合成 EPUB2 样本验证导入、目录跳转、重启恢复和日志边界；
- 研究并记录 EPUB 规范、Readest / foliate-js 与成熟库方案；现有 quick-xml / zip 能保持边界和复杂度时不增加依赖。

## Non-Goals

- 不在本切片增加 CBZ、Markdown、TXT、FB2、MOBI / KF8、PDF、DRM 或固定版式；
- 不迁移到 Readest UI 或 foliate-js，不重写 ReaderManifest、BookRoot、Locator、Svelte 壳或消息模型；
- 不为畸形 EPUB2 做无限宽容模式，不加载外部 DTD / 实体，不把 guide、page-list、landmarks 或 SMIL 预建为通用导航模型；
- 不把 x86_64 模拟器结果称为 ARM 真机性能证据。

## Architecture Impact

present

- Design purpose: 在唯一 EPUB 边界把 OPF2 / NCX 归一为现有 ReaderManifest，所有下游继续只消费同一 sections / toc / resources 契约。
- Drivers / quality scenarios: `A-EPUB2-01`（高业务重要性 / 中技术风险，负责人：EPUB Importer）；刺激源是 Android 用户，刺激是选择 DRM-free EPUB2，环境是离线系统 picker 与系统 WebView，响应是受控解析 OPF2 / NCX、显示目录、跳转、重启恢复，度量是固定样本无 unsupported / invalid XML，TOC 引用与 section 一致，现有 EPUB3 和 Windows 门禁不回归。`A-EPUB2-SEC-01`（P0 内容安全，负责人：EPUB Importer）；恶意 NCX 提交外部 URI、实体、路径越界、重复或超量 navPoint 时，在复制到受控书根前返回稳定错误且无网络 / 越界文件访问。
- Modules / Interfaces / Seams / Adapters: 扩展 `backend::reader::epub::package` 的 package / navigation 归一化，并在 reader 既有内容 / 搜索安全边界共享固定 XHTML 文档声明判断；`import_epub`、ReaderManifest、BookRoot、Tauri SAF bridge 与 reader runtime 接口保持不变。
- Candidate and tradeoffs: 优先复用已安装的 quick-xml / zip 与现有安全 helper；只有研究或固定样本证明它们无法正确覆盖规范结构时才采用成熟库。拒绝并行 importer、格式工厂和 JS/Rust 双份 EPUB 事实。
- Evidence / review trigger: EPUB 2.0.1 / EPUB 3 一手规范、Readest / foliate-js 源码、动态 fixture 单元测试、现有 EPUB3 / Windows gate 与 Android EPUB2 真实链路；只有 parser 复杂度或真实性能越过现有边界时才重评库。

## Acceptance Criteria

- [x] 动态生成的标准 OPF2 + NCX EPUB 可导入，ReaderManifest 的 sections、扁平 TOC、title、authors 与 cover 正确，嵌套 navPoint 顺序稳定；
- [x] 外部 / 越界 NCX 引用、实体或未知文档声明、重复 / 空 / 超量条目与缺失 spine toc 都被稳定拒绝，现有 EPUB3 安全测试继续通过；
- [x] Android 正式入口完成系统 picker 导入、目录跳转、first-stable / ready、强停重启与位置恢复，日志和证据不含书名、路径、URI 或正文；
- [x] Rust fmt / Clippy / tests、Svelte / Windows Tauri 回归、required docs gate 与独立 Spec / Standards review 通过；
- [x] 研究结论进入事实所有者，未引入无证据依赖或第二套阅读模型。

## Files And Steps

1. 研究 EPUB2 / NCX 规范、Readest / foliate-js 与成熟 Rust 库，固定最小兼容边界；
2. 先加入动态 EPUB2 / NCX 成功与拒绝 fixture，确认现有失败点；
3. 在 package parser 内归一 OPF2 navigation / metadata，并最小放行标准 XHTML 固定文档声明，保持现有 archive 与 ReaderManifest 契约；
4. 运行 Rust / Windows 回归，再在 Android gate AVD 用本机 EPUB2 样本验证真实链路；
5. 更新事实文档，完成双轴 review、required gate 与 workflow 收尾。

## Checks

- `cargo test -p atha-backend --test epub_import` 与 workspace Clippy `-D warnings`；
- `scripts/check-tauri-reader.ps1`；
- `scripts/check-android-reader.ps1 -EpubPath <generated-epub2.epub> -CleanAppData -VerifyEpub2NcxFixture`；
- EPUBCheck 5.3.0 对动态生成制品执行 EPUB 2.0.1 校验；
- AutoCorrect、required docs gate、Spec / Standards review。

## Evidence And Residual Risks

- 最高证据为最终候选 APK 在专用 Android 16 KiB x86_64 模拟器上的真实系统 picker、NCX 目录跳转与重启位置恢复；Windows Tauri / WebView2 全链路、官方 EPUBCheck 与动态恶意 fixture 提供交叉证据；
- Android ARM 真机性能、UTF-16 与 EPUB2 非 XHTML 内容仍未覆盖；WebView 124 在 16 KiB 模拟器上的上游 MemoryInfra 缺陷仍是环境残余风险，不归因于 Atha；
- fixture 与 EPUBCheck 工具仅存在于被 Git 忽略的本地目录，书名、作者、目录、正文 token、路径和 URI 不进入产品日志或版本控制。

## Rollback

删除 OPF2 / NCX 分支与对应 fixture 即恢复 EPUB3-only 行为；ReaderManifest、Library、消息数据库和导入缓存 schema 不迁移，回滚不改写用户事实。

## Approval

用户已明确批准按照路线图持续完成 Android 与 Readest 非 PDF 格式，并要求先研究成熟方案、少造轮子、保留日志和目标端验证。本 change 是已批准路线图中的下一最小切片。

## Result

已完成实现、双轴复审、最终目标端复验与 required docs gate。

- 使用既有 `zip 8.6`、`quick-xml 0.41` 与同一 importer；OPF2 `spine@toc` 只解析对应 NCX，嵌套 `navPoint` 前序投影到现有 flat TOC，ReaderManifest / Locator / BookRoot 接口未变；
- NCX、container、OPF 与 EPUB3 nav 共用 256 层 XML 深度上限；固定 canonical DOCTYPE、root 顺序、ID / href / playOrder、label、TOC 和 legacy cover 失败语义均有动态拒绝测试；
- fixture 为原创 3,617-byte EPUB，SHA-256 `6991bfb8edd895a44cb5b0e9066805ee6cea030f47856f3607e8ee2cf4be5887`；EPUBCheck 5.3.0 以 EPUB 2.0.1 规则报告 0 fatal / error / warning / info；
- Windows 正式 gate 通过全部 Rust / Svelte / Tauri / WebView2 回归和十样本性能门槛；最终 run `1786136879142-4104` 的 P95 为 cold start 778.879ms、first stable 183.900ms、hot open 24.500ms、page turn 8.500ms、font reflow 50.000ms；
- 最终 Android debug APK 在 `Atha_API_35_16K`（API 35、x86_64、16 KiB、WebView 124.0.6367.219）干净数据上通过系统 picker、导入、目录第二项跳转、first-stable / ready、强停重开和同一 section / page 恢复；重启前后日志均扫描 URI、路径、标题、作者、目录与唯一正文 token，结构化证据未含书籍内容；
- Windows 完整门发现 staging 原子发布约每 16 次出现一次瞬时 `PermissionDenied`（OS 5）；只对该错误增加最多 4 次、每次 10ms 的有界重试，最终失败记录固定 `publish-rename` stage 与非敏感 `io_kind`，随后压力探针 100 / 100 通过；
- UTF-16、DTBook、OEBPS 文档、完整 fallback 与 ARM 真机性能仍明确在本切片外。

## Review

- Blocking: 初次与二次 review 指出的双启动日志隐私、规范 oracle、NCX root / label / external media、cover 失败语义、UTF-8 导入边界、共享 XML 深度、fixture hash 绑定、边界矩阵和通用 Android gate 均已修复；最终 Spec review 为 PASS，Standards review 无阻塞问题。
- Non-blocking: EPUBCheck 保持开发期 oracle，不作为运行时、APK 或新增依赖；官方工具只在本机忽略目录执行。
- Out-of-scope: 其他格式、ARM 真机性能与发布交付留在后续切片。

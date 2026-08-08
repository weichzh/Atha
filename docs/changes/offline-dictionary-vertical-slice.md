---
description: MDict 与经典 Kindle 离线词典的导入、精确查词、安全释义和真机性能切片。
---

# 离线词典垂直切片

## Status

accepted

## Problem

Atha 已能导入并阅读书籍，但选中文本只能复制、标注或记笔记，不能使用用户自己的离线词典。现有 Kindle 书籍导入器会正确拒绝词典文件；直接把词典当普通书展开会产生约 130 万节和约 1.7 GiB 内存，`libmobi` 全量 RawML 路线也已触发 120 秒停止条件。MDict 则已有通过私有样本 P0 的按需 Rust reader，不需要再造解析器。

## Scope

- 在应用本地数据目录导入、列出和移除 MDict v2 的 `.mdx` 与可选 `.mdd`，固定使用已通过 P0 的 `mdict-rs 0.1.4`；
- 为匿名 `KINDLE-D` 实现经典 MOBI6、HUFF/CDIC、Windows-1252 与 orth 索引的最小随机访问精确查词，不经过普通书籍章节模型；
- 选中正文后提供“查词”，在不离开阅读上下文的工具面板显示词典、命中词头、纯文本释义和结构化错误；
- 释义在后端去除标记、脚本、样式、表单、外链和资源引用，前端只以文本节点渲染；MDD 资源只在后端私有样本测试与 benchmark 验证范围读取，首版 UI 不显示图片、CSS 或音频；
- 日常功能回归使用 Linux Tauri / WebKitGTK；PCT-AL10 只执行 release arm64 专项查词、PSS 与交互验收，不启动 Android 模拟器；
- 私有 fixture、查询词、释义正文、资源内容、原文件名、完整路径和哈希不进入 Git、日志、截图或 benchmark 产物。

## Non-Goals

- MDict v1、口令或记录加密、DRM、KF8 词典、全文搜索、模糊查找、跨词典并行查询；
- Kindle 旧 names / keys 屈折变化、语言无关词干化或词形学；精确查词稳定后另行扩展；
- 在释义中渲染第三方 HTML、CSS、图片、音频或可点击外链；
- provider 注册表、动态插件 ABI、工厂、sidecar 数据库、后台服务或在线词典；
- 借词典 change 提前改造字号、DPI、滑动 / 滚动模式或设置菜单；这些按路线图在词典性能关闭后实施。

## Architecture Impact

present

- Design purpose: 让离线查词走有界随机读取和独立数据域，不污染书籍导入、阅读状态或诊断数据。
- Drivers / quality scenarios: `A-DICT-01` 要求不可信词典不能越界、全量展开或把正文写入日志；`A-DICT-02` 要求精确查词在 Linux 与 PCT-AL10 满足交互预算。
- Modules / interfaces: 后端 `dictionary` 模块拥有本地词典目录、静态格式分派与净化；Tauri command 只传词典 ID、查询和安全结果；`annotations.mjs` 只发送当前选区；Svelte 面板只负责管理与文本投影。
- Candidate and tradeoffs: MDict 直接使用一个固定依赖；Kindle 只借鉴 `libmobi 0.12` 的 INDX/TAGX 行为和 `boko 0.5.0` 的 HUFF/CDIC 算法，不引入 C FFI 或第二套运行时。打开 MDX 每次约 0.1 ms，先不做句柄缓存。
- Evidence / review trigger: 公共合成边界测试、私有样本 opt-in benchmark、Linux GUI 选词链路、PCT-AL10 release 查词与 PSS、独立 Spec / Standards review。只有 Android 数据证明重复打开或索引解析超预算时才增加缓存或 sidecar。

## Acceptance Criteria

- [ ] 用户可导入、列出和移除 MDict；重复导入幂等，损坏、超限、未来 schema 与半写入不会破坏既有词典；
- [ ] MDict v2 `encrypt=2` 私有样本可精确命中、miss、跟随有限深度链接，并对配套 MDD 做单资源范围读取；
- [ ] 经典 Kindle 私有样本可精确命中词典头、中、尾部条目，只读取索引、HUFF/CDIC 表和覆盖目标定义的文本记录；
- [ ] 阅读器选词后可查词，桌面与窄视口面板不遮挡关键控制、可滚动、可关闭，结果使用文本节点且不会发起网络请求；
- [ ] 日志只记录格式、匿名词典 ID、阶段、耗时、结果数量与错误码，不记录查询、释义、路径或资源；
- [ ] Linux release 热精确查词 P95 不高于 100 ms、冷查词 P95 不高于 500 ms、额外 RSS 不高于 64 MiB；PCT-AL10 使用相同预算并记录 PSS；
- [ ] Rust、Node、Svelte、Linux GUI、PCT-AL10、AutoCorrect、文档 gate 与独立 review 通过。

## Files And Steps

1. 先以 `LocalDictionaries` 公共行为测试固定事务导入、记录校验、MDict 精确查词、净化和私有样本入口，再实现 `mdict-rs` 适配。
2. 用公共结构测试固定 PalmDB、INDX/TAGX 与 HUFF/CDIC 边界，再实现 `KINDLE-D` 所需的最小精确查词路径并留下上游归属。
3. 增加 Tauri 管理 / 查词 command、原生 picker 和选词事件，在现有工具层增加词典面板，不建立 provider 框架。
4. 扩展正式检查脚本，先跑 Linux GUI 和私有样本 benchmark，再在 PCT-AL10 做 release 专项验收；更新事实所有者、路线图和 review。

## Checks

- `cargo test -p atha-backend --test dictionary_lookup`；
- `node --test reader/web/reader-state.test.mjs` 与相关 reader 单测；
- `pnpm --dir reader/app check`、`pnpm --dir reader/app build`；
- `pwsh -NoProfile -File scripts/check-dictionary-source.ps1 -PrivateFixtures fixtures/local -VerifyLinuxGui`；
- `pwsh -NoProfile -File scripts/check-dictionary-source.ps1 -PrivateFixtures fixtures/local -VerifyAndroid -Device 5ENDU19917001679`；
- workspace Rust、AutoCorrect、文档 gate 与 `git diff --check`。

## Rollback

移除词典 command、面板、后端数据域和新依赖即可；书籍库、消息、阅读状态、CSS 模块和统计 schema 不变。已导入词典目录可作为孤立本地数据保留或由用户显式删除，不自动删除用户源文件。

## Approval

用户已要求在成熟度与性能优先下完成本地词典，允许在 Kindle 现成库不合格时借鉴成熟算法实现最小解析器，并已提供 PCT-AL10 真机用于专项验收；本 change 不扩大到在线服务、动态插件或后续阅读设置重构。

## Result

待实施。

## Review

待实施后独立评审。

## Evidence And Residual Risks

当前只有引擎研究、Linux x86_64 P0 和 ADB 链路证据。MDict P0 三次精确查词 P95 为 1.653 ms、三次 MDD 流式读取 P95 为 1.452 ms、峰值 RSS 为 3308 KiB；尚不是产品链路、Linux GUI 或 PCT-AL10 结果。经典 Kindle 精确查词、旧 names / keys 屈折变化与所有 Android 预算仍未通过。

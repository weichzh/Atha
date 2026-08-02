# R1 阅读会话与多章节输入

## Status

implemented

## Problem

当前 reader 只能从 CLI 的单个 `--entry` 打开一份 XHTML；书籍内容没有版本、有序 section、资源集合或可选 TOC 的运行时契约。页面内容 module 持有一份隐式的当前 URL 和缓存，也没有可验证的关闭语义。继续实现 Locator 或跨章导航前，必须先让多章节输入与阅读会话成为稳定且受控的 seam。

用户指定 `fixtures/local/数学及其历史 (2026).epub` 作为 R1 真实样本。该文件是 EPUB 3.0，SHA-256 为 `0af5dff0c0d1eb369a096b18d05eb77a4cd9c03808748db8274d5e77bbfe7368`，含 173 个 spine section、200 条目录和 2701 个 manifest 项。R1 只从它可重复导出三个连续 XHTML section 及直接资源，不让运行时解析 EPUB。

## Scope

- 定义 schema 1 的最小书籍 manifest：64 位十六进制内容版本、有序且唯一的 section、显式资源集合和可选 TOC；
- manifest、section、资源与 TOC href 都限制为受控书根内的规范相对路径，拒绝编码绕过、绝对路径、查询、越界、重复项、未知字段和超量输入；
- Windows host 接受互斥的 `--manifest` 或既有 `--entry`，并继续通过 `BookRoot` 提供唯一书根；既有单章节 CLI 保持兼容；
- 新增一个 reading session module；其最小 interface 是按索引 `open`、`close` 和只读诊断快照，生命周期明确区分内容已加载、布局已稳定、关闭与失败；
- 每次 `open` 先释放前一 section 的 DOM、样式和缓存，再加载 manifest 声明的 section 与资源；`close` 后不保留书籍 DOM；
- 扩展本地样本导出器，以用户指定 EPUB 的连续 `ch012.xhtml`、`ch013.xhtml`、`ch014.xhtml` 生成忽略目录中的 R1 书根和 manifest；源 EPUB 与导出内容不提交；
- 正式 host 与 Agent Browser 验收确认三个 section 依次加载、前一 section 被释放、关闭后可重新打开，并继续覆盖明暗主题、安全、公式、普通图片和固定页面几何。

## Non-Goals

- 不在产品运行时解析、解包或导入 EPUB，不建立格式探测、格式工厂或多格式 adapter；
- 不实现 section 前后跳转控件、TOC 跳转、Locator、位置恢复或耐久状态；这些属于 R2 及以后；
- 不一次加载整本书，不缓存未打开 section，不增加数据库、worker、预取或持久缓存；
- 不改变分页算法、字号、公式倍率、页面设备像素、主题、外部网络策略、benchmark 阶段或遥测格式；
- 不放宽现有 active link、脚本、CSS 子资源和未知媒体拒绝规则。

## Acceptance Criteria

- [x] schema 1 manifest 可表示内容版本、三个以上有序 section、显式资源和可选 TOC；损坏、重复、越界或未知结构明确失败；
- [x] `ReadingSession.open(index)` 只加载目标 section，并依次产生内容已加载和布局已稳定状态；再次打开时前一 section 的内容与缓存被释放；
- [x] `ReadingSession.close()` 清空书籍 DOM 与当前缓存，随后可以重新打开；无效索引、缺失 section 和未声明资源明确失败；
- [x] Windows host 的 `--manifest` 与 `--entry` 互斥，manifest 仍受 `BookRoot` 路径、MIME、大小和符号链接边界约束；
- [x] 指定 EPUB 可重复导出三个连续 section，内容版本等于已记录源哈希，源文件和导出内容不被修改或提交；
- [x] R1 样本依次验证“1.1 算术与几何”“1.2 勾股数组”“1.3 圆上的有理点”，最终回到首章并通过实际 host、明暗浏览器、公式、2 张普通 PNG、无裁切和网络阻断检查；
- [x] 既有三个单章节样本与 reader benchmark 保持通过，独立 review 没有 blocking 项；
- [x] `READER-CORE`、代码地图、路线图、change 和 `ACTIVE` 与最终契约一致。

## Files And Steps

1. 用指定 EPUB 冻结内容哈希、连续 section、资源和样本边界；对照 Readest 的 `BookDoc.sections[].loadText/createDocument`、`load`/`stabilized` 生命周期与关闭清理，只采用契约和生命周期，不采用格式加载器或巨型 viewer；
2. 扩展 `BookRoot` 的 JSON 媒体支持、host 参数与资源交付，并补充后端和 CLI 负向检查；
3. 在页面新增 reading session module，把 manifest 校验、section 顺序、资源声明、打开和关闭隐藏在小 interface 后；内容和分页 module 只拥有单章加载与布局实现；
4. 扩展样本导出器与正式样本清单，生成用户指定 EPUB 的三章本地 fixture；
5. 运行静态、Rust、导出器、实际 WebView2、Agent Browser、benchmark、文档和独立 review，更新事实所有者并关闭 R1。

## Checks

- `python3 scripts/export_reader_sample.py --self-check`；
- 所有页面 JavaScript module 的 `node --check`；
- `cargo fmt --all --check`；
- `cargo clippy --workspace --all-targets --locked -- -D warnings`；
- `cargo test --workspace --all-targets --locked`；
- host 参数的 manifest、legacy entry、互斥与缺失负向检查；
- `pwsh -NoProfile -File scripts/check-reader-slice.ps1`；
- `pwsh -NoProfile -File scripts/check-reader-samples.ps1`；
- `pwsh -NoProfile -File scripts/Invoke-Atha.ps1 check docs -Activity validation -Scope reader-session-r1`；
- 本次中文 Markdown 的 `autocorrect --fix` 与 `autocorrect --lint`；
- `git diff --check`。

## Rollback

回滚本 change 的提交即可恢复单章节入口；R1 不迁移用户数据、不修改源 EPUB，也不新增耐久状态。

## Approval

用户明确指定《数学及其历史 (2026)》作为 R1 样本并要求“开始 R1”。本 change 以 R1 路线图范围和上述非目标为批准边界。

## Result

已实现受控 schema 1 manifest、互斥 host 入口和小接口 `ReadingSession`。内容 module 继续只负责一份 section；切章与关闭会清除上一章 DOM、书源样式和缓存。样本导出器仅在验证阶段从指定 EPUB 生成三章书根，产品运行时没有 EPUB importer。

## Review

- Blocking：独立规格与标准复查发现 TOC 重复项未拒绝、manifest 与索引失败未进入 `failed`、真实浏览器未直接证明三章标题和旧 DOM 释放、事实所有者尚未更新；均已修正并复查；
- Non-blocking：样本导出器的单章与多章路径仍有重复解包流程，host bundle 与验证服务器仍重复维护页面 module 顺序；当前实现清楚且检查覆盖，等真实修改需要同时触碰时再合并；
- Out-of-scope：无。

## Evidence And Residual Risks

- 本地静态与单元证据：导出器自检、全部页面 module 语法、Cargo fmt/clippy/test 通过；资源与遥测 3/3、host 参数 2/2 通过；
- 真实目标证据：实际 Windows host 和 Agent Browser 在明暗模式依次验证三个精确标题、两次旧 DOM 释放、关闭后重开、23 个公式、2 张普通 PNG、无裁切和外网阻断；既有三个单章节样本继续通过；
- 性能证据：10 次样本中位数为冷启动 772.623ms、首个稳定页 166.300ms、热打开 20.700ms、翻页 6.300ms、字号重排 20.800ms；没有旧代码的同时间对照，不声称 R1 带来性能改善；
- 一次全量浏览器检查无输出非零退出，针对 R1 与既有样本分别复现均通过，随后全量重跑也通过；未找到残留端口或稳定代码故障；
- R1 只验收指定 EPUB 的三个连续 section，不证明运行时具备 EPUB 导入能力，也不代表其余 170 个 section 已逐章验收。

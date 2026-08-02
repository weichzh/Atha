# R7 标注与引用

## Status

accepted

## Problem

阅读器已有稳定 range Locator、原生选择和每书状态，但还不能把一段正文保存为可恢复的用户事实。R7 需要闭合“选择 → 引用 → 标注事实 → 耐久写入 → 当前 section overlay”链路，并让未来消息链路可以直接消费同一份 `SourceAnchor`。

Readest 证明 selection、Locator、annotation store 与 overlay 应是分开的职责；其 notebook、同步索引、颜色系统和 renderer 适配层不进入 Atha 第一版。P0 `source_anchor` 只提供字段语义参考，R7 不提前实现 SQLite 或消息表。

## Scope

- 从当前 section 的单一非空原生选择生成严格 range Locator；原文最多 4096 个 UTF-16 code unit，笔记最多 2000 个；
- `SourceAnchor` 固定包含 schema 1 canonical Locator、原文、前后各至多 32 个 code unit 的上下文和原文 SHA-256；
- 每条标注保存稳定 id、`highlight` 或 `note` 类型、`SourceAnchor`、笔记、创建/更新时间与 `deletedAt` tombstone；
- 标注使用独立的每书 schema 1 记录，写入失败必须回滚内存变更并在标注面板报告；损坏记录保留在存储中且禁止覆盖；
- 当前 section 只从未删除的耐久标注投影 CSS Custom Highlight；切章、重开和重排后重画，不把 Range 或 overlay 当成事实；
- 同版本先验证 Locator 与原文；版本或文本不一致时，只在同 section 以唯一原文快照重锚，零个或多个候选都明确报告失败；
- 原生面板完成创建、选择、跳转、编辑笔记和软删除；四样本 runner 与《数学及其历史》覆盖真实选择、保存、重排、重开、跳转、tombstone 和跨 host 进程恢复。

## Non-Goals

- 不实现 notebook、标签、颜色选择、批注气泡、富文本、导出或外部标注导入；
- 不实现同步、冲突合并、tombstone 压缩、SQLite 迁移或消息写入；
- 不跨 section 建立选择，不进行模糊、跨章节或多候选重锚；
- 不接管浏览器复制，不用 DOM 包裹节点绘制高亮，不增加依赖或 renderer 抽象。

## Acceptance Criteria

- [x] 真实原生选择可创建 highlight 或带笔记的 annotation；其 `SourceAnchor` 的 range、原文、上下文与 SHA-256 均经过严格验证；
- [x] 标注写入与 overlay 分离；切章、字号/样式重排和页面重开后可重画，唯一原文可重锚，失败有明确状态；
- [x] 笔记更新和删除都先完成耐久写入再报告成功；删除只写 `deletedAt`，不物理移除事实；
- [x] 损坏或不可写存储不会被覆盖，不把失败伪装为成功，也不使 reading session 失败；
- [x] `SourceAnchor` 可由标注 id 直接导出，字段与 P0 `source_anchor` 的 canonical locator、原文、上下文和内容哈希语义一致；
- [x] 四样本实际 host、明暗浏览器、跨 WebView2 host 进程恢复、Rust 检查和 benchmark 均通过；
- [x] 独立规格与标准 review 无 blocking，事实所有者和 `ACTIVE` 与最终实现一致。

## Files And Steps

1. 增加独立 Annotation Store 与 Annotation module，前者拥有严格记录校验和事务式 localStorage 写入，后者拥有 `SourceAnchor`、唯一原文重锚和 CSS Highlight 投影；
2. 在现有书签面板内增加最小标注编辑区，并由 app 组合恢复、切章重画和状态探针；
3. 扩展 diagnostics 与正式 runner，验证真实选择、创建、编辑、重排、重开、跳转、软删除、写入失败和跨进程恢复；
4. 更新事实所有者，运行完整检查、benchmark 与独立 review。

## Checks

- 所有页面 JavaScript module 的 `node --check`；
- `cargo fmt --all --check`、`cargo clippy --workspace --all-targets --locked -- -D warnings`、`cargo test --workspace --all-targets --locked`；
- `pwsh -NoProfile -File scripts/check-reader-samples.ps1`；
- `pwsh -NoProfile -File scripts/check-reader-slice.ps1`；
- workflow `docs` gate；
- 本次中文 Markdown 的 `autocorrect --fix` 与 `autocorrect --lint`；
- `git diff --check`。

## Rollback

回滚本 change 的提交即可移除标注 module、UI 与独立 schema 1 记录。已写入的 `atha.reader.annotations.*.v1` 将成为无读取者的本地数据，但不会被回滚代码删除。

## Approval

用户明确授权依据路线图连续实现至 M2 结束，并认为现有 Readest 与微信读书研究足以完成整个 M2。本 change 只完成 R7 标注与引用。

## Result

新增独立 Annotation Store 与 Annotations module。Store 拥有 schema 1 记录、严格 `SourceAnchor` 校验、入站复制与冻结、写入成功后替换内存、损坏记录禁写和 tombstone；Annotations 从原生选择生成 SHA-256 Anchor，以 CSS Custom Highlight 投影当前 section，并在重排、重开和跨版本时验证或唯一重锚。

书签面板内增加最小标注区，完成创建、可选笔记、选择、跳转、更新和软删除。没有增加依赖、DOM 包裹、颜色系统、notebook、同步、SQLite 或模糊重锚。

## Review

- Spec：通过；审查发现 hash 未重算、非 canonical Locator、重锚写失败被覆盖、跳转绕过文本验证和 tombstone 重载证据不足，逐项修复后无 blocking；
- Standards：通过；审查发现选择缓存、暗色错误对比、嵌套 Anchor 可变引用和缺失 section 静默跳过，补齐边界与自检后无 blocking。

## Evidence And Residual Risks

- 最终 Windows 四样本正式 runner 通过（257 秒）。《数学及其历史》用真实鼠标创建带笔记标注，完成 32→40→32px 重排、暗色重载恢复、精确跳转、笔记更新、软删除与再次重载后的 0 active/1 tombstone，并通过两个真实 WebView2 host 进程恢复；
- 隔离自检覆盖 canonical Locator、UTF-16 长度、UTF-8 SHA-256、损坏记录保留、写失败回滚、原文零/多候选、缺失 section、外部引用不可变和软删除后按 id 导出；四样本既有安全、内容、交互、搜索和状态链路均保持通过；
- R7 benchmark run `1785705802647-45340` 的 10 样本中位数为：冷启动 820.208ms、首个稳定页面 180.000ms、热打开 23.650ms、翻页 7.100ms、字号重排 31.200ms；均在正式门槛内，未执行旧代码同时间对照；
- 最高证据等级为 Windows 本地真实 host 与真实浏览器；未在生产环境或完整 EPUB 导入链路执行；
- `replaceAnchor` 当前只由唯一重锚路径替换 canonical Locator，公开接口尚未收窄；读取异常统一报告为损坏；1000 条上限包含 tombstone。这些边界均不扩展 R7，后续只由真实需求触发。

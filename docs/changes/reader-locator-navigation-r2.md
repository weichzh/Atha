# R2 Locator 与导航

## Status

implemented

## Problem

R1 已能按索引打开多章节，但页码仍是唯一位置表达，上一页和下一页也只能在当前章节内移动。字号或样式重排后，同一页码不再代表同一内容；后续恢复、搜索、书签和标注都缺少可比较、可序列化的内容坐标。

Readest 证明 locator 应独立于显示页，并由渲染会话在 `relocate` 与稳定布局阶段消费；其 CFI、导航缓存和巨型分页 hook 不直接复制。Atha 当前只加载一份受控 XHTML，可先用 section 顺序和 DOM 文本偏移形成更小的稳定契约。

## Scope

- 定义 schema 1 的 point/range Locator：书籍内容版本、起点 section 与 UTF-16 文本偏移，可选终点；字段严格、长度有界，并按 manifest section 顺序比较；
- 文本偏移按 section DOM 的文本节点文档顺序计算，不使用显示页码；CSS 字号和布局变化不得改变 Locator；
- Locator 支持严格序列化、解析、比较、当前可见位置捕获和布局后定位；R2 range 限于单个 section，终点不得早于起点或超出实际文本；
- 未知版本、未知 section、越界偏移或损坏 Locator 不让阅读器崩溃，回落到安全的 section 起点并留下只读诊断状态；
- 新增一个导航 module，在小 interface 后组合 reading session、Locator 与 pagination；页内翻页到边界时进入相邻 section，并支持 manifest TOC 跳转；
- 工具栏使用原生 TOC `select`，保留既有上一页、下一页、页码和字号控件；不在 R2 设计沉浸控制层；
- 正式 WebView2 验证三章节前后导航、TOC 跳转、Locator 往返与比较、无效 Locator 回落，以及 24/32/40px 重排后的内容位置恢复。

## Non-Goals

- 不把 Locator、进度或偏好写入磁盘；R5 才增加耐久恢复和书签；
- 不实现搜索、选择、标注、引用、外部 CFI/XPointer 互操作或跨版本自动重锚；
- 不实现 EPUB importer、导航缓存、worker、预取或整本 DOM；
- 不提前实现 R3 的偏好模型和最终控制层视觉；字号控件只用于验证重排；
- 不复制 Readest 的 renderer adapter、CFI 包装层、React hook 或 store 结构。

## Acceptance Criteria

- [x] point/range Locator 可严格序列化、解析并按 manifest 顺序比较，损坏、未知字段、错书版本、未知 section、逆序 range 和超量输入均明确处理；
- [x] 当前可见内容可捕获为 Locator；24/32/40px 重排后恢复到包含同一文本偏移的页面，而非复用旧页码；
- [x] 上一页和下一页在章节边界进入相邻 section，首尾边界安全停留；
- [x] TOC 原生控件按 manifest 顺序跳转到对应 section；跳转后标题、当前 section、页码和控件状态一致；
- [x] 无效 Locator 回落到安全 section 起点，诊断快照记录回落原因，阅读会话保持可用；
- [x] 既有 legacy 单章节入口、R1 三章节样本、安全边界、公式、普通图片、明暗主题和 benchmark 保持通过；
- [x] 独立规格与标准 review 无 blocking，事实所有者和 `ACTIVE` 与最终实现一致。

## Files And Steps

1. 固定 Locator 领域语义和负向样例，只采用 Readest 的“内容坐标独立于显示页”原则；
2. 在页面新增 Locator module，并让 pagination 提供捕获与定位文本偏移所需的最小能力；
3. 新增 navigation module，统一拥有页、section、TOC 与 Locator 跳转；入口只组合 module；
4. 扩展正式诊断与三章节样本断言，覆盖重排恢复、TOC、首尾与安全回落；
5. 运行页面、Rust、实际 host、Agent Browser、benchmark、文档和独立 review，更新事实所有者并关闭 R2。

## Checks

- 所有页面 JavaScript module 的 `node --check`；
- `cargo fmt --all --check`、`cargo clippy --workspace --all-targets --locked -- -D warnings`、`cargo test --workspace --all-targets --locked`；
- `pwsh -NoProfile -File scripts/check-reader-samples.ps1`；
- `pwsh -NoProfile -File scripts/check-reader-slice.ps1`；
- workflow `docs` gate；
- 本次中文 Markdown 的 `autocorrect --fix` 与 `autocorrect --lint`；
- `git diff --check`。

## Rollback

回滚本 change 的提交即可恢复 R1；R2 不迁移用户数据，也不修改源 EPUB 或本地样本内容。

## Approval

用户明确授权继续实现到 M2 结束，并允许依据现有 Readest 与微信读书研究补齐规格。本 change 只实施路线图 R2，不把该授权扩大到后续阶段的具体代码。

## Result

已实现严格 point/range Locator、跨重排文本位置恢复和统一 Navigation module。上一页、下一页可跨 section，原生 TOC 控件可按 manifest 跳转；导航动作在同一入口串行化，错误 Locator 回落到安全起点并保留诊断。200% DPI 下的文本栏位按真实显示缩放归一化。没有增加持久化、CFI、缓存或后续阶段状态。

## Review

- Spec：初审发现 range 实际终点未检查、同 section TOC 项会复位、24px 恢复缺少直接断言；修复后复核为 blocking、non-blocking、out-of-scope 均无；
- Standards：初审发现栏位应使用 `floor`、导航动作需要串行、change 状态和 ROADMAP 过早完成；二次复核补充 200% DPI 坐标需按真实 scale 归一化。全部修复后复核为 blocking、non-blocking、Fowler smells 均无。

## Evidence And Residual Risks

- 静态与本地证据：页面 module 语法、Cargo fmt/clippy/test、资源与遥测 3/3、host 参数 2/2 通过；
- 真实目标证据：实际 Windows host 与 Agent Browser 在本机 200% DPI 下验证 Locator 往返、range 实际终点、多栏显示坐标、32→40→24→32px 逐次重排恢复、TOC 控件切章、并发导航串行化、前后 section、错版本回落、明暗主题、安全与既有内容断言；
- 性能证据：10 次样本中位数为冷启动 666.998ms、首个稳定页 177.850ms、热打开 20.800ms、翻页 6.300ms、字号重排 27.800ms；没有同时间旧代码对照；
- 诊断中发现两处验证竞态：导航过渡期快照曾把空 current section 当成错误，现统一返回 `current: null`；TOC runner 现等待 `layout-stable` 而非只等待 section id；
- 独立审查发现并修复实际 TOC 事件值捕获、异步 session 交叉、栏位四舍五入和 DPI 坐标系混用；正式检查包含对应并发与多栏回归断言；
- UTF-16 文本偏移只承诺同一内容版本内跨 CSS 重排稳定；内容版本变化后的重锚属于 R5/R7。

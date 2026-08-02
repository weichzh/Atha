# R4A 翻页输入

## Status

implemented

## Problem

R3 仍依赖常驻的上一页和下一页按钮，键盘、滚轮、页面点击与触摸没有统一进入 Navigation。直接在页面各处调用 Pagination 会绕过现有串行队列，也容易让点击翻页抢走文本选择或书内链接。

Readest 的滚轮实现证明触控板惯性尾流必须被视为同一次手势，但其 iframe 消息转发、平台状态与可配置手势不适合当前单 WebView 页面。Atha 只需要一个薄输入层，把明确的翻页意图交给 Navigation。

## Scope

- 新增一个页面输入 module，只识别无修饰键的方向键、Page Up/Page Down 和 Space、滚轮累计阈值、鼠标左右页区与单指横向滑动；
- 所有输入只调用 Navigation 的 previous/next，不直接修改分页或 section；
- 一次滚轮手势最多翻一页，惯性尾流在空闲窗口结束前被吞掉；
- 控件、链接、可编辑内容、非主按键与非折叠文本选择不触发页面点击或滑动翻页；
- 复用当前按钮作为可见且键盘可达的保底入口，不重做控制层。

## Non-Goals

- 不处理复制、书内/外链接、脚注、媒体放大、表格、代码或公式操作；后续 R4 change 各自验收；
- 不增加按键映射设置、手势设置、翻页动画、连续滚动、双页或 RTL；
- 不复制 Readest 的 iframe event bridge、React hooks、平台判断或复杂触摸仲裁；
- 不持久化任何输入偏好。

## Acceptance Criteria

- [x] 键盘、滚轮、鼠标页区和单指横向滑动均可前后翻页，并可跨 section；
- [x] 一次滚轮手势最多翻一页，阈值以下移动不翻页，空闲后可开始下一次手势；
- [x] 控件、链接、编辑区、组合修饰键和有效文本选择不会误触翻页；
- [x] 所有翻页仍经过 Navigation 串行队列，既有按钮、TOC、Locator 与排版恢复保持有效；
- [x] 四样本实际 host、明暗浏览器、安全断言、Rust 检查和 benchmark 保持通过；
- [x] 独立规格与标准 review 无 blocking，事实所有者和 `ACTIVE` 与最终实现一致。

## Files And Steps

1. 用最小纯判定固定滚轮阈值、触摸横向意图和受保护目标；
2. 在 reader 页面绑定键盘、滚轮和 pointer 事件，把动作交给 Navigation；
3. 扩展真实诊断和 Agent Browser 验收，覆盖正反翻页、惯性抑制、选择与控件保护；
4. 运行页面、Rust、实际 host、benchmark、文档和独立 review，更新事实所有者并关闭本 change。

## Checks

- 所有页面 JavaScript module 的 `node --check`；
- `cargo fmt --all --check`、`cargo clippy --workspace --all-targets --locked -- -D warnings`、`cargo test --workspace --all-targets --locked`；
- `pwsh -NoProfile -File scripts/check-reader-samples.ps1`；
- `pwsh -NoProfile -File scripts/check-reader-slice.ps1`；
- workflow `docs` gate；
- 本次中文 Markdown 的 `autocorrect --fix` 与 `autocorrect --lint`；
- `git diff --check`。

## Rollback

回滚本 change 的提交即可恢复按钮翻页；不涉及耐久数据或 schema。

## Approval

用户明确授权依据当前路线图继续实现到 M2 结束，并要求缺少规格时补规格。本 change 只落实 R4 的翻页输入切片。

## Result

已新增薄 `interaction` module：键盘、滚轮、鼠标页区和单指横向滑动只把前后翻页意图交给 Navigation。滚轮使用 60px 累计阈值与 240ms 空闲窗口，一次手势只触发一次；closed Shadow DOM 内外复用同一处理器并按事件去重，保留原生文本选择，跳过控件、链接、编辑区、修饰键和多指输入。没有增加手势设置、动画、平台分支或 iframe 转发层。

## Review

- Spec：初检发现 closed Shadow DOM 隐藏书内链接、Shift 仍翻页、已有选择的 down 状态丢失和双指未取消等 blocking；修复后复审无 blocking，保留“滚轮、鼠标和触摸跨 section 只由共同路径证明”的 non-blocking；
- Standards：同类 blocking 与 detached Promise 的未处理 rejection 均已修复，最终复审无 blocking、无 non-blocking。

## Evidence And Residual Risks

- 静态与本地证据：九份页面 module 语法、Cargo fmt/clippy/test、资源与遥测 3/3、host 参数 2/2 通过；
- 真实目标证据：四样本实际 Windows host 与 Agent Browser 明暗主题通过；每次真实诊断均执行键盘正反翻页、滚轮阈值与尾流抑制、鼠标左右页区、触摸左滑、非折叠文本选择、书内链接、原生控件、Shift 组合键和双指手势保护；多章节样本另执行键盘跨 section 往返；
- 性能证据：10 次样本中位数为冷启动 863.273ms、首个稳定页 194.800ms、热打开 20.700ms、翻页 6.150ms、字号重排 27.800ms；没有同时间旧代码对照；
- 触摸只实现单指、横向占优且至少 48px 的滑动；多指、笔输入、平台手势设置与真实触屏硬件验收不在本切片内。
- 正式跨 section 交互证据使用 Page Down/Page Up；滚轮、鼠标和触摸共享同一个 `run → Navigation` 入口，但未分别重复边界用例。

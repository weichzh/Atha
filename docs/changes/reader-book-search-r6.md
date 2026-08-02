# R6 书内搜索

## Status

implemented

## Problem

当前阅读器只能依靠目录和翻页定位内容。R6 需要在不改变阅读会话、不执行书籍内容的前提下搜索 manifest 声明的全部 XHTML，并用现有 Locator 把结果可靠地交回 Navigation。

Readest 证明了逐步结果、可替换查询和 Locator 导航的价值，也暴露出搜索选项、持久缓存、历史和全局 store 会迅速扩大职责。Atha 第一版只保留一个会话内的字面量、不区分大小写搜索。

## Scope

- 按 manifest section 顺序只读获取 XHTML，解析正文文本并生成 range Locator；搜索不加载资源、不执行脚本、不替换当前书籍 DOM；
- 一个查询最多 128 个 UTF-16 code unit，最多保留 2000 条结果；达到上限时明确报告；
- 新查询用 `AbortController` 取消旧查询，用户也可显式取消；旧查询不得覆盖新结果；
- 每条结果显示 section 标签和限长文本片段，并在跳转时由现有 Navigation 验证当前位置；明确隐藏的文本不进入结果，其他需完整渲染才能确定的候选可能在跳转时报告位置失效；
- 搜索错误只更新搜索面板，不把 reading session 置为失败；结果与状态只存在于当前页面；
- 四样本 runner 覆盖真实控件、查询替换、取消、错误隔离和 Locator 跳转；《数学及其历史》用“数”验证三个 section 的 66 条结果。

## Non-Goals

- 不实现正则、整词、模糊、词形、语言分词、大小写或变音符选项；
- 不实现搜索历史、持久缓存、数据库索引、worker、预加载队列或跨书搜索；
- 不在正文绘制搜索高亮，不保存搜索结果或把结果当作用户数据；
- 不改变内容安全、外部网络和 WebView IPC 边界。

## Acceptance Criteria

- [x] 四个样本均可从原生搜索控件得到完整、顺序稳定的结果；多章节样本覆盖全部三个 section；
- [x] 每条结果携带严格 range Locator；可定位结果跳转后目标 section 与文本起点在当前页可见，不可定位候选明确报告失效；搜索过程不替换当前会话内容；
- [x] 后发查询取消并替换先发查询，显式取消不会留下错误或让旧结果回写；
- [x] 无效查询和 section 获取/解析错误只在搜索域报告，阅读、翻页和已有用户状态继续可用；
- [x] 不增加依赖、worker 或持久缓存，删除搜索状态不会丢失用户数据；
- [x] 四样本实际 host、明暗浏览器、Rust 检查和 benchmark 均通过；
- [x] 独立规格与标准 review 无 blocking，事实所有者和 `ACTIVE` 与最终实现一致。

## Files And Steps

1. 增加会话内 Search module，以原生 fetch、DOMParser、AbortController 和现有 Locator 完成只读扫描；
2. 增加最小搜索面板、结果跳转和取消交互，并在 app 中组合；
3. 扩展诊断、四样本配置和正式 runner，验证结果数、section 覆盖、查询替换、取消与跳转；
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

回滚本 change 的提交即可移除搜索模块和 UI；搜索没有耐久记录或迁移数据。

## Approval

用户明确授权依据路线图连续实现至 M2 结束，并要求缺少规格时补规格。本 change 只完成 R6 书内搜索。

## Result

新增会话内 Search module 和原生搜索面板。Search 顺序获取 manifest 的 XHTML，拒绝 active content，屏蔽明确隐藏的正文文本，并用现有 Locator 生成 range 结果；新查询或取消通过 `AbortController` 终止旧扫描。结果跳转时确认起点在当前页可见，不可定位候选与其他错误只留在搜索面板。

实现只保留字面量、不区分大小写搜索、128 字符查询和 2000 条结果上限；没有依赖、worker、索引、历史或持久缓存。

## Review

- Spec：通过；审查发现隐藏文本和空格占位可能产生不可见或跨隐藏区伪命中，改用不可匹配的等长 NUL 哨兵并收窄动态可见性契约后无 blocking；
- Standards：通过；补齐搜索输入、搜索与书签面板在系统和显式明暗主题下的控件尺寸、焦点与颜色规则后无 blocking。

## Evidence And Residual Risks

- 最终 Windows 本地四样本正式 runner 在哨兵和主题修复后通过（259.5 秒）：三个单章节标题查询各 1 条；《数学及其历史》查询“数”得到 66 条并覆盖三个 section，明暗模式均通过真实控件与跨章来回跳转；
- 模块自检覆盖后发查询替换、显式取消、无效查询隔离和 active content 拒绝；四样本实际 host、既有交互、R5 跨进程状态恢复、Rust 检查与截图回归通过；
- R6 benchmark run `1785702017083-45900` 的 10 样本中位数为：冷启动 828.584ms、首个稳定页面 186.300ms、热打开 23.200ms、翻页 7.100ms、字号重排 31.400ms；
- 最高证据等级为 Windows 本地真实 host 与真实浏览器；未使用大于三章的整书样本，worker、索引和缓存继续由真实瓶颈触发；
- 2000 条截断边界由静态实现保证，正式样本最高 66 条，尚未构造上限样本。
- 搜索不会复制渲染器来计算未打开章节的完整可见性；`hidden`、`inert`、`aria-hidden`、`noscript` 和内联隐藏样式会用不可匹配的等长哨兵屏蔽并保持 UTF-16 offset，其他需完整渲染才能确定的候选在跳转时报告位置失效。

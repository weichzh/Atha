---
description: 本地阅读统计的计时语义、持久化和进度面板投影。
---

# 本地阅读统计

## Status

accepted

## Problem

Atha 已持久化阅读位置，但没有可解释的阅读时长和连续阅读统计。直接按页面打开时间累计会在失焦、后台、休眠和长期闲置时虚增；另建服务或数据库则会复制现有本地阅读状态边界，并增加不需要的迁移与 IPC 开销。

## Scope

- 只在书籍完成稳定排版、页面可见、窗口聚焦且最近 5 分钟有阅读活动时累计时长；失焦、隐藏和闲置立即暂停；
- 使用 15 秒心跳和单调时钟计量，超过 30 秒的间隔视为休眠或调度中断并丢弃，不用墙钟补时；
- 把有效短区间按本地日期跨午夜拆分，原子保存最近 400 天和至多 2048 本书的有界 schema 1 记录；
- 在现有进度面板投影今日、近 7 天、本书累计和连续阅读天数，复用微信读书 WR-05 的紧凑指标层级和 Readest RD-03 的安静工具层级；
- 用纯 Node 测试固定计时、失焦 / 闲置 / 休眠、跨日、损坏恢复和边界，再用 Linux Tauri / WebKitGTK 验证真实界面与生命周期。

## Non-Goals

- 账户、上传、跨设备合并、遥测、排行榜或社交统计；
- 单独的趋势页、图表、导出、目标设置或预计读完算法；
- 新数据库、后端 command、依赖、后台服务或 Android 日常验收；
- 为小于 1 分钟的偶然打开增加连续阅读天数。

## Architecture Impact

present

- Design purpose: 在现有阅读状态边界内保存可解释、不会因后台与休眠虚增的本地统计，不把产品数据混入诊断日志。
- Drivers / quality scenarios: `A-STATS-01` 要求失焦、后台、休眠和异常退出不虚增；`A-STATS-02` 要求日 / 周 / 书籍投影可复核且不影响翻页与排版。
- Modules / Interfaces / Seams / Adapters: `reader-state.mjs` 继续拥有浏览器本地状态，并增加独立的 reading-statistics interface；`app.mjs` 只连接 session、生命周期和现有输出；`ProgressPanel.svelte` 只负责投影。
- Candidate and tradeoffs: 不增加 SQLite。当前单窗口 WebView 已用 localStorage 保存位置和每书设置；一个受 512 KiB 上限约束的原子统计记录更短、更少阻塞，也没有数据库与 WebView 之间的定时 IPC。跨设备同步恢复时再重新评估持久化边界。
- Evidence / ADR / review trigger: Node 状态机测试、Svelte 检查、Linux 真壳失焦 / 恢复 / 重开和计时开销；只有记录接近容量上限、需要跨设备或多窗口并发写入时才升级存储。

## Acceptance Criteria

- [ ] 只有稳定、可见、聚焦且未闲置的阅读区间被计入；隐藏、失焦、5 分钟闲置、休眠长间隔和异常退出不会虚增；
- [ ] 有效区间跨本地午夜正确拆分，今日、近 7 天、本书累计与至少 1 分钟 / 天的连续阅读可从同一记录稳定推导；
- [ ] 损坏、未来 schema、超限记录和存储失败不阻断打开书籍，重开恢复已提交统计；
- [ ] 进度面板在桌面与窄视口不重叠，四项指标可扫描且不挤压进度控制；
- [ ] 计时心跳 P95 小于 5 ms，reader 初始 bundle 增量有记录，翻页、排版和 Locator 恢复门禁不回退；
- [ ] Node、Svelte、Rust、文档检查、Linux Tauri / WebKitGTK 和独立 review 通过。

## Files And Steps

1. 先写纯状态机测试，再在 `reader-state.mjs` 增加有界记录、投影和生命周期控制。
2. 把统计连接到稳定排版与现有进度面板，补充紧凑四指标布局。
3. 扩展正式 Linux GUI 检查，验证失焦、恢复、重开、窄视口和开销；更新事实所有者并 review。

## Checks

- `node --test reader/web/reader-state.test.mjs`；
- `pnpm --dir reader/app check`、`pnpm --dir reader/app build`；
- `pwsh -NoProfile -File scripts/check-fb2-source.ps1 -VerifyLinuxGui`；
- workspace Rust、AutoCorrect、文档 gate 与 `git diff --check`。

## Rollback

移除统计记录和进度面板投影即可；现有位置、偏好、书签、消息、书架和书籍内容 schema 不变。

## Approval

用户已要求按照路线图持续完成阅读统计，并要求 Linux GUI 作为日常目标验证；本 change 不扩大到同步、社区或 Android 日常验收。

## Result

待实施。

## Review

待独立评审。

## Evidence And Residual Risks

待实施后记录。

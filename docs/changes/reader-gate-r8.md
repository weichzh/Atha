# R8 阅读器门槛

## Status

implemented

## Problem

R0 至 R7 已闭合阅读会话、定位、排版、交互、进度、搜索和标注，但证据仍分散在阶段性 runner 与 benchmark 中。R8 需要把困难样本、大书、安全失败、进程崩溃后的恢复、内存和性能收成一个可重复门槛，并明确哪些指标会阻止阅读器 V1 交付。

本阶段不增加阅读功能。只有门槛测出超标或不稳定才修改产品实现；当前数据未证明需要缓存、worker、虚拟化或其他专项优化。

## Scope

- 验证导出器增加仅用于 fixture 的“全部 XHTML”模式，继续复用现有 ZIP、路径、单文件大小和资源边界，不把它当作 EPUB 产品导入；
- 从 SHA-256 `0af5dff0c0d1eb369a096b18d05eb77a4cd9c03808748db8274d5e77bbfe7368`、16.03MiB 的《数学及其历史》生成 173 section 的忽略目录压力 fixture，真实 WebView2 host 完成 manifest、前三节切换、安全探针和关闭重开；
- 固定全书查询“数学”，独立 oracle 为 288 条、覆盖 104 个 section；实际搜索必须 `complete`、未截断、无 section 错误并精确匹配两个计数；
- 大书内存连续测量三次；从进程启动起每 100ms 等待后，将 host 根进程与当时全部 WebView2 后代的 working set 求和；host 完成前三节、全书搜索与安全探针后发出验证完成信号，仅在门槛模式驻留至至少取得五个有效采样，随后由 gate 整树终止；每次必须观测到子进程，以三次各自峰值的最大值判定，无法完整采样或超过 1024MiB 时失败；
- 同一个 `write` probe host 写入并确认耐久化唯一的进度、偏好、书签和标注探针后，不触发正常 close/flush，直接终止根进程及全部后代；新 host 必须恢复并核对完全相同的四类探针；
- 为既有 10 样本 benchmark 增加 nearest-rank P95 门槛：冷启动 2000ms、首个稳定页 750ms、热打开 120ms、翻页 50ms、字号重排 150ms，并保留每项 10 个原始样本的 CSV 及汇总中的 P95、门槛和判定；
- 新总 gate 依次运行四困难样本、大书/崩溃/内存与 benchmark，并输出可追溯证据；独立复核事实所有者、module interface 和剩余风险。

## Non-Goals

- 不让产品 host 直接打开 EPUB，不解析 OPF spine、书架或真实导入身份；这些属于 M3；
- 不增加持久缓存、预热、worker、虚拟化、第二渲染引擎或性能模式；
- 不把本机一次测量泛化为跨设备承诺，不建立遥测服务、跨日期数据库或图表系统；
- 不为测试抽象新的 runner framework，不修改 R0 至 R7 的产品 interface，除非门槛发现 blocking。

## Acceptance Criteria

- [x] 四样本正式 host 与明暗浏览器总验收继续通过，外链、active content、路径越界、损坏状态和存储失败均安全失败；
- [x] 《数学及其历史》压力 fixture 的 source hash 和 173 个 section 精确匹配，实际 host 验证通过，且输出 section、资源与源文件规模；
- [x] 全书查询“数学”精确得到 288 条、覆盖 104 个 section，状态 complete、未截断且无错误；
- [x] 三次大书 host 的进程树峰值 working set 按固定协议可测，聚合最大值不超过 1024MiB；
- [x] 同一 `write` probe host 确认四类探针耐久化后被整树强杀，新 host 可恢复完全相同的探针；
- [x] 10 样本 benchmark 的五项 P95 均在固定门槛内，超标时脚本非零退出；
- [x] 未测得瓶颈时不增加优化实现；总 gate、Rust 检查、文档 gate 与中文排版检查通过；
- [x] 独立 review 对阅读器核心能力、事实所有者、module interface、真实证据和剩余风险无 blocking，M2 状态与 `ACTIVE` 一致。

## Files And Steps

1. 在既有安全 fixture exporter 增加全部 XHTML 验证模式和自检；
2. 给既有 benchmark 增加 P95 门槛，新增 R8 总 gate 组合四样本、大书、内存和崩溃恢复；
3. 只在总 gate 测得 blocking 时修改对应拥有者；否则保留现有产品代码；
4. 更新路线图与事实所有者，完成独立规格、标准与过度设计复核。

## Checks

- `python scripts/export_reader_sample.py --self-check`；
- `pwsh -NoProfile -File scripts/check-reader-gate.ps1`；
- workflow `docs` gate；
- 本次中文 Markdown 的 `autocorrect --fix` 与 `autocorrect --lint`；
- `git diff --check`。

## Rollback

回滚本 change 的提交即可移除 R8 fixture 模式、总 gate 和固定门槛；所有大书导出物、截图、日志与 benchmark 都位于已忽略目录，不产生数据迁移。

## Approval

用户明确授权设置 goal 连续实现至 M2 结束，并要求仅在 project-workflow 出现必须解决的问题时中断。本 change 只完成 R8 门槛与 M2 收口。

## Result

新增 fixture-only 全 XHTML 导出、固定搜索 oracle、WebView2 进程树内存采样、强杀恢复与 P95 失败门槛。验证完成后的驻留只供内存 gate 与强杀写入探针使用；强杀后必须确认已捕获的 host 与 WebView2 进程全部退出，才启动恢复探针。普通阅读、读取探针与 benchmark 退出路径不变。四样本 runner 使用带进程 PID 的唯一 Agent Browser session，可在上一次执行中断后直接重跑。状态持久化失败作为固定非内容错误码通过宿主边界，不再被降格为无效消息。

门槛未测出产品瓶颈，未增加缓存、worker、预热、虚拟化或新的产品 interface。M2 的 R0 至 R8 已完成；M3 尚未开始。

## Review

- Spec：独立复核发现并关闭“构建晚于证据”和“空快照计数”两个 blocking；驻留协议、固定 oracle 与最终 diff 无 blocking；
- Standards：独立复核项目契约、claim、事实所有者、module interface、安全边界和脚本失败语义无 blocking；
- Ponytail：独立复核无 blocking；删除 `crash-write` 测试别名，复用 `write + hold-after-verify`，并把本机实测值从架构移到本 change 与代码库地图。

## Evidence And Residual Risks

- 完整 gate：`pwsh -NoProfile -File scripts/check-reader-gate.ps1` 通过；四样本 host 与 Agent Browser 明暗验收全部通过；
- 固定源文件为 16.03MiB、173 个 section、2527 个资源；“数学”得到 288 条结果并覆盖 104 个 section；
- 三轮进程树峰值为 647.3、649.3 和 650.4MiB，每轮 5 个有效样本，峰值 8 个进程；强杀后确认已捕获的进程树全部退出再恢复，定向重复 8/8 与完整 gate 均通过；
- benchmark `1785710178116-34252` 的冷启动、首稳、热开、翻页与重排 P95 为 849.571、161.300、24.000、7.800 和 31.400ms，均低于固定门槛；
- 证据等级为当前 Windows 设备上的真实 WebView2 本地运行。未覆盖跨设备分布、生产安装包、真实 EPUB 产品导入或 OPF spine 语义；这些结果不授权提前实现 M3。

# Android 可诊断性基础

## Status

implemented

## Problem

Atha 当前只有 Windows 验收 host 的本地 benchmark Recorder、少量 `eprintln!` 和浏览器 `console.error`。安装后的产品没有统一、持久、有限轮转的日志；交互阅读的指标通常被丢弃，reader 失败跨 IPC 时又只保留错误码而丢失阶段。直接开始 Android 移植会让启动、导入、协议、阅读布局和持久化故障难以定位。

## Scope

- 采用官方 `tauri-plugin-log` 和 `log` facade，为 Windows 与后续 Android 共用 stdout / 平台 AppLog；
- 只记录固定 operation、stage、稳定错误码、耗时、计数和页面几何，不记录书名、路径、内容哈希、原文、笔记、查询或提示词；
- 记录应用启动、书架导入 / 打开、reader 首稳 / ready / failure，以及自定义协议的非预期内部失败；
- 把 reader failure 的稳定阶段 token 纳入现有 telemetry 白名单和测试；
- 保留 Windows benchmark Recorder，日志不替代正式 benchmark 制品。

## Non-Goals

- 不生成 Android 工程，不安装 target，不处理 Android content URI；
- 不引入 `tracing` subscriber、远程遥测、崩溃上传或全量 console forwarding；
- 不记录每次翻页、每个资源请求或预期的 4xx 安全拒绝；
- 不借日志改写 backend 错误模型、消息事实或 reader module 结构。

## Architecture Impact

present

- Design purpose: 在 Android 开发前建立不泄露阅读内容的最小持久诊断链；停止条件是固定故障可由 AppLog 中的 operation、stage 与 code 定位。
- Drivers / quality scenarios: `ASR-PRIV-01`（最高业务重要性 / 高技术风险，负责人：reader / Tauri Adapter）；刺激源是安装态应用、reader 或内部文件 I/O，刺激是启动、导入、打开、布局或协议内部失败，环境是 Windows 安装态及后续 Android 离线运行，制品是 Tauri Adapter、reader telemetry 与 AppLog，响应是只写固定 `atha::` 字段并有限轮转；度量是产品 smoke 取得正常打开与固定失败、敏感值和预期 4xx 日志命中均为 0、单文件不超过 1 MiB 且只保留两个归档。
- Modules / Interfaces / Seams / Adapters: reader telemetry Interface 增加白名单 stage；Tauri Adapter 记录跨 IPC / 文件 / protocol seam 的结果；官方日志插件负责平台输出和轮转。
- Candidate and tradeoffs: 采用 Tauri 官方插件；拒绝扩展 Windows Recorder，因为它依赖 Wry 与当前工作目录；拒绝 `tracing` 栈，因为当前没有 span、异步关联或 exporter 需求。
- Evidence / ADR / review trigger: `ADR-0007`；parser 单元测试、前端检查 / build、Rust 测试、Windows 产品日志 smoke；只有真实异步关联或跨进程分析不足时复查。

## Acceptance Criteria

- [x] 产品使用官方日志插件，最低生产级别为 Info，日志文件有显式大小上限和有限保留；
- [x] 启动、书架导入 / 打开、reader 首稳 / ready / failure 与 protocol 内部失败留下稳定字段；
- [x] reader failure 的 stage 由白名单 parser 校验，未知或非 ASCII 输入仍拒绝；
- [x] 日志不含 fixture 路径、书名、内容哈希、原文、笔记或查询；预期资源 4xx 不写盘；
- [x] Windows 正式最小检查保持通过，安装态 AppLog smoke 能看到一次正常打开和一次固定失败。

## Files And Steps

1. 更新 telemetry event 与测试，先锁定 failure stage 契约；
2. 接入官方日志插件并记录 Tauri Adapter 的关键阶段；
3. 更新架构、代码地图、外部参考和本 ADR；
4. 运行最小检查、产品日志 smoke、独立 review 和 required gate。

## Checks

- `cargo test -p atha-backend --test reader_slice`；
- `pnpm --dir reader/app check` 与 `pnpm --dir reader/app build`；
- `cargo test -p atha-reader-app`；
- Windows Tauri 产品日志 smoke；
- `scripts/Invoke-Atha.ps1 check docs`。

## Rollback

移除插件初始化、`log` 调用与 failure stage 字段，并恢复旧的两字段 reader error。没有 schema、用户数据或外部服务迁移。

## Approval

用户于 2026-08-07 明确要求在 Android 开发前完成日志检索，采用成熟日志库与最佳实践，并批准继续路线图开发；本 change 是该范围的首个最小切片。

## Result

Tauri 产品入口现使用官方 `tauri-plugin-log`，只持久化 `atha::` Info 以上固定字段，单文件 1 MiB 并保留两个归档。启动状态在 logger 初始化后打开；导入 / 打开、reader 首稳 / ready / failure 与 protocol 5xx 均留下稳定诊断，预期 4xx 静默。reader failure IPC 增加 backend 白名单 stage，旧 Windows Recorder 保持原职责。

## Review

- Blocking: 两轮独立 Spec / Standards review 的阻塞项均已修复；最终复核为 Blocking 0，Standards 另为 Non-blocking 0。
- Non-blocking: 带 CLI 参数的旧验证入口仍在 Tauri Builder 前解析参数和准备直接书籍；它不属于无参数安装态启动，继续由正式 runner / stderr 负责，Android 产品入口不复用该路径。
- Out-of-scope: Android 工程与 content URI bridge 进入下一 change。

## Evidence And Residual Risks

- 本地证据：backend reader slice 3 项、reader host 5 项、Tauri app 5 项测试通过；Clippy `-D warnings`、Svelte check 与 Vite build 通过。
- 真实 Windows 产品证据：`scripts/check-tauri-reader.ps1` 在最终代码上通过真实 Tauri / WebView2；10 次 P95 为 cold start 783.092 ms、first stable 211.900 ms、hot open 21.500 ms、page turn 6.800 ms、font reflow 48.600 ms，均低于正式门槛。
- 安装态 AppLog smoke：新增 5 条记录，包含正常 reader ready 与 `layout-stable / state-persistence` 固定失败；预期资源 404 没有 protocol 日志，fixture 路径、文件名、书名和内容标识命中数为 0，所有记录均来自批准的 `atha::` target。
- 一次正式 gate 的 EPUB 写入用例受 Windows 瞬态文件锁影响返回 `WriteFailed`；精确测试随后通过，完整 EPUB test binary 连续 20 / 20 通过，原正式 gate 重跑通过，因此没有添加掩盖根因的重试。
- 未覆盖：尚未生成 Android 工程，也没有 Android AppLog / logcat 或真机证据；这是下一 change 的首个验收目标。构建间歇报告 target incremental 目录无法 finalize，影响增量复用但不影响测试结果。

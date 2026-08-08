# 全仓日志缺口收口

## Status

implemented

## Problem

全仓审计确认现有持久日志已覆盖启动、导入与打开、reader、受控协议、消息导出及备份恢复，但书架列出与移出失败、消息常规命令的内部存储失败仍只向前端返回错误码。用户看到空书架、批量移出中断或消息读写失败时，AppLog 无法区分操作边界。

## Scope

- 复用现有 `tauri-plugin-log`、`log` facade、`atha::` target 过滤和固定错误码；
- 为书架列出与移出的内部失败记录固定 operation、stage、code 与耗时；
- 为消息命令的数据库、根目录、未来 schema 和损坏数据故障记录固定 operation 与 code；
- 保持无效输入、未知对象、版本冲突、预期协议 4xx 和前端可恢复状态静默；
- 增加最小分类测试，并复用现有 AppLog 隐私门，不增加依赖或新日志系统。
- 修复正式 Android 日志门对展开通知栏和错误显示尺寸的隐式依赖，固定验证 720 × 1280、320 dpi 的目标形状。

## Architecture Impact

none

本变更只补齐 `ADR-0007` 已定义的 Tauri Adapter 诊断责任，不改变 Module、Interface、数据、信任边界、依赖或日志保留策略。

## Acceptance Criteria

- [x] 书架列出和移出的内部失败可由固定 operation、stage 与 code 定位；
- [x] 消息查询与写入的内部存储故障可由固定 operation 与 code 定位；
- [x] 预期输入、并发和安全拒绝不写持久日志；
- [x] 所有产品 Rust 日志继续使用 `atha::` target，且不记录标题、路径、正文、查询、标识或内容哈希；
- [x] Android gate 在 UI 驱动前收起通知栏，并拒绝偏离 720 × 1280、320 dpi 的 AVD；
- [x] Tauri 单元测试、前端检查 / build、Rust 格式与 Clippy、现有日志隐私门和 required docs gate 通过。

## Files And Steps

1. 在 Tauri library 与 message command 边界增加最小内部错误分类和固定日志；
2. 用单元测试锁定需记录与需静默的错误集合；
3. 收起 Android 系统栏并校验固定显示形状，重跑日志隐私门；
4. 运行最小检查、独立 review 和 required gate；
5. 更新代码地图与当前执行指针并提交关闭。

## Checks

- `cargo test --locked -p atha-reader-app`；
- `pnpm --dir reader/app check` 与 `pnpm --dir reader/app build`；
- `cargo fmt --all -- --check` 与 Tauri Clippy；
- 现有 Tauri / Android AppLog 隐私检查中的可运行入口；
- `scripts/Invoke-Atha.ps1 check docs`。

## Result

书架 `list` / `remove` 与全部常规消息 command 现在共用各自的内部错误分类，只把固定 operation、stage、code 和耗时写入现有 `atha::` AppLog。无效输入、未知对象、revision conflict、备份 / 导出类外层错误和安全拒绝不进入该内部故障日志。

Android 正式 gate 会在每次启动前收起通知栏，并在构建和 UI 操作前拒绝偏离 720 × 1280、320 dpi 的显示环境。专用 AVD 的运行时显示和持久配置已恢复到该形状。

## Review

独立 Spec / Standards 复核未发现 blocking、non-blocking 或 out-of-scope 问题。复核补齐了 `MessageError` 全部维护 / 导出变体的静默分类测试。

## Evidence And Residual Risks

审计覆盖 74 个产品 Rust / TypeScript / Svelte / JavaScript 源文件、24 个 Tauri command 和全部现有运行时日志调用。11 个文件直接写运行时日志是分层结果，不要求每个领域文件自行写日志；backend 继续返回稳定错误，由 Tauri Adapter 在外部边界记录。P0 与检查脚本使用进程退出、控制台和本地 evidence，不接入产品 AppLog。

本地证据：Tauri app 10 项测试、Clippy `-D warnings`、Rust fmt、Svelte check 和 production build 通过。真实目标证据：API 36 x86_64、16 KiB 页、720 × 1280、320 dpi、WebView 133 的 `Atha_API_36_16K` 通过 README Markdown 的系统 picker、目录首 / 中 / 末、全书搜索、翻页、强停恢复、PSS、应用健康及 `logcat` + `Atha.log*` 隐私门。

首次正式门因模拟器通知栏展开而无法取得 `import-trigger`，同时暴露 AVD 被留在 320 × 640 显示形状。进程、启动日志、UI hierarchy 和截图将原因收敛到测试环境；收起通知栏并恢复固定显示后，失败阶段和完整门均通过。未覆盖 ARM64 真机、Windows AppLog 和生产包；本变更不把 AVD 结果称为真机性能证据。

## Approval

用户于 2026-08-08 明确要求先完成全仓日志缺口审计，再持续开发 Atha 路线图；本 change 是审计收敛出的首个最小修复切片。

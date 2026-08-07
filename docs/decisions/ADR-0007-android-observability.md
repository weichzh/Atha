# ADR-0007：Android 开发前的本地诊断日志

## 状态

accepted

## 日期

2026-08-07

## 背景

现有 `Diagnostics` / `Recorder` 属于 Windows Wry 验收 host：它依赖当前工作目录和 benchmark 参数，交互模式不保存多数 reader metric，也不能作为 Android 安装态日志。产品侧只有少量 stderr 与浏览器 console，应用重启后无法稳定回看。日志又位于阅读隐私边界，不能通过记录路径、书名、正文或查询来换取可诊断性。

## 驱动因素与场景

- `ASR-PRIV-01`（最高业务重要性 / 高技术风险，负责人：reader / Tauri Adapter）：
  - 刺激源：安装态应用、reader 或内部文件 I/O；
  - 刺激：冷启动、文件导入、书籍打开、布局或协议内部失败；
  - 环境：Windows 安装态及后续 Android 离线运行，正常启动或失败重启后；
  - 制品：Tauri Adapter、reader telemetry 与平台 AppLog；
  - 响应：只写 `atha::` 固定 operation、stage、code、耗时与计数，并执行有限轮转；
  - 响应度量：产品 smoke 同时取得一次正常打开和一次固定失败；书名、原始路径、正文、笔记、查询与内容哈希命中数为 0；预期 4xx 日志数为 0；单文件不超过 1 MiB，只保留当前文件与两个归档。
- 日志写入必须有容量上限，预期安全拒绝不得制造写盘洪泛；
- 正式性能结论仍来自有起止点、样本和设备指纹的 benchmark，不从普通日志推断。

## 决策

1. 使用 Tauri 官方 `tauri-plugin-log` 与标准 `log` facade。默认目标保留平台 stdout 和 AppLog，生产最低级别为 Info；target filter 只接受 `atha::`，避免依赖日志绕过字段约束。
2. 单文件达到 1 MiB 时轮转，只保留当前文件和最近两个归档；记录使用 UTC 和插件默认格式。
3. Tauri Adapter 只写固定键值：event / operation、outcome、mode、stage、code、duration_ms、count 与 reader 页面数值。禁止书名、路径、内容哈希、原文、笔记、查询和提示词。
4. reader error IPC 从 `error|code` 扩展为 `error|code|stage`；code 与 stage 都由 backend 白名单验证。用户可见中文阶段只留在 UI，不进入 IPC 或日志。
5. 预期的无书资源、非法路径、未支持 MIME 等 4xx 不记录；只有锁中毒、读取失败等 5xx 类内部错误记录稳定 code。
6. Windows benchmark Recorder 暂时保留，继续拥有正式 CSV 和安全探针；普通日志不复制其全量热路径指标。

## 候选与权衡

- 扩展现有 Recorder：拒绝。它是 Windows 验收 Adapter，依赖 Wry、当前工作目录和 benchmark 类型，移植会把验证 host 反向带入产品。
- 自建 `tracing` + subscriber：拒绝。当前只需要固定事件与本地 sink，没有 exporter、span 关联或多 subscriber 场景。
- 平台分别使用 Android logger 与 Windows 文件：拒绝。会形成两套字段、轮转和过滤行为。
- 官方 `tauri-plugin-log`：采用。它与现有 Tauri 版本同 release line，支持 Windows / Android、AppLog、stdout、级别过滤与有限轮转。

## 依赖评估

- 实际问题：需要 Tauri 已支持的跨 Windows / Android 本地 sink、级别过滤和轮转；标准库及现有 Windows Recorder 都不覆盖安装态产品日志。
- 许可证：直接依赖 `tauri-plugin-log 2.9.0` 为 `Apache-2.0 OR MIT`，`log 0.4.33` 为 `MIT OR Apache-2.0`；生产分发前仍运行整棵依赖树的许可证检查。
- 总成本与体积：只新增一个运行时插件并把锁文件已有的 `log` facade 设为直接依赖；`Cargo.lock` 新增六个 package 记录，无数据库、后台服务、JS 包或网络流量。Android APK 体积增量在首个 Android 基线中测量，明显超过诊断价值时移除插件。
- 锁定与学习：调用只依赖标准 `log` facade 和一个 Tauri Builder；没有自定义格式、远程协议或持久 schema，移除成本是删除插件初始化与调用。
- 支持、更新与安全：插件由 Tauri 官方 plugins workspace 维护，并跟随 Tauri 2 release line；升级时使用锁文件、正式 Windows / Android gate，并在发布前执行依赖公告检查。
- 离线、数据与隐私：运行完全离线，只写平台本地日志；target filter 与字段白名单在 sink 前生效，不上传、不记录阅读内容，也不引入用户配置或密钥。

## 后果

- 正面：Android 与 Windows 共用一套 Rust 日志入口，故障重启后仍可回看；
- 正面：日志容量与隐私字段有明确上限，不需要远程服务或新配置系统；
- 负面：首切不持久化浏览器全部 console，也不提供跨事件 correlation id；
- 负面：普通日志只能提示性能异常，不能替代真机 benchmark。

## 假设

- 无参数安装态的产品启动会在 Tauri plugin 初始化后进入 `setup`；带 CLI 参数的旧验证入口仍由正式 runner 与 stderr 诊断，不作为 Android 产品入口。
- 平台 AppLog 目录在正常安装账户下可写；Android 上的实际目录、logcat 可取得性与轮转行为必须由下一 change 的设备 smoke 验证。
- P0 Android 故障可以先由固定 operation、stage、code、耗时与计数定位；出现真实跨异步关联证据前不引入 correlation id 或 tracing。

## 风险与缓解

- 插件升级可能改变默认 sink 或轮转语义：锁定 `Cargo.lock`，升级随 Tauri release line，并由 Windows / Android AppLog smoke 复核容量与保留数。
- 日志字段可能意外带入阅读隐私：sink 前只接受 `atha::` target，reader IPC 对 code / stage 做白名单校验，产品 smoke 对路径、文件名、书名与内容标识执行零命中断言。
- 平台日志目录不可写时故障仍可能只剩 stderr / logcat：启动状态错误使用固定 code，并把 Android AppLog / logcat 读取列为首个移动端验收；不得用远程上传绕过该风险。
- 带 CLI 参数的旧验证准备发生在 logger 前：保留为已知非产品路径，由正式 runner 报错；只有它进入安装态产品入口时才移动到 plugin 后。

## 取代关系

本 ADR 不取代既有 ADR。它补充 Tauri 产品 adapter 的诊断责任，并保留 Windows benchmark Recorder；后续若改用 tracing、远程 exporter 或平台专用 sink，新的 ADR 必须明确取代本决定。

## 回滚与复查

没有数据迁移；可直接移除插件与调用。出现真实跨异步链关联、多个 sink 策略、结构化查询或经批准的崩溃上传需求时，重新评估 `tracing` 或 exporter；普通本地日志不足的证据出现前不升级。

## 实施与检查位置

- Tauri Adapter：`reader/app/src-tauri/src/lib.rs`；
- reader telemetry：`reader/web/app.mjs`、`backend/atha-backend/src/reader/telemetry.rs`；
- 契约测试：`backend/atha-backend/tests/reader_slice.rs`；
- 产品检查：`scripts/check-tauri-reader.ps1` 与 required `docs` gate。

## 相关资料

- Tauri Logging：<https://v2.tauri.app/plugin/logging/>
- 官方插件源码：<https://github.com/tauri-apps/plugins-workspace/tree/v2/plugins/log>

# 消息快照资产进程中止恢复

## Status

accepted

## Problem

`MessageStore::prepare_resources` 当前把资源字节直接写入最终 SHA-256 文件名，随后才提交 `snapshot_resource` 数据库事实。若写文件时进程中止，最终路径可能保留截断内容；若后续数据库事务失败或在提交前中止，则会留下没有数据库引用的孤儿资产。现有 `MessageStore::open` 不清理这些文件，正确哈希名下的截断内容还会阻止同一资源重试。

只清理孤儿不能解决截断文件，只把资源改存 SQLite BLOB 又会扩大数据库和迁移范围。当前最高风险是进程级失败原子性，不是存储可替换性。

## Scope

- 所有 SourceSnapshot 资源在 SQLite `IMMEDIATE` 写事务取得后才开始发布；
- 新资产先以同目录独占临时文件写入，完成 `write_all` 与 `sync_all` 后再 rename 到最终内容哈希名；
- `MessageStore::open` 在同一 SQLite 写锁下删除 Atha 临时资产和数据库未引用的规范哈希文件；
- 已引用资产、非 Atha 文件和未知目录不得被清理；已引用资产损坏必须由读取和 `health` 明确报告，不能静默删除或覆盖；
- 保持 `MessageStore` 公开 Interface、schema、DTO、稳定错误、Tauri command、依赖和正常产品行为不变；
- 更新消息架构、数据库事实与系统风险顺序，并完成专项检查和独立 review。

## Non-Goals

- 本 change 不实现用户发起的完整备份 / 恢复、加密、checkpoint、同步或恢复 UI；这些会新增独立用户 Interface，进入下一份 change；
- 不声称抵抗磁盘、文件系统或硬件损坏；本轮响应目标是正常文件系统上的进程中止和已返回的 I/O / 数据库失败；
- 不把快照资源改存 SQLite BLOB，不增加 storage trait、后台清理器、定时任务、锁文件、crate 或依赖；
- 不物理删除 Message、Snapshot 或其正常资产。

## Architecture Impact

present

- Design purpose：关闭 `ASR-DATA-01` 中数据库事务之外的快照资产失败窗口；重开后自动回到可重试状态；
- Module / Interface：恢复协议留在既有 deep Module `backend::messages::MessageStore` 内，调用方继续只使用 `open`、写入、读取和 `health`；不新增外部 Seam 或 Adapter；
- Quality scenario：见下表；失败注入测试在正式 `MessageStore` Interface 上验证数据库、文件与重开结果；
- Candidate：采用同目录临时发布 + SQLite 写锁协调 + open 清理；拒绝直接写最终文件和 SQLite BLOB 迁移，理由见 ADR-0005；
- Evidence / review：Rust interface 集成测试、fmt、clippy、消息专项、docs gate 与 Standards / Spec review；完整备份 / 恢复和硬件损坏是明确的后续触发器。

| 字段 | `ASR-DATA-01A` |
| --- | --- |
| 刺激源 | 应用进程或已注入的文件 / SQLite 故障 |
| 刺激 | 在资源写入、发布或数据库提交任一点中止 / 失败 |
| 环境 | 正常本地文件系统、单一 Atha 数据根，可有另一个遵循同协议的进程 |
| 制品 | `Messages.sqlite3`、`snapshot_resource` 与 `Messages/Assets/` |
| 响应 | 临时文件永不作为正式资产读取；数据库提交前资源已完整发布；重开在 SQLite 写锁下删除临时和未引用资产，保留引用及未知文件 |
| 响应度量 | 重开后零 Atha 临时文件、零未引用规范哈希文件、零部分 Message 事实；有效资源仍按原字节读取，同一失败写入可重试成功 |

## Acceptance Criteria

- [x] `create_root` 与 `reselect` 都在 `IMMEDIATE` transaction 内准备资源；
- [x] 新资产通过 `create_new` 临时文件、完整写入、`sync_all` 和同目录 rename 发布，最终文件名继续是小写 SHA-256；
- [x] 进程中止遗留的 Atha 临时文件和数据库未引用的 64 位小写哈希文件在下次 `open` 时清理；
- [x] 清理与写入使用同一 SQLite writer lock 协调，不删除已引用资产、未知文件、目录或可能正在发布的资产；
- [x] `health.integrity` 同时覆盖 SQLite / 外键与已引用资产的存在、普通文件类型、长度和哈希；损坏资产不被静默修复；
- [x] 失败注入回归证明数据库事务回滚、截断孤儿被清理、未知文件保留、重试成功且后续重开仍可读取；
- [x] 不改变 schema、公开 DTO / error / command，不增加依赖或抽象；
- [ ] 中文 Markdown、目标检查、required gate、diff 检查和独立 Standards / Spec review 通过。

## Files And Steps

1. 用现有 outbox 失败注入测试固定资源孤儿、截断文件和重开恢复场景。
2. 在 `MessageStore` 内实现临时发布、引用完整性检查与 open 清理，并把两个资源写入口移入 `IMMEDIATE` transaction。
3. 更新 `OVERVIEW`、`MESSAGE-READING`、`DATABASE` 与 ADR-0005。
4. 运行专项检查、候选 required gate 和双轴 review，记录证据并关闭任务。

## Checks

- `cargo test -p atha-backend --test message_reading`；
- `cargo fmt --all --check`；
- `cargo clippy -p atha-backend --all-targets -- -D warnings`；
- `pwsh -NoProfile -File scripts/check-message-reading.ps1`；
- `autocorrect --fix` / `autocorrect --lint` 仅作用于本次中文 Markdown；
- `python scripts/doc_guard.py`、`python scripts/doc_length_check.py`、`git diff --check`；
- `project_workflow.py station message-snapshot-asset-recovery --activity verification --gate docs`。

## Rollback

回退 `store.rs` / `write.rs`、测试和文档即可；不涉及 schema 或已有数据迁移。回退后已经发布的内容寻址资产仍可读取，但重新暴露直接写最终文件和孤儿不清理风险。

## Approval

用户于 2026-08-07 要求继续按已确定计划执行；该计划把快照资产进程中止恢复列为 Atha 下一项 P0 架构风险。本 change 只实施其中不新增用户 Interface 的自动恢复切片，完整备份 / 恢复单独设计。

## Result

- `create_root` 和 `reselect` 现在先取得 SQLite `IMMEDIATE` transaction，再准备 SourceSnapshot 资源；
- 新资源通过独占临时文件、`write_all`、`sync_all` 和同目录 rename 发布；最终名称仍是内容 SHA-256，既有有效资产继续去重复用；
- `MessageStore::open` 在同一 writer lock 下删除 Atha 临时文件和未引用规范哈希文件，保留引用资产、未知文件和目录；
- `health.integrity` 现在同时检查数据库、外键以及每个已引用资产的类型、长度、名称与实际 SHA-256；
- 未改变 schema、公开 Interface、DTO、错误代码、Tauri command、crate 或依赖。

## Review

- Blocking：待 review。
- Non-blocking：待 review。
- Out-of-scope：待 review。

## Evidence And Residual Risks

- 实施前 as-built 静态证据：只有 `create_root` 与 `reselect` 调用资源准备；旧实现直接 `create_new` 最终哈希文件并在数据库事务外或事务提交前写入；`open` 只迁移 schema；
- 官方语义证据：Rust `create_new` 提供原子独占创建，`File::sync_all` 报告关闭时可能遗漏的写回错误，`fs::rename` 在同一文件系统移动名称；
- Red：目标失败注入测试在旧实现的重开步骤观察到截断孤儿仍存在，按预期失败；
- Green：同一测试在新实现通过，并证明事务事实回滚、Atha 临时 / 截断孤儿清理、未知文件保留、同资源重试和再次重开读取；
- Windows 本地：16 项 `message_reading` interface 集成测试、fmt、backend clippy `-D warnings`、中文 Markdown lint、doc guard、doc length 与 diff 检查通过；
- Windows 本地完整消息专项：16 项后端集成测试、Markdown 测试、Svelte check / production build、3 项 Tauri app 测试与 5 项旧 host 测试通过；
- 证据边界：本轮最高证据为 Windows 本地测试 / build 与静态检查，没有执行真实 Tauri / WebView2 交互或进程强杀；
- 并发假设：所有正式资源发布和 open 清理均先取得 SQLite `IMMEDIATE` writer lock；若以后增加绕过该锁的资源写入口，必须重评本协议；
- 残余风险：目录项在断电后的耐久性依赖操作系统、文件系统与硬件；完整备份 / 恢复、加密和 checkpoint 仍未实现。

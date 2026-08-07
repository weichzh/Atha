# ADR-0005：消息快照资产进程中止恢复

## 状态

accepted

## 日期

2026-08-07

## 背景

`MessageStore` 把快照资源按 SHA-256 保存为 SQLite 外部文件。数据库事实由 `IMMEDIATE` transaction 和 WAL 保护，但资源当前直接写入最终名称：进程在 `write_all` 中止可能留下截断文件，数据库提交失败则留下孤儿。后续同一哈希写入会把截断文件识别为损坏并拒绝，`open` 也不会恢复。

资源是不可变、内容寻址且正常产品路径从不物理删除。只有 `create_root` 与 `reselect` 两个写入口，这使数据库 writer lock 可以同时作为资源发布与清理的现有协调机制。

## 驱动因素与场景

- `ASR-DATA-01A`：进程在资源写入或 SQLite 提交任一点中止后，重开必须删除临时 / 未引用资产、保留有效资产并允许重试，不能留下部分 Message 事实；
- `ASR-REF-01`：已提交 SourceSnapshot 的资源字节与 SHA-256 必须继续不可变，损坏不能被静默覆盖；
- 范围只覆盖正常本地文件系统上的进程中止和已返回故障，不替代完整备份 / 恢复或硬件损坏防护。

## 假设

- 所有正式 SourceSnapshot 资源写入都经过 `MessageStore`，并在 SQLite `IMMEDIATE` transaction 取得后执行；
- 正常数据不会物理删除 Snapshot / Resource，因此数据库引用集合在本轮只增长；
- `Assets` 与临时文件位于同一目录和文件系统；
- 应用数据目录不是跨主机共享文件系统，外部程序不会同时改写 Atha 管理的规范哈希文件。

## 决策

1. 新资产写到 `Assets` 内由 Atha 命名的独占临时文件；`write_all` 和 `sync_all` 成功后，以同目录 `rename` 发布到最终小写 SHA-256 名称。
2. 已存在最终资产只在其为普通文件且字节哈希与名称一致时复用；损坏、目录或 symlink 均返回稳定损坏错误，不静默修复。
3. `create_root` 与 `reselect` 在资源发布前取得 SQLite `IMMEDIATE` transaction；数据库只在全部资产发布成功后登记资源并提交。
4. `MessageStore::open` 在迁移后取得同样的 writer lock，删除 Atha 临时资产和数据库未引用的规范哈希普通文件 / symlink；未知名称和目录不删除。
5. `health.integrity` 在原 SQLite / 外键检查之外验证每个已引用资产的普通文件类型、长度与 SHA-256。
6. 恢复完全隐藏在现有 `MessageStore` Interface 后，不增加 storage port、后台 worker、定时器或新公开维护操作。

## 候选、理由与证据

- 保持直接最终写入，只在错误返回时删除：否决；无法覆盖进程中止，也可能误删既有共享资产。
- **临时发布 + SQLite writer lock + open 清理**：采用；复用现有 transaction、标准文件 API 与内容寻址事实，只触碰两个真实写入口。
- 把资源改存 SQLite BLOB：否决；虽然可获得单事务，但会改变 schema、数据库体积、查询与导出行为，当前没有收益证据支持迁移。
- 增加资源 journal、storage trait、锁文件或后台垃圾回收：否决；现有 SQLite writer lock 已能协调全部写入口，额外协议只增加恢复状态。

Rust 标准库文档确认 `create_new` 是避免 TOCTOU 的原子独占创建，`File::sync_all` 用于显式处理持久化错误；SQLite 官方文档确认事务在进程 / OS 中止后自动恢复，但外部资产必须由应用单独协调。代码审计确认当前只有两个资源写入口且没有正常物理删除。

## 后果

- 正面：截断字节不再出现在最终名称；数据库失败最多留下可在重开清理的完整孤儿；重试不会被截断文件永久阻塞。
- 正面：调用方、schema、DTO、错误代码和依赖不变，恢复复杂度集中在 `MessageStore`。
- 负面：资源首次写入增加一次 `sync_all` 和 rename；`open` 需要扫描 `Assets` 并读取数据库引用集合。
- 负面：扫描成本与资产文件数线性相关；当前本地单用户规模可接受，只有启动实测超出门槛时才引入增量 GC。

## 风险与缓解

- 清理与活跃写入竞态：两个操作都持有 SQLite `IMMEDIATE` writer lock；新增绕过入口时 review 必须拒绝。
- 误删用户文件：只删除 Atha 临时前缀或 64 位小写哈希且未被数据库引用的文件 / symlink；未知名称和目录保留。
- 已引用损坏被误当孤儿：引用集合优先于文件内容；已引用项不清理，由读取和 `health` 明确报损坏。
- 断电后 rename 目录项耐久性：写入前执行 `sync_all`，但不把本轮本地验证提升为硬件保证；完整备份 / 恢复仍是独立 P0 场景。

## 实施与检查位置

- 实施：`backend/atha-backend/src/messages/store.rs`、`backend/atha-backend/src/messages/write.rs`；
- 失败注入：`backend/atha-backend/tests/message_reading.rs`；
- 事实：`docs/codebase/DATABASE.md`、`docs/architecture/MESSAGE-READING.md`、`docs/architecture/OVERVIEW.md`；
- 正式检查：`scripts/check-message-reading.ps1` 与 required `docs` gate。

## 回滚与替代

没有 schema 或数据迁移，可直接回退实现。若资产规模使 open 扫描不可接受，以测量支持的增量清理取代本决定；若需要抵抗硬件损坏或跨设备恢复，以完整备份 / 恢复 ADR 补充而非静默扩大本协议。

## 复查触发器

- 出现第三个资源写入口、正常物理删除或绕过 SQLite writer lock 的调用；
- `Assets` 扫描使启动时间超过既有门槛；
- 数据目录进入共享 / 网络文件系统；
- 完整备份 / 恢复、加密、同步或硬件故障恢复进入实施范围。

## 取代关系

本 ADR 不取代既有 ADR，当前也未被其他 ADR 取代。

## 相关文档

- 当前 change：`docs/changes/message-snapshot-asset-recovery.md`
- 系统架构：`docs/architecture/OVERVIEW.md`
- 消息架构：`docs/architecture/MESSAGE-READING.md`
- 数据库事实：`docs/codebase/DATABASE.md`
- SQLite 官方恢复说明：<https://sqlite.org/howtocorrupt.html>
- Rust 标准文件接口：<https://doc.rust-lang.org/std/fs/>

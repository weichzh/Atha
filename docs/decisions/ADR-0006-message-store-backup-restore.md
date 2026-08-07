# ADR-0006：消息存储完整备份与恢复

## 状态

accepted

## 日期

2026-08-07

## 背景

`MessageStore` 的完整事实由 WAL 模式 SQLite 和外部内容寻址资产共同组成。现有消息导出服务于按书 / 会话交换：它重建业务 manifest、限制范围且没有导入路径，不能作为数据库灾难恢复。直接复制活动 `Messages.sqlite3` 可能遗漏 WAL 中的已提交页；直接替换正在使用的数据库文件又会绕过 SQLite 锁与连接语义。

前一决定已保证资产发布、重开清理和进程中止后的可重试性。下一项 P0 是让用户取得一个可验证的全 MessageStore 制品，并在失败时保持当前事实不变。

## 驱动因素与场景

- `ASR-DATA-02A`：WAL 数据库仍可写时，备份必须是一个一致数据库 snapshot，并包含该 snapshot 引用的全部且仅有资产；
- `ASR-DATA-02B`：损坏制品或 SQLite busy / I/O 返回失败时，当前数据库不得变成部分恢复状态；
- `ASR-DATA-02C`：成功恢复后，所有备份事实和资源可读，备份后新增事实消失，重开结果相同；
- 备份 / 恢复是 MessageStore 维护 Interface，不改变按书交换导出，也不拥有书籍、偏好或阅读状态。

## 假设

- 正式 SourceSnapshot 资产不可变，发布发生在数据库提交前，正常产品路径不物理删除；
- 所有 Atha open / backup / restore 都遵循同一个维护锁；普通事实写入继续使用 SQLite transaction；
- 制品来自用户选择的本地文件，但仍按不可信输入验证；不把校验等同于加密或真实性认证；
- V1 只恢复当前正式 database schema；将来 schema 升级时必须显式定义旧备份迁移，不静默猜测。

## 决策

1. `MessageStore::create_backup` 使用 SQLite Online Backup API 把活动 main database 复制到暂存数据库；不复制活动 DB、`-wal` 或 `-shm` 文件。
2. schema 1 `.atha-backup` 是 ZIP：`manifest.json` 记录数据库哈希 / 长度及排序后的资产哈希 / 长度，`Messages.sqlite3` 是一致 snapshot，`assets/<sha256>` 是该 snapshot 引用的完整资产集合。
3. 备份写到目标同目录独占临时文件；ZIP 完成、`sync_all` 并重新读取校验后，才以标准库 hard link 原子创建不存在的最终路径并删除临时名。hard link 在目标已存在时失败，消除“先检查、后 rename”覆盖竞态；进程中止最多留下显式 `.tmp` 或完整最终制品，不会发布半成品。
4. `restore_backup` 先验证 archive 唯一 / 已知 entry、容量边界、manifest、数据库 / 资产哈希与长度，再打开暂存数据库验证当前 schema 精确签名、`integrity_check`、`foreign_key_check`、消息关系、Edition / 修订 / Locator / Snapshot 内容、Outbox、旧迁移凭据、FTS NULL-safe 精确投影和资产引用集合；任何正式写入都发生在完整验证之后。
5. 恢复先在 SQLite `IMMEDIATE` writer lock 下复用既有原子资产发布，再通过 Online Backup API 把暂存数据库复制到活动数据库。SQLite 在 backup sequence 未完成时回滚 destination write transaction；已提前发布但未引用的资产由下次 open 清理。
6. `Assets/.atha-maintenance.lock` 是具体生命周期协调文件：open / recovery 持 exclusive lock，备份持 shared lock，恢复持 exclusive lock。启动恢复会删除 Atha 暂存文件，因此必须与备份 / 恢复互斥；并发备份可以共享锁。它不发展为通用锁服务。
7. 书架 Tauri Adapter 负责 save / open dialog、书架路由校验与 blocking worker；backend 独占 archive、SQLite、资产和恢复语义。恢复前 UI 明确确认“替换全部消息事实”。

## 候选、理由与证据

- 普通复制 `Messages.sqlite3` 并猜测 WAL：否决；SQLite 官方明确要求对活动数据库使用 Online Backup API、`VACUUM INTO` 或持锁复制，普通副本可能损坏或丢失已提交事实。
- 复用按书 JSON 导出并实现 merge import：否决；它遗漏完整数据库状态且引入 ID 冲突、合并和幂等语义，不是灾难恢复的最短路径。
- 关闭应用后替换整个 `Messages/` 目录：否决；UI / 多进程生命周期和 Windows 打开句柄使离线 swap 需要重启编排与额外状态，当前 Online Backup API 已提供 destination 原子事务。
- `VACUUM INTO` 备份 + 文件替换恢复：部分可行但否决；备份简单，恢复仍不能安全替换活动数据库。统一使用 rusqlite 官方 `backup` feature 直接覆盖两向 copy，且不增加 crate。
- **Online Backup API + ZIP + 内容寻址资产 + 具体维护锁**：采用；复用 SQLite 事务、现有 zip / SHA-256 与资产发布，只有一个新 backend Module 和两个外部操作。

SQLite 官方文档保证成功 backup sequence 产生源数据库一致 snapshot，destination 在 sequence 未完成时回滚；Rust 1.97 `std::fs::File` 已稳定提供 Windows `LockFileEx` 与 Unix `flock` 对应的 shared / exclusive lock，无需新依赖。

## 后果

- 正面：备份不依赖 WAL 文件时机，恢复失败不会留下部分 SQLite copy；资产完整性与数据库引用在触碰正式事实前验证。
- 正面：制品单文件、用户可移动；backend 与 Tauri / UI 职责清晰，按书交换导出保持原语义。
- 正面：只启用既有 rusqlite feature，复用现有 ZIP、哈希、writer lock 与资产发布，不增加 storage / repository 抽象。
- 负面：完整备份 / 恢复是 O(database + referenced assets)，V1 没有进度条、取消或增量；在书架页 blocking worker 中运行。
- 负面：恢复是全量替换而非 merge；旧孤儿资产到下次 open 才按恢复后的引用集合清理。
- 负面：原子 no-replace 发布要求目标目录支持 hard link；不支持的文件系统会安全拒绝，不回退到有覆盖竞态的 rename。

## 风险与缓解

- 活动 WAL 普通复制：禁止，所有数据库 copy 只经 Online Backup API。
- 备份暂存数据库或恢复资产被并发 open 清理：open / recovery 与 restore 使用 exclusive lock，backup 使用 shared lock；测试重开后的清理与恢复结果。
- ZIP 路径穿越 / zip bomb / 重复 entry：不做通用解压；只按精确名称读取，拒绝未知 / 重复 / overlapping entry，并限制数量、manifest 与总解压长度。
- 恢复 DB 引用缺失 / 损坏资产：数据库与全部资产先流式哈希，数据库引用集合必须与 manifest / archive 完全相等，正式 DB copy 前发布全部资产。
- SQLite busy：bounded retry 后返回稳定恢复错误；未完成 destination transaction 由 SQLite 回滚，当前事实保留。
- 未加密敏感数据：UI 与文档明确制品边界；只有真实加密需求、密钥生命周期和恢复 UX 被批准时再设计。

## 实施与检查位置

- backend：`backend/atha-backend/src/messages/backup.rs`、`store.rs`、`model.rs`；
- Tauri / UI：`reader/app/src-tauri/src/message_maintenance.rs`、`lib.rs`、`permissions/reader.toml`、`reader/app/src/library.ts`、`components/LibraryView.svelte`；
- 失败注入：`backend/atha-backend/tests/message_reading.rs`；
- 事实：`docs/codebase/DATABASE.md`、`docs/architecture/MESSAGE-READING.md`、`docs/architecture/OVERVIEW.md`；
- 正式检查：`scripts/check-message-reading.ps1` 与 required `docs` gate。

## 回滚与替代

没有 schema 或既有事实迁移，可回退方法、Adapter、UI 与 rusqlite feature。若未来要求离线 bit-for-bit 目录快照、增量、加密或跨版本 merge，以新的质量场景和 ADR 扩展，不改变本制品含义。

## 复查触发器

- MessageStore schema 升级，需要恢复旧 schema 备份；
- 真实备份接近 V1 entry / 字节上限，或耗时需要进度 / 取消；
- 引入正常资产物理删除、网络文件系统或绕过维护锁的新生命周期；
- 加密、签名、云端、自动计划、合并恢复或全应用备份进入批准范围。

## 取代关系

本 ADR 补充 ADR-0005 的资产恢复协议，不取代既有 ADR，当前也未被其他 ADR 取代。

## 相关文档

- 当前 change：`docs/changes/message-store-backup-restore.md`
- 资产恢复：`docs/decisions/ADR-0005-message-snapshot-asset-recovery.md`
- 消息架构：`docs/architecture/MESSAGE-READING.md`
- 数据库事实：`docs/codebase/DATABASE.md`
- SQLite Online Backup API：<https://sqlite.org/backup.html>
- SQLite Backup C API：<https://sqlite.org/c3ref/backup_finish.html>
- Rust 文件锁：<https://doc.rust-lang.org/std/fs/struct.File.html#method.lock_shared>
- Rust hard link：<https://doc.rust-lang.org/std/fs/fn.hard_link.html>

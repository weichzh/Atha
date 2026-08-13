# 消息数据库

## 状态与位置

正式消息数据库由 `backend::messages::MessageStore` 拥有，使用锁定的 `rusqlite 0.40.1` 与 bundled SQLite。Windows 产品入口把数据放在 `%LOCALAPPDATA%\Atha\Messages\Messages.sqlite3`，快照资源按 SHA-256 内容寻址保存在相邻的 `Assets/`；界面和导出均不暴露该路径。

数据库当前为 schema v2。`MessageStore::open` 先以 no-follow 元数据确认 `Assets/` 是实际目录并拒绝 Windows reparse point、symlink 或 junction，再独占 `Assets/.atha-maintenance.lock`，按 `PRAGMA user_version` 在 `IMMEDIATE` 事务中顺序执行只向前迁移并拒绝未来版本；每个连接启用外键和 WAL。迁移后，`open` 在同一维护锁与 SQLite writer lock 下清理 Atha 临时资产和数据库未引用的规范哈希文件。P0 的 `p0/sqlite/` 只保留历史对照，不再是正式 schema 来源。

## 当前表

| 表 | 责任 |
| --- | --- |
| `work`、`edition` | 作品与内容哈希 Edition；同一内容重导复用身份 |
| `conversation` | Edition 内由根 Message 开始的阅读对话及最近活动时间 |
| `message` | 稳定身份、顺序、父回复、当前修订、当前原文引用和删除墓碑 |
| `message_revision` | schema 1 的不可变 source-only 或 text 修订与纯文本投影 |
| `source_anchor` | 不可变原始 Locator、可唯一重锚的当前 Locator、原文、上下文和哈希 |
| `source_snapshot` | 创建或重选时的 HTML、reader/book/user CSS 与呈现参数 |
| `snapshot_resource` | 快照资源的原始相对路径、媒体类型、长度、哈希和资产名 |
| `message_reference` | Message 之间可正反向查询的有向引用 |
| `message_search` | 仅当前、未删除修订的 FTS5 trigram 投影 |
| `legacy_import_state`、`legacy_annotation_import` | localStorage 标注原子、幂等迁移的完成凭据与旧 ID 映射 |
| `outbox_event` | 与每次事实写入同事务保存的本地待处理事件 |

## 已实现不变量

- 根 Message 必须拥有同 Edition 的 `SourceAnchor` 与 `SourceSnapshot`；回复不拥有原文快照。
- 添加笔记和编辑只追加 `MessageRevision`，调用方必须提交期望修订 ID；旧版本冲突不会覆盖新版本。
- 自动唯一重锚只更新当前 Locator；主动重选创建新的 Anchor 与 Snapshot，并保留旧捕获。
- 删除只写 Message 墓碑，不改写或追加正文修订；既有修订、关系、快照和资源仍可查询或导出。
- 引用目标必须存在、未删除且属于同一 Edition；拒绝未知、跨 Edition、父消息和自身引用。
- 快照再次校验 Locator 版本、原文哈希、HTML 文本、活动元素/属性、CSS 子资源、Shadow DOM 穿透选择器、presentation schema/长度、资源路径、媒体类型、长度和 SHA-256；未绑定资源和多余资源均拒绝。系统主题在捕获时冻结为实际明暗主题。
- 每次写入与 Outbox 事件同事务提交；两个资源写入口都先取得 `IMMEDIATE` transaction，再以独占临时文件完整写入、`sync_all` 并 rename 到最终 SHA-256。数据库失败最多留下完整孤儿，重开会清理；截断临时 / 孤儿不会成为可读取事实。
- open 清理只删除 Atha 临时文件和未引用的 64 位小写哈希文件 / symlink，保留已引用资产、未知文件和目录；所有读取、导出、复用与完整性检查共用普通文件、长度、名称和 SHA-256 校验，已引用 symlink 或损坏资产不会被清理或覆盖，读取 / 导出返回 `corrupt-message-data`，`health.integrity` 为 false。
- open / recovery 持 exclusive maintenance lock，完整备份持 shared lock，完整恢复持 exclusive lock；并发备份可以共存，但启动清理和恢复不会删除另一维护操作的暂存文件或提前发布资产。普通消息写入仍只由 SQLite transaction 协调。
- FTS 只返回当前未删除修订，可按 Edition 与 section 过滤；对话最近有回复、编辑、重选或删除时，根消息投影随之上浮。
- 书架移除不触碰消息数据库或快照资产；同内容重新导入后继续使用相同 Edition。

## 公开 interface

`MessageStore` 直接提供根消息、对话、搜索、关系、修订、历史捕获与资源查询，以及创建根消息、回复、修订、删除、重选、重锚、旧标注导入、自包含交换导出和完整备份 / 恢复。它是唯一 SQLite 实现，不存在 repository trait 或 UI 数据副本。

`reading_memory_search` 直接复用 `message_search`，连接当前 `message_revision`、Conversation、Edition 与未删除根 Message 的当前 Anchor，返回跨 Edition 的只读 DTO。三字符以上使用精确短语 FTS，短查询使用 escaped `LIKE`；结果上限为 200，搜索排除命中 Message 和根 Message 的墓碑，不新增 schema 或索引。资料库路由另有只读的 SourceCapture / SnapshotResource command，仍复用同一资源验证边界。

schema 1 `.atha-backup` 是严格单文件 ZIP，只允许 `manifest.json`、`Messages.sqlite3` 与 `assets/<sha256>`。备份通过 SQLite Online Backup API 取得 WAL 活动库的一致快照，写同目录临时 ZIP，`sync_all` 并重开自检后以 hard link 原子且不覆盖地发布。恢复在正式写入前检查唯一 entry、重叠、manifest、数据库 / 资产哈希与长度、当前 schema 精确签名、`integrity_check`、外键、领域关系、Edition / 修订 / Locator / Snapshot 内容、Outbox 事件、旧迁移凭据、FTS NULL-safe 精确投影和数据库引用的资产集合；资产原子发布后再以 Online Backup API 写入活动库，未完成 copy 保留原数据库事实，完成后清理新数据库未引用的旧资产。

V1 上限是 16 MiB manifest、单资产 16 MiB、65,536 个资产及 8 GiB 总展开字节；只有真实数据接近门槛时才配置化。最终发布要求目标目录支持 hard link，不支持时返回 `message-backup`，不以有覆盖竞态的 rename 降级。稳定错误区分 `message-backup`、`invalid-message-backup` 与 `message-restore`。制品不加密、不认证，也不包含 EPUB、书架或阅读状态。

schema 1 `.atha-data` 是应用级外层制品，其中 `Messages.atha-backup` 必须通过上述同一精确校验。`backend::local_data` 在当前数据根的 staging 中验证嵌套备份，提交时仍只调用 `MessageStore::restore_backup`，不复制活动 SQLite、WAL 或 Assets 目录。第一次目录切换前持久写入 `prepared` 日志和旧 `.atha-backup`，发布开始前再写 `publishing` marker；后端全部发布后以独占 commit marker 进入 `committed`，浏览器状态确认后才丢弃消息 rollback。启动遇到 `prepared` 或 `publishing` 恢复旧消息，遇到 `committed` 则等待 WebView 完成或显式回滚。

Tauri 阅读消息 command 只暴露受限 DTO，并校验调用窗口仍位于阅读路由；兼容 `message_maintenance` 与应用级 `local_data_maintenance` 都只接受主窗口资料库根路由，并在 blocking worker 调用 backend。前端 `message-store.mjs` 把根 Message 投影为既有标注 / 笔记 interface；`conversations.mjs` 使用同一事实显示回复、引用关系、修订、历史快照、跳回和导出；`library.ts` 只拥有生产 localStorage allowlist 的捕获、语义校验和事务替换，不解释 ZIP 或 SQLite。

## 验证入口

```powershell
pwsh -NoProfile -File .\scripts\check-message-reading.ps1
```

该检查覆盖正式迁移、重复打开、未来版本拒绝、外键、FTS5、数据库 / 已引用资产完整性、`Assets` junction 拒绝、symlink 读取 / 导出拒绝、事务回滚后的截断孤儿和临时文件恢复、并发修订、墓碑、重锚、资源、旧标注迁移、关系、书内与跨书搜索、自包含导出、WAL 完整备份、伪造 schema / 活动内容 / FTS `NULL` / 未知 Outbox / 迁移计数不一致拒绝、损坏 / busy 恢复回滚与历史呈现参数解析，并编译 Tauri 前端、维护路由和 command permission seam。跨书阅读记忆的当前组合入口是 `bash scripts/check-memory-center.sh`；Linux 真壳仍以 `bash scripts/check-reader-linux.sh` 为正式入口。

应用级组合检查使用 `bash scripts/check-local-data.sh`；显式 `--private-fixtures fixtures/local` 只增加内容无输出的真实 MDict 往返。它覆盖 `.atha-data` 往返、损坏输入拒绝、prepared / publishing / committed 恢复、存储总计、owned root 越界拒绝、两级删除、MessageStore 回归、浏览器状态事务、Svelte build 和 Tauri command / permission seam。Linux 真壳管理界面另由 `bash scripts/check-reader-linux.sh` 验收；这些本地证据不等同于 PCT-AL10 SAF 往返。

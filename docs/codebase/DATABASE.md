# 消息数据库

## 状态与位置

正式消息数据库由 `backend::messages::MessageStore` 拥有，使用锁定的 `rusqlite 0.40.1` 与 bundled SQLite。Windows 产品入口把数据放在 `%LOCALAPPDATA%\Atha\Messages\Messages.sqlite3`，快照资源按 SHA-256 内容寻址保存在相邻的 `Assets/`；界面和导出均不暴露该路径。

数据库当前为 schema v2。`MessageStore::open` 先以 no-follow 元数据确认 `Assets/` 是实际目录并拒绝 Windows reparse point、symlink 或 junction，再按 `PRAGMA user_version` 在 `IMMEDIATE` 事务中顺序执行只向前迁移，拒绝未来版本；每个连接启用外键和 WAL。迁移后，`open` 在同一 SQLite writer lock 下清理 Atha 临时资产和数据库未引用的规范哈希文件。P0 的 `p0/sqlite/` 只保留历史对照，不再是正式 schema 来源。

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
- FTS 只返回当前未删除修订，可按 Edition 与 section 过滤；对话最近有回复、编辑、重选或删除时，根消息投影随之上浮。
- 书架移除不触碰消息数据库或快照资产；同内容重新导入后继续使用相同 Edition。

## 公开 interface

`MessageStore` 直接提供根消息、对话、搜索、关系、修订、历史捕获与资源查询，以及创建根消息、回复、修订、删除、重选、重锚、旧标注导入和自包含 ZIP 导出。它是唯一 SQLite 实现，不存在 repository trait 或 UI 数据副本。

完整备份 / 恢复仍未实现。它需要定义用户选择的备份制品、在线 SQLite 一致快照、资产集合、恢复前验证、离线替换和失败回滚；不得用活动 WAL 数据库文件的普通复制代替。

Tauri 只暴露受限 DTO command，并校验调用窗口仍位于阅读路由。前端 `message-store.mjs` 把根 Message 投影为既有标注/笔记 interface；`conversations.mjs` 使用同一事实显示回复、引用关系、修订、历史快照、跳回和导出。

## 验证入口

```powershell
pwsh -NoProfile -File .\scripts\check-message-reading.ps1
```

该检查覆盖正式迁移、重复打开、未来版本拒绝、外键、FTS5、数据库 / 已引用资产完整性、`Assets` junction 拒绝、symlink 读取 / 导出拒绝、事务回滚后的截断孤儿和临时文件恢复、并发修订、墓碑、重锚、资源、旧标注迁移、关系、搜索、自包含导出与历史呈现参数解析，并编译 Tauri 前端与 command seam。证据等级为 Windows 本地；真实书籍的完整交互闭环由最终阅读器验收单独记录。

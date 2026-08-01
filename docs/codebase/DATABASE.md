# 数据库 P0 基线

## 状态

该文档记录已实现的实验语义，不是生产迁移规范。事实源为 `p0/sqlite/schema.sql` 和相应检查脚本。

## 已接受的正式政策

`docs/decisions/ADR-0002-sqlite-and-migrations.md` 已固定 M2 的方向：精确使用 `rusqlite 0.40.1`、关闭默认 features、启用 `bundled`，并用 `PRAGMA user_version` 和 `IMMEDIATE` 事务执行仅向前的顺序迁移。

该政策尚未在正式后端中实现。P0 schema 只是 v1 语义来源，不能连同 SQLite CLI、benchmark 或故障注入指令复制为生产迁移。

## 当前表

| 表 | 当前责任 |
|---|---|
| `work` | 抽象作品身份与题名 |
| `edition` | 具体 EPUB/TXT 文件、指纹、解析器和元数据 |
| `conversation` | 某版本的阅读对话 |
| `message` | 稳定消息身份、类型、回复和当前修订 |
| `message_revision` | 版本化富文本 JSON 与纯文本投影 |
| `source_anchor` | 规范 Locator、后端 Locator、原文上下文和内容哈希 |
| `outbox_event` | 与事实同事务写入的待处理事件 |
| `message_fts` | `message_revision.plain_text` 的 FTS5 外部内容索引 |

## 已验证不变量

- ID 为 16 字节 BLOB，文件指纹和内容哈希为 32 字节 BLOB。
- `message.current_revision_id` 必须属于同一消息。
- JSON 列必须通过 SQLite `json_valid`。
- 书籍会话必须关联 `edition`。
- FTS 触发器覆盖插入、更新和删除，并通过 `integrity-check`。
- 消息事实与 Outbox 事件可在同一事务回滚。
- 外键检查与 `integrity_check` 通过。

## 尚未决定或实现

- M2 迁移代码的具体文件布局；
- ID 生成算法；
- 富文本 JSON 的完整 schema；
- FTS 是否只物理索引当前修订，或索引全部修订后在查询中连接过滤；
- 删除墓碑、附件、阅读会话和导入任务表；
- 备份、加密、checkpoint 和升级策略。

未决项目必须在对应后端规格中明确，不能把 P0 DDL 直接当成生产 schema。

## 验证入口

```powershell
pwsh -NoProfile -File .\scripts\check-p0-sqlite.ps1
```

该命令会删除并重建 `build/p0-sqlite/atha-p0.sqlite`。输出属于 Windows 本地证据。

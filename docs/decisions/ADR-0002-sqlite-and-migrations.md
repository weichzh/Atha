# ADR-0002：SQLite 与迁移政策

## 状态

accepted

## 背景

P0 使用系统 SQLite CLI 验证了 schema、FTS5 和事务 Outbox，但正式 Windows 后端需要可重复的 SQLite 版本、编译能力和迁移入口。M1 只决定政策，不向后端 crate 添加数据库依赖；具体存储代码属于 M2。

## 证据

以下证据于 2026-08-01 通过 RsProxy 包元数据、上游 crate 源码和 SQLite 官方文档核对：

- `rusqlite 0.40.1` 的 `bundled` feature 启用 `libsqlite3-sys/bundled` 和 `modern_sqlite`；其依赖元数据指向 `libsqlite3-sys 0.38.1`。
- `libsqlite3-sys 0.38.1` 随包源码包含 SQLite `3.53.2`；bundled 构建明确启用外键默认值、FTS5、JSON1 和线程安全。
- SQLite 官方将 `PRAGMA user_version` 定义为供应用自行使用的整数，SQLite 本身不解释该值。
- SQLite 官方说明 `BEGIN IMMEDIATE` 会立即启动写事务，并可能在已有写事务时返回 `SQLITE_BUSY`。

来源：

- <https://github.com/rusqlite/rusqlite/tree/v0.40.1>
- <https://docs.rs/crate/rusqlite/0.40.1/features>
- <https://www.sqlite.org/pragma.html#pragma_user_version>
- <https://www.sqlite.org/lang_transaction.html>

## 决策

1. M2 使用精确依赖 `rusqlite = { version = "=0.40.1", default-features = false, features = ["bundled"] }`；不直接依赖 `libsqlite3-sys`。
2. 提交根 `Cargo.lock` 固定传递依赖。升级 `rusqlite` 或随包 SQLite 必须由独立变更完成，并重跑存储集成测试与完整性检查。
3. 不引入额外迁移 crate。M2 在存储 module 内提供一个内部迁移入口，数据库连接对外可用前必须调用它。
4. schema 版本使用 `PRAGMA user_version`。迁移只允许从当前版本逐级向前执行；未知的更高版本必须拒绝打开，不能猜测兼容。
5. 每次迁移在 `IMMEDIATE` 事务内执行；所有 SQL 成功后更新 `user_version` 并提交，任一步失败则回滚并返回脱敏错误。
6. P0 `schema.sql` 只作为 v1 语义来源。M2 必须移除 SQLite CLI 指令，并把纯 schema SQL 转为版本化迁移；P0 benchmark、故障注入和校验 SQL 不进入生产迁移。
7. M2 集成测试必须核对实际 SQLite 版本、FTS5 编译选项、外键、逐级迁移、重复打开、失败回滚和未来版本拒绝行为。
8. 迁移仅向前。需要回退时恢复升级前备份，不编写未经真实需求证明的 down migration。

## 影响

- Windows 不依赖机器预装 SQLite，FTS5 能力由随包构建固定。
- M2 会承担一次 C 源码编译成本，但运行时版本更可预测。
- 精确版本不会自动获得 SQLite 修复，依赖更新必须成为显式维护动作。
- 简单的顺序迁移无需新依赖；当迁移出现分支、跨版本跳转或重复样板时再评估迁移库。

## 备选方案

- 使用系统 SQLite：否决，Windows 机器的版本和编译选项不可控。
- 使用 `bundled-full`：否决，当前只需要 bundled SQLite 与 FTS5，不启用无用的高级 rusqlite API。
- 立即引入迁移 crate：否决，当前只有线性 v1，标准事务和 `user_version` 足够。
- 编写 down migration：否决，增加破坏性路径且没有当前需求。

## 相关文档

- 规格：`docs/specs/SPEC-0001-windows-backend-foundation.md`
- 计划：`docs/plans/PLAN-0001-windows-backend-foundation.md`
- 数据库基线：`docs/codebase/DATABASE.md`
- P0 schema：`p0/sqlite/schema.sql`

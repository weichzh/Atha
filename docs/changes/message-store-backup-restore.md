# 消息存储完整备份与恢复

## Status

accepted

## Problem

消息事实同时位于 WAL 模式的 `Messages.sqlite3` 与外部内容寻址 `Assets/`。现有消息导出只覆盖一部书或一个会话的交换数据，不包含完整数据库状态，也没有导入路径；普通复制活动数据库则可能遗漏 WAL 中的已提交事实。用户目前无法把全部标注、笔记、对话、修订、关系、旧迁移凭据和 Outbox 连同快照资源作为一个一致制品恢复。

完整恢复会替换既有消息事实，是独立的用户 Interface 和失败模型。它不能并入按书导出，也不能通过替换正在使用的数据库文件实现。

## Scope

- 新增 schema 1 的单文件 `.atha-backup` ZIP 制品，包含 SQLite Online Backup API 生成的 `Messages.sqlite3` 一致快照、manifest 与快照引用的全部内容寻址资产；
- 备份先写同目录临时文件，完整写入、`sync_all`、重新检查后以同文件系统 hard link 原子且不覆盖地发布，再删除临时名；
- 恢复先完整验证 ZIP 结构、边界、manifest、数据库哈希 / 长度、当前 database schema 精确签名、SQLite / 外键完整性、消息关系 / 内容 / FTS 投影及资产集合 / 哈希，再触碰正式数据；
- 资产先在 SQLite writer lock 下原子发布，随后以 Online Backup API 把已验证数据库恢复到活动数据库；未完成的数据库恢复由 SQLite 回滚，原消息事实保持不变；
- `Assets/.atha-maintenance.lock` 使用 Rust 标准库文件锁：open / recovery 持 exclusive lock，备份持 shared lock，恢复持 exclusive lock，避免启动清理删除备份暂存数据库或恢复资产；
- 在书架页提供“备份消息”和“恢复消息”入口；恢复前明确确认会替换全部消息事实，Tauri Adapter 只接受主窗口书架路由并在 blocking worker 执行文件 I/O；
- 更新稳定错误、Tauri 权限、架构 / 数据库 / 代码地图和专项检查。

## Non-Goals

- 不备份 EPUB、书架目录、阅读进度、WebView2 数据、偏好或导入缓存；本轮制品只拥有 `MessageStore` 的完整事实；
- 不实现按事实合并、选择性恢复、跨设备同步、云端、计划任务、历史版本列表或后台自动备份；
- 不实现加密、密码、签名或真实性认证；制品仍按不可信本地输入执行完整结构与内容验证；
- 不改变消息 schema，不增加 crate、storage trait、repository interface、通用 archive framework 或第二套数据库实现；只为既有 `rusqlite` 启用官方 `backup` feature；
- 不声称抵抗磁盘、文件系统或硬件损坏，也不执行真实进程强杀；本轮覆盖正常文件系统上的进程中止残留、无效制品和 SQLite 已返回失败。

## Architecture Impact

present

- Design purpose：关闭 `ASR-DATA-02` 的全库可恢复性缺口，并保持备份、恢复与按书交换导出的语义分离；
- Module / Interface：协议留在 deep Module `backend::messages::MessageStore`，新增具体 `create_backup` / `restore_backup` Interface；Tauri 只是文件选择与书架路由 Adapter，不解释数据库或 ZIP；
- Quality scenarios：见下表；正式测试从公开 `MessageStore` Interface 注入 WAL、损坏制品与 SQLite busy 故障；
- Candidate：采用 Online Backup API + 单文件 ZIP + 具体维护锁；拒绝活动数据库普通复制、复用交换导出和离线目录替换，理由见 ADR-0006；
- Evidence / review：Rust interface 集成测试、Tauri permission / route 静态检查、fmt、clippy、消息专项、docs gate 与 Standards / Spec review。

| 字段 | `ASR-DATA-02A` 备份 |
| --- | --- |
| 刺激源 | 用户或同时写入消息的另一个 Atha 连接 |
| 刺激 | 用户选择新目标并创建完整消息备份 |
| 环境 | WAL 数据库可读写，快照资产不可变且正常路径不删除 |
| 制品 | 活动 `Messages.sqlite3`、已提交 `snapshot_resource` 与 `Assets/` |
| 响应 | Online Backup 生成单点一致数据库快照；只打包该快照引用且通过校验的资产；最终路径只在完整自检后出现 |
| 响应度量 | 制品恰有 manifest、一个数据库和全部且仅有引用资产；同一制品可在新数据根恢复；目标已存在时零覆盖 |

| 字段 | `ASR-DATA-02B` 拒绝 / 失败 |
| --- | --- |
| 刺激源 | 损坏 / 伪造备份或占用活动数据库的另一个连接 |
| 刺激 | 恢复验证失败，或 Online Backup 未完成并返回 busy / I/O 错误 |
| 环境 | 当前消息库可正常读取，可有 WAL 与已发布恢复资产 |
| 制品 | 备份 ZIP、暂存数据库、活动数据库与 `Assets/` |
| 响应 | 无效制品在正式写入前拒绝；未完成数据库 copy 由 SQLite 回滚；暂存文件在返回错误时删除，进程中止残留由下次 open 清理 |
| 响应度量 | 原 Conversation / Message 数量和内容不变，`health.integrity` 仍为 true；没有部分数据库事实 |

| 字段 | `ASR-DATA-02C` 成功恢复 |
| --- | --- |
| 刺激源 | 用户选择当前版本生成且已验证的备份 |
| 刺激 | 用户确认替换全部消息事实 |
| 环境 | 主窗口处于书架路由，维护锁可独占取得 |
| 制品 | 已验证数据库快照、资产集合和活动 MessageStore |
| 响应 | 先保证全部引用资产可读，再原子完成数据库恢复；下次 open 按恢复后的引用集合清理旧孤儿资产 |
| 响应度量 | 恢复后 `health.integrity` 为 true，备份时存在的事实 / 资源可读，备份后新增事实不存在；重开结果相同 |

## Acceptance Criteria

- [x] 备份使用 SQLite Online Backup API，不普通复制活动 DB / WAL；备份时并发已提交事实形成一致单点快照；
- [x] `.atha-backup` 只在临时 ZIP 完成、`sync_all` 和重新校验后以 hard link 原子且不覆盖地发布，已存在目标内容保持不变；
- [x] manifest、entry 数 / 总解压长度、重复 / 未知路径、数据库精确 schema / 完整性 / 外键、领域关系 / 内容 / FTS 投影、数据库与资产哈希 / 长度及引用集合均在恢复前验证；
- [x] open / recovery exclusive、backup shared、restore exclusive maintenance lock 使用标准库实现，不引入通用锁层；
- [x] 成功恢复替换全部 MessageStore 事实并保持资源可读；重开后旧孤儿资产被清理；
- [x] 损坏备份和 SQLite busy 失败注入证明原消息事实不变，暂存 / 孤儿可在重开恢复；
- [x] 书架页只通过受限 Tauri command 选择文件并在 blocking worker 调用 backend；取消、确认、忙碌和错误状态可见；
- [x] 不改变 schema，不备份书籍 / 阅读状态，不增加 crate、trait 或通用 archive abstraction；
- [ ] 中文 Markdown、目标检查、required gate、diff 检查和独立 Standards / Spec review 通过。

## Files And Steps

1. 用现有 MessageStore interface 测试固定 WAL 一致备份、成功替换、损坏制品和 SQLite busy 回滚。
2. 在新 `messages::backup` 模块实现 schema 1 archive、Online Backup copy、边界验证、资产发布与维护锁；复用既有 ZIP、哈希和资产原子发布。
3. 在独立 `message_maintenance` Adapter 增加资料库根路由、文件选择 command、权限和最小状态 UI，不把协议复制到前端。
4. 更新 `OVERVIEW`、`MESSAGE-READING`、`DATABASE`、`MAP` 与 ADR-0006。
5. 运行专项检查、候选 required gate 和双轴 review，记录证据并关闭任务。

## Checks

- `cargo test -p atha-backend --test message_reading`；
- `cargo fmt --all --check`；
- `cargo clippy -p atha-backend --all-targets -- -D warnings`；
- `pwsh -NoProfile -File scripts/check-message-reading.ps1`；
- `pnpm --dir reader/app check`、`pnpm --dir reader/app build` 与 `cargo test -p atha-reader-app`；
- `autocorrect --fix` / `autocorrect --lint` 仅作用于本次中文 Markdown；
- `python scripts/doc_guard.py`、`python scripts/doc_length_check.py`、`git diff --check`；
- `project_workflow.py station message-store-backup-restore --activity verification --gate docs`。

## Rollback

回退 backend / Tauri / UI、`rusqlite` feature 与文档即可；没有 schema 或既有事实迁移。已生成的 `.atha-backup` 不会被旧版本识别，恢复前正式数据不变；`Assets/.atha-maintenance.lock` 是零字节协调文件，可由回退版本作为未知文件保留。

## Approval

用户于 2026-08-07 要求继续按既定架构计划执行；前一 P0 change 明确把完整消息存储备份 / 恢复列为下一项独立用户 Interface。本 change 只实现该已批准范围，不扩展到书籍、云端或加密。

## Result

已形成实施候选：`MessageStore` 增加 schema 1 `.atha-backup` 创建 / 恢复，数据库双向 copy 只使用 SQLite Online Backup API；严格 ZIP / manifest / database schema / 领域内容 / FTS / asset 验证位于 backend，维护锁与既有资产原子发布共同保护暂存生命周期，最终备份用 hard link 原子 no-replace 发布。书架页通过独立 `message_maintenance` Adapter 提供备份 / 恢复、覆盖确认和忙碌 / 取消 / 错误状态；交换导出语义未改变。

## Review

- Blocking：待独立 Standards / Spec review。
- Non-blocking：待独立 Standards / Spec review。
- Out-of-scope：待独立 Standards / Spec review。

## Evidence And Residual Risks

- 审计静态证据：现有按书 / 会话导出没有恢复入口，也不包含完整 DB 状态；正式数据库使用 WAL，资产位于事务之外；
- 官方语义证据：SQLite Online Backup API 对活动源生成一致 snapshot，在 copy 未完成时回滚 destination write transaction；Rust 1.97 标准库 `File` 提供 Windows / Unix 文件 shared / exclusive lock；
- Windows 本地证据：后端 `cargo fmt`、warnings-as-errors clippy 与消息 interface 集成测试 19/19 通过；测试覆盖 WAL 快照、目标零覆盖、完整恢复、损坏资产修复、重开孤儿清理、损坏制品、伪造 schema、活动快照内容拒绝和 SQLite busy 回滚；
- Windows 本地证据：正式 `scripts/check-message-reading.ps1` 通过，包含 command / permission 精确映射、Markdown 测试、Svelte check、production build、Tauri app / host 测试；
- 证据边界：尚未通过真实原生 dialog 执行用户数据备份 / 恢复，也未执行进程强杀、磁盘故障或生产等价验收；不会把 backend 测试和前端 build 称为真实 Tauri / WebView2 恢复验收；
- 并发边界：维护锁只协调遵循 Atha 协议的 open / backup / restore；已有正常写连接继续由 SQLite destination transaction 排他，外部程序绕过协议不受保证；
- 制品边界：本地备份未加密、未认证且只含消息事实；用户应按敏感数据保存，书籍与阅读状态另行恢复；
- 容量边界：V1 对 entry 数与总解压字节设固定上限；真实数据接近门槛时以测量驱动流式分卷或可配置上限，不预建框架。
- 文件系统边界：最终 no-replace 发布依赖目标目录支持同文件系统 hard link；不支持时安全返回 `message-backup`，不降级为可能覆盖并发目标的 rename。

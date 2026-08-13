# ADR-0010：应用级本地数据生命周期

## 状态

accepted

## 日期

2026-08-13

## 背景

Atha 的耐久数据分别由 `LocalLibrary`、`LocalDictionaries`、MessageStore 和 WebView localStorage 拥有，`ImportedBooks`、Picker cache 与日志则是可重建或诊断数据。现有 `.atha-backup` 只恢复 MessageStore；复制整个应用目录会遗漏活动 WAL、一并复制缓存和日志，并绕过所有内容与路径校验。

用户现在需要一个可迁移资料库、可解释的存储占用，以及“移出书架”和“删除本地数据”两种不同操作。该决定不改变各 Module 的事实所有权，只决定应用级协调、制品和失败恢复。

## 驱动因素与场景

- `LD-DATA-01`：有效资料库完整往返后，耐久书源、书架、词典、消息和阅读状态相等，可重建缓存不进入制品；
- `LD-DATA-02`：恢复写入或进程中止只能留下恢复前或恢复后的完整集合；
- `LD-SEC-01`：本地备份仍是不可信输入，未知、重叠、越界、超限、哈希或语义伪造在正式写入前拒绝；
- `LD-PRIV-01`：日志不包含书名、路径、正文、查询、笔记或内容哈希；
- 约束：复用现有 ZIP、SHA-256、SQLite Online Backup API、Tauri dialog / SAF bridge 和 localStorage schema，不新增依赖、同步模型或第二数据根。

## 决定

1. 新增 concrete backend `local_data` Module，只协调应用级备份、恢复日志、存储统计和书籍物理数据删除；书架、词典、消息和阅读状态继续由原 Module 校验和拥有。
2. schema 1 `.atha-data` 是严格 ZIP。`manifest.json` 记录排序后的已知文件路径、长度和 SHA-256；内容只允许规范化 `Library/` 记录、`SourceBooks/`、`Dictionaries/`、嵌套 `Messages.atha-backup` 与 `BrowserState.json`。总解压上限 8 GiB、文件数 100,000、浏览器状态 16 MiB。
3. `ImportedBooks`、Picker cache、日志和未知文件不进入制品。书架记录在备份时合并当前已验证 metadata，恢复后即使缓存为空仍保留书名和作者；封面与正文在首次打开时由耐久源重建。
4. 恢复先在当前数据根内解压到独占 staging，复用书架源身份、词典 reader、MessageStore 完整备份和浏览器 key allowlist 做语义验证。任何正式 rename 前创建旧 MessageStore 备份，并原子写入 `prepared` 恢复日志与浏览器前后状态。
5. `Library`、`SourceBooks`、`Dictionaries` 和 `ImportedBooks` 通过同文件系统目录 rename 切换，MessageStore 继续只经 ADR-0006 的 Online Backup API 替换。日志在全部 backend 发布成功后变为 `committed`；浏览器状态确认后才删除 rollback。启动看到 `prepared` 必须恢复旧目录与旧 MessageStore，看到 `committed` 则阻止普通书架操作，直到 WebView 幂等应用新状态并确认或显式回滚。
6. localStorage 只备份生产 allowlist key。替换先保留旧快照，任一同步写失败立即回写旧集合；书籍物理删除只清理本书偏好、书签、进度、遗留标注和统计中的本书项，不改全局偏好或其他书状态。
7. “移出书架”仍只删除书架记录，保留源、缓存、阅读状态和 Message；“删除本地数据”删除记录、同身份源、缓存与本书浏览器状态，但保留 Message、SourceAnchor、SourceSnapshot 和资产。重新导入同一内容可重新建立可跳转书籍身份。
8. Tauri Adapter 负责主书架 origin、dialog、SAF cache、blocking worker 和固定字段日志；Svelte 负责确认文案、状态事务与重载，不解释 ZIP、SQLite 或文件布局。

## 候选与权衡

- **复制整个应用目录**：代码短，但活动 WAL、WebView profile、缓存、日志、平台路径和未知文件无法形成可信一致制品，否决。
- **每个 Module 独立备份 / 恢复**：复用现有接口，但把顺序、一致性和遗漏风险交给用户，不能满足应用级恢复，否决。
- **把全部状态迁到新 SQLite**：可以获得单事务，但会重写成熟 Module、复制事实并引入迁移风险，当前没有收益证据，否决。
- **严格协调 ZIP + staging / rollback + 既有 MessageStore 备份**：采用。它增加一个真实应用生命周期边界和短暂双份空间成本，但不改变领域模型或引入新依赖。

## 后果

- 正面：用户得到一个不含日志与缓存的完整资料库制品；恢复输入在触碰正式事实前完整校验。
- 正面：中止恢复由耐久日志收敛，SQLite 仍服从既有事务与维护锁，不直接复制 WAL 文件。
- 正面：两级删除不牺牲长期阅读记忆；后续跨书中心可以在书籍缺失时显示 Snapshot，而不伪造跳转。
- 负面：完整备份和恢复是 O(耐久数据)，恢复期间需要 staging、旧消息备份和 rollback，峰值空间可接近两份资料库。
- 负面：schema 1 只接受当前数据语义，不做 merge、旧 schema 猜测、加密、签名、进度或取消。
- 负面：WebView localStorage 是独立持久化边界，需要提交确认；在确认前启动必须完成或回滚 pending restore，不能直接开放书架。

## 风险与缓解

- ZIP 路径穿越、bomb、重复或 overlapping entry：精确 allowlist、唯一 entry、数量 / 单项 / 总量预算和流式哈希；不通用解压未知路径。
- 书架记录与源伪造：恢复 staging 上复用内容身份与记录校验；缺少耐久源的书架项拒绝。
- 恢复中止：第一次 rename 前耐久发布日志和旧消息备份；`prepared` 启动回滚，`committed` 等待浏览器确认。
- 浏览器存储 quota / 禁用：同步替换失败回写旧快照并调用 backend rollback；pending 状态不允许继续普通书架操作。
- 删除误解：界面分别说明保留项；破坏性操作二次确认，Message 与 Snapshot 永不由书籍文件删除路径物理清除。
- 隐私：manifest 只含路径类别、长度与哈希，产品日志不记录 archive 路径、书名、ID、浏览器 key/value 或内容哈希。

## 实施与检查位置

- backend：`backend/atha-backend/src/local_data.rs`、`reader/library.rs`、`reader/dictionary.rs`、`messages/backup.rs`；
- Tauri：`reader/app/src-tauri/src/local_data_maintenance.rs`、`lib.rs`、`platform_file.rs`；
- Web：`reader/app/src/library.ts`、`components/LibraryView.svelte`、`library.css`；
- tests / gate：`backend/atha-backend/tests/local_data.rs`、`reader/app/tests/library.test.ts`、`scripts/check-local-data.sh`；
- facts：`docs/architecture/READER-CORE.md`、`docs/architecture/MESSAGE-READING.md`、`docs/codebase/MAP.md`、`docs/codebase/DATABASE.md`。

## 回滚与兼容

没有数据库 schema 迁移。回退代码后，既有书架、词典、消息和 localStorage 仍保持原格式；已生成 `.atha-data` 只失去产品恢复入口，不会被旧版本自动解释。原 `.atha-backup` 协议与 MessageStore 方法不变。

## 复查触发器

- MessageStore、书架、词典或浏览器状态 schema 升级，需要定义旧 `.atha-data` 迁移；
- 备份接近 8 GiB / 100,000 文件，或真实耗时要求进度、取消、增量；
- 出现第二进程写入、网络文件系统、正常消息物理删除或跨设备 merge；
- 加密、签名、云同步、自动计划或生产发布进入批准范围。

## 取代关系

本 ADR 组合并补充 ADR-0006，不改变或取代 MessageStore `.atha-backup` 的内部灾难恢复语义。

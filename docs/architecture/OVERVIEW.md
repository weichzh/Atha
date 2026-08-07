# Atha 架构总览

## 入口

Atha 是 Windows 优先、本地优先的高保真个人阅读系统。产品目标和不可违反的体验原则见 `docs/product/OVERVIEW.md`；通用架构设计与评估规则见 `docs/architecture/DESIGN-GUIDE.md`。本文件拥有系统级结构、依赖方向、质量优先级和长期迁移边界；阅读与消息的详细语义分别由专门文档维护。

当前目标不是重写或分布式拆分，而是在单产品部署单元的模块化单体中让事实模块保持深、平台 adapter 保持窄、composition root 只负责装配。该部署单元包含原生 Tauri host 与 WebView2 管理的多进程树，不是单进程系统。Windows UI、Tauri、Svelte 和 WebView2 不得反向成为领域或数据类型。

## 架构驱动与质量场景

| ID | 优先级 | 质量属性 | 场景与成功标准 |
| --- | --- | --- | --- |
| ASR-SEC-01 | P0 | 内容安全 | 不可信 EPUB 提交脚本、事件属性、外部资源、越界路径或未声明资源时，在进入书籍 Shadow DOM 或发出外部网络 / 宿主 command 请求前明确拒绝；正式安全探针保持零外联。 |
| ASR-DATA-01 | P0 | 数据完整性 | 消息、修订、引用、快照与 Outbox 任一步失败或进程中止时，不留下部分消息事实；重开后外键、schema、已引用资产与完整性检查通过。快照资产具备进程中止安全发布和孤儿清理；完整 MessageStore 可由一致、可验证制品备份，并在失败时保留原事实。 |
| ASR-REF-01 | P0 | 引用保真 | 同一内容版本重开、重排或重新导入后，`SourceAnchor` 能回到原文，`SourceSnapshot` 保留当时呈现；无法唯一恢复时显式回落，不静默改写历史。 |
| ASR-PERF-01 | P1 | 性能 | 正式困难样本的冷启动、首个稳定页、热打开、翻页与字号重排 P95 继续低于阅读内核规定的固定门槛；没有测量证据时不增加缓存、worker 或虚拟化。 |
| ASR-MOD-01 | P1 | 可修改性 | 新用例修改拥有规则的 deep module 及一个真实边界 adapter；composition root 只增加装配或注册，不复制验证、数据或阅读算法。 |
| ASR-PRIV-01 | P1 | 隐私与可诊断性 | 诊断只记录固定事件、阶段和数值，不记录书名、路径、原文、笔记、查询或提示词；失败保留稳定错误代码和证据等级。 |

## 架构选择

| 候选 | 收益 | 代价与风险 | 结论 |
| --- | --- | --- | --- |
| 保持现状，只补文档 | 零代码迁移，现有行为最稳定 | Tauri composition root 继续同时拥有平台启动、协议、书架与消息 IPC；command 与 capability 漂移仍靠脆弱文本检查发现 | 不选 |
| 模块化单体 + 显式 adapter | 保留单产品部署单元、SQLite、WebView2 和现有 deep module；只在真实信任 / 平台边界形成窄 adapter，可逐片迁移 | 需要少量源码移动和边界回归 | 采用 |
| 全面 Ports and Adapters、repository trait 或拆服务 | 理论上可替换平台、存储或部署 | 当前没有第二实现、独立部署或伸缩场景，会增加 DTO、trait、网络和一致性成本 | 拒绝；出现真实第二实现后重评 |

采用方案见 `docs/decisions/ADR-0004-modular-monolith-adapters.md`。微服务、事件总线、命令总线、插件注册表、通用多格式工厂和单实现 repository 均不属于当前目标架构。

## Module 视图

| Module | 责任 | 公开 Interface | 依赖限制 |
| --- | --- | --- | --- |
| Svelte 产品壳 `reader/app/src/` | 书架、工具栏、面板、dialog 与受信任用户操作 | `library.ts`、`messages.ts` 的受限 Tauri client | 不拥有书籍 DOM、分页热状态、SQL 或消息事实 |
| 浏览器阅读内核 `reader/web/` | 内容校验、会话、Locator、分页、导航、偏好、状态、搜索、标注 / 消息投影与诊断 | 各 `create*` 返回的冻结小对象；`app.mjs` 只组合 | 不访问文件系统、SQLite 或任意宿主 API；书内文档没有 command interface |
| Tauri 平台 adapter `reader/app/src-tauri/` | 平台启动、窗口、受控协议、dialog、capability、固定字段本地日志与 IPC DTO 映射；阅读消息与全库消息维护分别由 `message_commands`、`message_maintenance` 集中，library / telemetry / protocol adapter 暂仍与 composition root 同文件 | 已注册的 Tauri command 与 `atha-book` / `atha-cover` | 不实现消息、EPUB、Locator 或分页不变量；新增规则不再进入 `lib.rs` |
| 阅读应用模块 `backend::reader` | EPUB 导入、受控书根、本地书架和遥测输入校验 | `import_epub`、`LocalLibrary`、`BookRoot`、`parse_reader_event` | 不依赖 Tauri、Svelte 或 WebView 对象 |
| 消息事实模块 `backend::messages` | schema、迁移、事务、查询、修订、引用、快照资产恢复、Outbox、交换导出与全库备份 / 恢复 | concrete `MessageStore` 及领域 DTO / 稳定错误 | 唯一 SQLite / 快照资产所有者；调用方不得复制消息事实或拼接 SQL |
| Windows 验证 host `reader/atha-reader-host` | 两个 host 共用的启动、窗口尺寸和诊断；保留直接 Wry/Tao 回归 adapter | `launch`、`diagnostics` 与旧 `run` | 不接受新产品能力；Tauri 达到等价覆盖后单独评估删除 |
| 数据 `Messages.sqlite3`、`Assets/`、`Library/`、`ImportedBooks/` | 消息事实、内容寻址快照资源、书目记录与导入缓存 | 只经所属 backend Module 访问 | UI、reader kernel 与平台 adapter 不直接读写 |

依赖方向为产品壳 / reader kernel → Tauri adapter → backend deep module → SQLite / 本地资产。`atha-reader-host` 是迁移期验证 adapter，不是领域层；不得因复用其 Windows 启动代码而把 Wry/Tao 类型带入 backend。

目标状态下 `lib.rs` 只做 composition；当前 as-built 已拆出阅读消息与全库消息维护两条信任 Seam。其余 in-file adapter 没有独立变化压力时不机械拆分，后续只在真实用例触碰相应边界时迁移。

## 运行时、数据与信任边界

当前 as-built 运行拓扑由一个原生 Tauri host 进程和 WebView2 管理的浏览器、renderer 等子进程组成；R8 本地基线最多观测到 8 个进程。Svelte 壳与 reader kernel 在 WebView2 renderer 中运行，Tauri command 和自定义 protocol 跨 WebView2 / 原生 host IPC 边界；backend module、SQLite 与本地资产访问位于原生 host 一侧。直接 Wry/Tao host 是单独启动的迁移期验证程序，不与产品 host 组成分布式服务。

产品 host 使用 Tauri 官方日志插件把同一组 Rust 事件写入 stdout 与平台 AppLog。插件只接受 `atha::` target，Info 以上按 1 MiB 单文件有限轮转；adapter 只记录固定 operation / event、mode、stage、稳定 code、耗时与计数。reader failure 的 code 与 stage 都先经 backend 白名单验证；预期 protocol 4xx 不写盘，书名、路径、正文、笔记、查询、提示词和内容哈希不得进入日志。Windows benchmark Recorder 继续独立拥有正式性能制品，普通日志不替代 benchmark。

1. **导入**：可信用户选择文件 → Tauri library command → `LocalLibrary` → `reader::epub` 校验 ZIP / OPF / 路径 / 大小 → 内容哈希书根与受限书目记录。
2. **阅读**：Svelte 路由装载唯一 reader runtime → `atha-book` protocol → `BookRoot` 再校验路径、MIME 与大小 → `session` 校验 manifest → `content` 校验 XHTML / CSS / SVG 后导入 closed Shadow DOM。
3. **消息**：已验证 Range → reader `message-store` 投影 → TypeScript Message client → Tauri message adapter 校验主窗口与阅读路由 → `MessageStore` 再校验 DTO 并在 SQLite 事务中写入事实与 Outbox。
4. **查询与导出**：同一 adapter 只返回受限 DTO；快照资源按 Source 与路径读取，导出由原生保存 dialog 选择目标，UI 不接触数据库或资产路径。
5. **全库备份 / 恢复**：资料库根页 → Tauri `message_maintenance` 校验主窗口、选择文件并进入 blocking worker → `MessageStore` 经维护锁、SQLite Online Backup、严格 ZIP / 数据库 / 资产校验和原子发布完成；前端不解释制品内容。

书籍内容是敌对输入；Svelte 应用壳是受信任但仍受 capability 和 DTO 限制的调用方；Tauri command 是平台信任 seam；backend 是不变量的最终执行者。XHTML、图片、任意文件路径和原始数据库连接均不得跨 IPC。

## Interface、Seam 与 Adapter 清单

- `BookRoot::read` 是书根到字节资源的 Interface；`atha-book` 是 WebView2 protocol Adapter；路径、MIME、大小与书根是安全 Seam。
- `LocalLibrary` 是本地书架的 deep module；Tauri library commands 是文件 dialog 和 UI DTO Adapter。
- `MessageStore` 是消息事实 Interface；Tauri `message_commands` 与 `message_maintenance` 分别是阅读消息和全库维护 IPC Adapter；`messages.ts` / `library.ts` 是受信任壳 client；reader `message-store.mjs` 是标注 / 笔记投影 Adapter。它们不形成第二份事实。
- reader 各 `create*` factory 是浏览器内现有 Module Interface，不因只有一个实现再包 trait、registry 或 service locator。
- `parse_reader_event` 是不可信 telemetry 输入的 Interface；Tauri 与旧 host 都只消费通过校验的事件。Tauri 产品 adapter 把 failure code / stage 和安全数值投影到平台 AppLog；旧验证 host 继续只把 code 交给既有 Recorder，不把平台日志反向带入基线 host。

新增 interface 只有在存在第二个真实实现，或必须隔离平台、信任、事务、性能或测试边界时才成立。单纯为缩短文件、模拟未来替换或追求图形对称而增加间接层不成立。

## 开放架构风险

消息 IPC 边界、快照资产中止恢复和完整 MessageStore 备份已分别由 ADR-0004、ADR-0005 与 ADR-0006 固化，并进入正式检查。下表只保留仍开放的结构风险。

| 风险 | 当前控制与触发条件 |
| --- | --- |
| Tauri 产品入口仍依赖保留的 Wry / Tao host crate，并维护两份 runtime 交付清单 | 旧 host 不增加产品能力；当 Tauri gate 覆盖其全部独有证据时，再用独立 ADR 决定删除或重命名 |
| reader runtime 通过固定顺序拼成单 bundle，依赖不是显式 import | production build、语法检查和多入口继续防止顺序漂移；只有出现真实加载、调试或顺序故障时才改为显式模块图 |
| 内容安全和引用保真具有最高影响 | 保持多层校验与真实 WebView2 gate；不以结构整洁为由削弱边界或批量重写 |

本表不是产品 backlog。只有风险阻塞路线图中的获批场景，或重复证据达到处理阈值时，才创建架构 change；产品方向由 `docs/roadmap/ROADMAP.md` 决定。每个迁移切片必须保持行为与依赖不变、更新 as-built 地图、运行所属 Module 的最小检查，再运行 required gate。

## 相关文档

- 架构设计规范：`docs/architecture/DESIGN-GUIDE.md`
- 产品定义：`docs/product/OVERVIEW.md`
- 阅读内核：`docs/architecture/READER-CORE.md`
- 消息与共读：`docs/architecture/MESSAGE-READING.md`
- 模块化单体决策：`docs/decisions/ADR-0004-modular-monolith-adapters.md`
- 快照资产恢复决策：`docs/decisions/ADR-0005-message-snapshot-asset-recovery.md`
- 完整消息备份 / 恢复决策：`docs/decisions/ADR-0006-message-store-backup-restore.md`
- Android 开发前的本地诊断日志决策：`docs/decisions/ADR-0007-android-observability.md`
- 当前状态：`docs/ACTIVE.md`
- 代码现状：`docs/codebase/MAP.md`
- 数据库基线：`docs/codebase/DATABASE.md`

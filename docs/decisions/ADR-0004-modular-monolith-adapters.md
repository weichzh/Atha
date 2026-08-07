# ADR-0004：模块化单体与显式平台 Adapter

## 状态

accepted

## 日期

2026-08-07

## 背景

Atha 已有一个 Windows 产品部署单元、一个 Tauri / WebView2 产品入口、一个 SQLite 消息事实源和数个职责清楚的 backend / reader Module。该产品不是单进程系统：Svelte 壳与 reader kernel 位于 WebView2 renderer，Tauri adapter、backend 和数据访问位于原生 host，command 与自定义 protocol 跨 IPC 边界；历史 R8 本地基线在完整进程树中最多观测到 8 个进程。直接 Wry/Tao host 是单独运行的迁移期验证程序。2026-08-07 的 as-built 审计确认，`MessageStore`、`LocalLibrary`、`BookRoot` 和 browser reader kernel 已具备较深 Interface；主要结构风险集中在平台 composition root 和迁移期双 host，而不是缺少通用抽象。

同次审计发现，Tauri root 同时注册 17 个消息 command 并直接导入大量消息 DTO；历史真实链路曾因 capability 漏授权而失败。现有静态 gate 的 handler 正则要求尾逗号，因此会漏检注册列表最后一个 command。需要在不改产品行为、数据 schema 或运行拓扑的前提下，明确平台信任 Seam 并消除检查盲区。

## 驱动因素与范围

- `ASR-MOD-01`：新用例只修改拥有规则的 deep module 和一个真实 adapter，composition root 不复制规则。
- `ASR-SEC-01`：不可信书籍内容不得获得 command interface；受信任应用壳也必须受 capability、来源与 DTO 约束。
- `ASR-DATA-01`：消息事务与事实所有权继续只由 `MessageStore` 承担，本次不改变 schema、资产发布或恢复策略。
- 范围限于模块边界、Tauri 消息 adapter、注册 / permission 一致性检查和相应 as-built 文档；不拆服务、不增加依赖，也不删除旧 host。

## 假设

- 当前只有一个需要独立发布和伸缩的 Windows 产品单元；WebView2 子进程是平台运行拓扑，不是独立服务边界。
- 消息存储、书架与阅读内核各只有一个生产实现，没有为可替换性增加 trait 的现实需求。
- 现有 command 名、DTO、错误代码、数据库与前端调用行为必须保持兼容。

## 决策

1. Atha 保持单产品部署单元的模块化单体。WebView2 renderer 内的 Svelte 产品壳 / browser reader kernel 通过 Tauri IPC 与原生 host 内的 adapter / backend 协作；SQLite 与本地资产仍由 host 侧 deep module 管理，不把进程边界误写成进程内调用。
2. `backend::messages::MessageStore`、`backend::reader::{LocalLibrary, BookRoot}` 和 reader `create*` 返回对象继续作为 concrete Interface；没有第二实现时不增加 repository trait、service locator、factory registry 或命令总线。
3. `reader/app/src-tauri/src/lib.rs` 的目标责任是创建状态、注册 command / protocol、构建窗口和连接生命周期。平台来源检查、DTO 映射与稳定错误映射由对应 adapter module 拥有；迁移按真实变化逐片完成，不为文件对称机械拆分现有 library、telemetry 或 protocol 代码。
4. 第一迁移切片把全部消息 command 及共同的主窗口 / 阅读路由检查移动到 `message_commands` module。Tauri handler 注册名和 `allow-message-commands` permission 必须由检查脚本按集合双向精确匹配。
5. 书内 XHTML 继续没有 command interface；Svelte 壳 client、Tauri message adapter 与 `MessageStore` 是连续三层信任边界，不复制消息事实或验证规则。
6. 直接 Wry/Tao host 只保留为迁移期验证 adapter，不再承接产品能力。只有 Tauri gate 覆盖其独有安全、困难样本、崩溃恢复和 benchmark 证据后，才单独决定删除或重命名。
7. 快照资产的 crash-safe 发布 / 孤儿清理与完整备份 / 恢复是下一项高影响数据风险，但必须从进程中止和用户恢复场景设计；本决策不预建 storage port 或同步接口。

## 影响

- 新消息用例需要修改 `MessageStore`、一个 Tauri message adapter 和必要的 TypeScript client / capability；composition root 只注册，不实现用例。
- 平台、权限和来源错误集中在 adapter，领域校验和事务仍只在 backend；现有序列化名称、command 名称和错误代码保持不变。
- 本次增加一个源码 module，不增加 crate、依赖、进程、数据库、运行时复制或网络边界。
- 模块化单体仍允许以后形成新 adapter，但必须先出现真实平台、信任或第二实现需求。

负面后果是多一个 Rust source module，且文本配置 gate 需要随 Tauri permission 格式演进；`lib.rs` 中 library、telemetry 与 protocol 责任仍未拆出。收益是消息 IPC 的来源检查、错误映射和权限清单落在一个真实平台 Seam，领域模块无需引入替换性抽象。

## 备选方案

- 只补文档、不移动源码：否决；composition root 的高变更频率和 capability 漏检证据仍存在。
- 为消息、书架和存储统一建立 trait / port：否决；当前都只有一个实现，抽象不会减少风险。
- 拆分独立消息服务或事件总线：否决；没有独立部署、伸缩、故障隔离或跨进程消费者，反而增加隐私、事务和恢复成本。
- 立即删除旧 Wry/Tao host：暂缓；正式困难样本与安全 gate 仍使用它提供独有回归证据。

采用方案由一次历史 capability 漏授权、17 个现有 message command、旧 gate 漏读末项的可重现静态证据，以及现有 `MessageStore` / reader module 的具体接口共同支持；没有独立部署或第二实现证据支持拆服务或通用 port。

## 风险与缓解

- 源码移动造成 command 名、DTO 或错误行为变化：保留函数体与注册名，运行 Rust 测试、clippy、Svelte build 和消息专项。
- handler 与实际启用 permission 再次漂移：`scripts/check-message-reading.ps1` 只读取 `allow-message-commands` 块，并与注册集合双向精确比较；解析不到唯一块时失败。
- 文档把模块视图误当运行拓扑：`docs/architecture/OVERVIEW.md` 和 `docs/codebase/MAP.md` 同时记录 WebView2 多进程与 IPC，交由 `docs` gate 和架构 review 检查。
- 剩余 adapter 长期滞留 composition root：只有真实用例触碰 library、telemetry 或 protocol 边界时再迁移，不用一次性拆文件制造风险。

## 实施与检查位置

- 实施：`reader/app/src-tauri/src/message_commands.rs`、`reader/app/src-tauri/src/lib.rs`。
- 信任边界检查：`scripts/check-message-reading.ps1`、`reader/app/src-tauri/permissions/reader.toml`、`reader/app/src-tauri/capabilities/main.json`。
- 结构与状态：`docs/architecture/OVERVIEW.md`、`docs/codebase/MAP.md`、`docs/changes/atha-modular-monolith-boundaries.md`。

## 回滚与替代

本切片没有 schema、数据或协议迁移；需要回滚时可把现有 message command 原样移回 `lib.rs` 并恢复同名注册，数据库和前端无需转换。若以后出现独立部署、第二平台或第二存储，以新 ADR 取代本决定并迁移相应 adapter；不得在本 ADR 上静默改变拓扑。

## 复查触发器

- 出现第二个平台、第二种存储或需要独立进程隔离的真实需求；
- 新用例仍需同时修改三个不相干 adapter，或 composition root 再次承载业务规则；
- Tauri gate 已覆盖旧 host 的全部独有证据；
- 完整备份 / 恢复、同步或多设备一致性进入获批范围。

## 取代关系

本 ADR 不取代既有 ADR，当前也未被其他 ADR 取代。

## 相关文档

- 架构规范：`docs/architecture/DESIGN-GUIDE.md`
- 系统总览：`docs/architecture/OVERVIEW.md`
- 消息语义：`docs/architecture/MESSAGE-READING.md`
- 当前变更：`docs/changes/atha-modular-monolith-boundaries.md`

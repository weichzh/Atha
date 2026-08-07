# ADR-0004：模块化单体与显式平台 Adapter

## 状态

accepted

## 背景

Atha 已有一个进程、一个 Tauri / WebView2 产品入口、一个 SQLite 消息事实源和数个职责清楚的 backend / reader Module。2026-08-07 的 as-built 审计确认，`MessageStore`、`LocalLibrary`、`BookRoot` 和 browser reader kernel 已具备较深 Interface；主要结构风险集中在平台 composition root 和迁移期双 host，而不是缺少通用抽象。

同次审计发现，Tauri root 同时注册 17 个消息 command 并直接导入大量消息 DTO；历史真实链路曾因 capability 漏授权而失败。现有静态 gate 的 handler 正则要求尾逗号，因此会漏检注册列表最后一个 command。需要在不改产品行为、数据 schema 或运行拓扑的前提下，明确平台信任 Seam 并消除检查盲区。

## 决策

1. Atha 保持单进程模块化单体。backend deep module、browser reader kernel、Tauri adapter、Svelte 产品壳和 SQLite / 本地资产按现有进程内调用协作。
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

## 备选方案

- 只补文档、不移动源码：否决；composition root 的高变更频率和 capability 漏检证据仍存在。
- 为消息、书架和存储统一建立 trait / port：否决；当前都只有一个实现，抽象不会减少风险。
- 拆分独立消息服务或事件总线：否决；没有独立部署、伸缩、故障隔离或跨进程消费者，反而增加隐私、事务和恢复成本。
- 立即删除旧 Wry/Tao host：暂缓；正式困难样本与安全 gate 仍使用它提供独有回归证据。

## 复查触发器

- 出现第二个平台、第二种存储或需要独立进程隔离的真实需求；
- 新用例仍需同时修改三个不相干 adapter，或 composition root 再次承载业务规则；
- Tauri gate 已覆盖旧 host 的全部独有证据；
- 完整备份 / 恢复、同步或多设备一致性进入获批范围。

## 相关文档

- 架构规范：`docs/architecture/DESIGN-GUIDE.md`
- 系统总览：`docs/architecture/OVERVIEW.md`
- 消息语义：`docs/architecture/MESSAGE-READING.md`
- 当前变更：`docs/changes/atha-modular-monolith-boundaries.md`

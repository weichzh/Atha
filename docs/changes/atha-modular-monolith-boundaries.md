# Atha 模块化单体边界

## Status

implemented

## Problem

通用架构规范已经建立，但 Atha 系统总览仍主要描述概念分层，缺少经代码核对的 Module、运行时、数据、信任边界、质量场景、候选比较与风险顺序。2026-08-07 的只读审计确认当前 Cargo workspace 有 `atha-backend`、`atha-reader-host` 和 `atha-reader-app` 三个 package；产品 app 依赖前两者。Tauri `lib.rs` 为 786 行，同时拥有窗口 / protocol / library / diagnostics 与消息 IPC，直接注册 17 个消息 command；`MessageStore` 本身已有 21 个 concrete 公开方法，是应保留的 deep module。

历史验收曾发现主窗口 capability 未授权消息 command，导致真实书架 → 阅读 → 标注链路在 IPC 边界失败。当前 `check-message-reading.ps1` 虽会比较 handler 与 permission，却因正则要求尾逗号而漏掉注册列表最后一个 `message_export`。这说明首要问题是平台 adapter 的职责与可验证性，不是缺少 service、trait 或新进程。

## Scope

- 用 as-built 证据补全 Atha 系统级质量场景、Module / Interface / Seam / Adapter、运行时、数据与信任边界；
- 比较保持现状、显式 adapter 的模块化单体、全面 Ports and Adapters / 分布式拆分，记录选择与重评条件；
- 用 ADR 固定模块化单体和增量迁移原则；
- 把全部 Tauri 消息 command、共同来源校验和错误映射提取为一个 concrete platform adapter module；
- 保持 command 名、DTO、错误、capability、数据库、reader kernel 和运行拓扑不变；
- 修复消息 gate 的最后命令盲区，并把注册 command 与实际启用的 `allow-message-commands` permission 块双向精确比较；
- 更新 as-built 代码地图并完成目标检查、required gate 与独立 review。

## Non-Goals

- 不重写 reader kernel、`MessageStore`、EPUB importer 或 Svelte UI；
- 不增加 trait、repository、命令总线、service locator、插件、多格式工厂、crate、依赖、进程或网络接口；
- 不修改 schema、SQLite / 资产布局、消息语义、command / DTO 名称或安全策略；
- 不在本 change 中删除旧 Wry/Tao host，或实现备份、加密、checkpoint、同步与 AI；
- 不把文件行数本身当作拆分标准。

## Architecture Impact

present

- Design purpose：推动 composition root 向只负责装配演进，并把消息 IPC 的平台 / 信任责任集中在一个可验证 adapter；
- Drivers / quality scenarios：内容安全、消息数据完整性、引用保真、可修改性、隐私和既有性能门槛；
- Modules / Interfaces / Seams / Adapters：保留 `MessageStore` concrete Interface；新增源码级 Tauri message adapter；保持 TypeScript client 与 reader 消息投影 adapter；不新增业务层抽象；
- Candidate and tradeoffs：采用显式 adapter 的模块化单体；拒绝只补文档与全面 trait / 服务化，理由记录于 ADR-0004；
- Evidence / ADR / review trigger：消息专项检查、Rust / Svelte build、docs gate、双轴 review；第二平台 / 存储、独立进程需求、旧 host 证据等价或 composition root 再次承载规则时重评。

## Acceptance Criteria

- [x] `docs/architecture/OVERVIEW.md` 包含 as-built Module、运行时、数据、信任边界、质量场景、候选比较和风险顺序；
- [x] ADR-0004 固定模块化单体、显式真实 adapter 与避免单实现抽象的决策；
- [x] 所有现有消息 Tauri command 及共同 route / error 规则由单一 `message_commands` module 拥有；
- [x] `lib.rs` 保留状态构造与 handler 注册，不再直接实现消息用例或导入无关消息 DTO；
- [x] command 名、序列化 DTO、稳定错误、capability、数据库和运行行为不变；
- [x] 消息 gate 双向精确比较 17 个已注册 command 与 `allow-message-commands` 块内的 17 个 permission，并覆盖无尾逗号的最后一项；
- [x] `docs/codebase/MAP.md` 与三 package workspace、Tauri adapter 的 as-built 事实一致；
- [x] 目标检查、中文 Markdown 排版、docs gate、diff 检查与独立 Standards / Spec review 通过。

## Files And Steps

1. 更新系统总览、ADR、活动 change 与 `ACTIVE.md`。
2. 提取 Tauri message adapter，保持 handler 注册和外部契约不变。
3. 修复 command / permission 集合检查并运行消息专项检查。
4. 更新代码地图，在候选提交上运行 required gate 和双轴 review。

## Checks

- `cargo fmt --all --check`；
- `cargo test -p atha-reader-app`；
- `pwsh -NoProfile -File scripts/check-message-reading.ps1`；
- `autocorrect --fix` 与 `autocorrect --lint` 仅作用于本次中文 Markdown；
- `python scripts/doc_guard.py`；
- `python scripts/doc_length_check.py`；
- `git diff --check`；
- `project_workflow.py station atha-modular-monolith-boundaries --activity verification --gate docs`。

## Rollback

回退 message adapter 源码移动、检查脚本和文档即可；不涉及数据库迁移、依赖、外部系统或用户数据。由于 command 名与 DTO 不变，不需要兼容迁移。

## Approval

用户于 2026-08-07 明确要求基于架构规范区分 workflow 优化与 Atha 架构重设计，随后要求设定计划并开始实现；workflow 变更已经独立关闭，本 change 是已批准计划中的 Atha 架构轨道及首个迁移切片。

## Result

- Atha 的目标架构明确为单产品部署单元的模块化单体；系统总览现在如实记录原生 host、WebView2 多进程树与 IPC，并同时记录质量场景、as-built / target 差异、依赖方向、信任边界、候选取舍与风险迁移顺序；
- ADR-0004 固定 concrete deep module 与显式平台 adapter，拒绝当前没有消费者的 trait、service、进程和网络边界；
- 17 个消息 command、共同窗口 / URL 检查、稳定错误映射与原生导出 dialog 已从 Tauri root 移入 `message_commands`；所有外部名称和 backend 调用保持不变；
- 消息专项 gate 现在把 handler 注册与实际 `allow-message-commands` 块解析成集合并双向比较，实际无尾逗号的 `message_export` 已被纳入；
- 没有新增依赖、crate、数据迁移、运行时或产品行为。

## Review

- Standards：最终通过；初审发现的运行拓扑表述、ADR 最小字段和 permission 块解析三个 Blocking 均已关闭。注册 / 授权为 17 / 17，跨块探针正确隔离，重复目标块会拒绝；无 Non-blocking 或 smell。
- Spec：最终通过；17 个 command 函数体与固定点等价，permission、capability、DTO、错误和运行数据未改；无 Blocking 或 Non-blocking。
- Out-of-scope：快照恢复与旧 host 风险已登记，未在本切片扩项。

## Evidence And Residual Risks

- 只读静态证据：Cargo metadata 显示三个正式 package；Tauri app 的本地依赖为 backend 与旧 host，旧 host 仅依赖 backend；
- 结构证据：审计时 Tauri root 为 786 行、reader composition 为 458 行、注册 / permission 各 17 个消息 command，`MessageStore` 有 21 个 concrete 公开方法；
- 历史真实目标证据：消息 capability 曾漏授权并在真实 Tauri 链路失败，修复后已进入正式检查；
- 本地静态证据：`cargo fmt --all --check`、`cargo clippy -p atha-reader-app --all-targets -- -D warnings` 与 PowerShell 语法检查通过；
- 本地目标证据：`cargo test -p atha-reader-app` 的 3 项测试通过；`check-message-reading.ps1` 通过 16 项消息集成测试、Markdown 测试、Svelte check、production build、3 项 Tauri app 测试与 5 项旧 host 测试；
- 干净候选证据：提交 `2e35856` 的消息专项与 required `docs` gate 通过；修正初审 findings 后，提交 `92543c1` 的完整消息专项与 required `docs` gate 再次通过；
- 独立 review：以 `c9397a9` 为固定点，Standards 与 Spec 最终均通过且无未关闭 finding；
- 证据边界：本 change 的新增证据最高为 Windows 本地测试 / build 与静态检查，没有重新启动真实 Tauri / WebView2 交互链路；上文真实目标证据来自既有验收基线；
- 残余风险：快照资产的 crash-safe 发布 / 孤儿清理与完整备份 / 恢复仍未实现；旧 Wry/Tao host 与 runtime 清单仍在迁移期并存；TypeScript / Rust DTO 仍人工同步；这些风险必须按 ADR 的独立触发条件处理，不能在本切片顺手扩张。

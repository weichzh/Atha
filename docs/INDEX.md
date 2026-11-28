# 文档索引

本文件是 Atha 当前文档的注册表。先读 `ACTIVE.md`；只按任务选择一个 Context Bundle。已关闭工作的版本记录由 Git 保存，本机检查收据按需放在 Git 忽略的 `artifacts/local/`。

## Context Bundles

| 任务           | 读取                                                                                                         |
| -------------- | ------------------------------------------------------------------------------------------------------------ |
| `product`      | `product/OVERVIEW.md`、`roadmap/ROADMAP.md`、`codebase/MAP.md`、活动 change                                  |
| `architecture` | `architecture/DESIGN-GUIDE.md`、`architecture/OVERVIEW.md`、`codebase/MAP.md`、相关 ADR 与活动 change        |
| `reader`       | `architecture/READER-CORE.md`、`codebase/MAP.md`、相关 ADR、活动 change 与样本契约                           |
| `messages`     | `architecture/MESSAGE-READING.md`、`codebase/DATABASE.md`、活动 change                                       |
| `workflow`     | `agents/workflow.md`、`agents/issue-tracker.md`、`agents/references.md`、`workflow/PROTOCOL.md`、活动 change |
| `audit`        | 直接事实所有者与一份必要证据                                                                                 |

## 当前注册表

| 当前文档                                                           | 唯一事实                                   |
| ------------------------------------------------------------------ | ------------------------------------------ |
| [AGENTS.md](../AGENTS.md)                                          | 启动、硬规则和收尾                         |
| [CONTEXT.md](../CONTEXT.md)                                        | 稳定目标、目录和所有权                     |
| [ACTIVE.md](ACTIVE.md)                                             | 当前执行指针                               |
| [agents/workflow.md](agents/workflow.md)                           | GitHub Flow、协作规则与检查入口            |
| [agents/issue-tracker.md](agents/issue-tracker.md)                 | Issue 登记、交接与关闭规则                 |
| [agents/references.md](agents/references.md)                       | 外部技术与标准的官方入口和项目快速用法     |
| [workflow/PROTOCOL.md](workflow/PROTOCOL.md)                       | 任务和 change 生命周期                     |
| [product/OVERVIEW.md](product/OVERVIEW.md)                         | 产品目标与体验边界                         |
| [architecture/DESIGN-GUIDE.md](architecture/DESIGN-GUIDE.md)       | 架构设计、评估与审查规范                   |
| [architecture/OVERVIEW.md](architecture/OVERVIEW.md)               | 系统边界                                   |
| [architecture/READER-CORE.md](architecture/READER-CORE.md)         | 阅读内核与样本策略                         |
| [architecture/MESSAGE-READING.md](architecture/MESSAGE-READING.md) | 消息、引用与共读语义                       |
| [codebase/MAP.md](codebase/MAP.md)                                 | 已实现代码与验证入口                       |
| [codebase/READER-MOBILE-UI.md](codebase/READER-MOBILE-UI.md)       | 阅读界面的组件、布局和手工调整入口         |
| [codebase/DATABASE.md](codebase/DATABASE.md)                       | P0 数据库语义与缺口                        |
| [roadmap/ROADMAP.md](roadmap/ROADMAP.md)                           | 当前方向、候选顺序、暂缓范围与完成能力摘要 |
| [decisions/](decisions/)                                           | 仍有效的长期决策                           |
| [changes/](changes/)                                               | 需批准的实施方案与验收证据                 |
| [research/](research/)                                             | 当前未决技术研究                           |

## 生命周期

- `ACTIVE.md` 只指向正在执行的工作；任务关闭后清除 change 指针。
- `changes/` 保留活动变更及仍为未决验收提供上下文的记录；关闭后由 Git 追溯。
- `research/` 只保存尚未形成结论的问题；结论进入事实所有者后删除研究文件。
- 已实现能力的摘要见[路线图](roadmap/ROADMAP.md)。

## 模板

- `templates/change.md`：需批准的实施方案；
- `templates/decision.md`：仍有效的长期技术或产品决策。

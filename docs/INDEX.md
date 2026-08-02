# 文档索引

本文件是 ATHA 当前文档的注册表。先读 `ACTIVE.md`；只按任务选择一个 Context Bundle。历史档案不进入默认上下文。

## Context Bundles

| 任务 | 读取 |
| --- | --- |
| `product` | `product/OVERVIEW.md`、`architecture/OVERVIEW.md`、相关当前 change |
| `reader` | `architecture/READER-CORE.md`、`codebase/MAP.md`、相关 ADR/change 与样本契约 |
| `messages` | `architecture/MESSAGE-READING.md`、`codebase/DATABASE.md`、相关 change |
| `workflow` | `agents/workflow.md`、`workflow/PROTOCOL.md`、活动 change |
| `audit` | 直接事实所有者与一份必要证据；不自动升级为实施 |

## 当前注册表

| 当前文档 | 唯一事实 |
| --- | --- |
| [AGENTS.md](../AGENTS.md) | 启动、硬规则和收尾 |
| [CONTEXT.md](../CONTEXT.md) | 稳定目标、目录和所有权 |
| [ACTIVE.md](ACTIVE.md) | 当前执行指针 |
| [agents/workflow.md](agents/workflow.md) | 全局工作流的项目契约与检查 gate |
| [workflow/PROTOCOL.md](workflow/PROTOCOL.md) | 任务和 change 生命周期 |
| [product/OVERVIEW.md](product/OVERVIEW.md) | 产品目标与体验边界 |
| [architecture/OVERVIEW.md](architecture/OVERVIEW.md) | 系统边界 |
| [architecture/READER-CORE.md](architecture/READER-CORE.md) | 阅读内核与样本策略 |
| [architecture/MESSAGE-READING.md](architecture/MESSAGE-READING.md) | 消息、引用与共读语义 |
| [codebase/MAP.md](codebase/MAP.md) | 已实现代码与验证入口 |
| [codebase/DATABASE.md](codebase/DATABASE.md) | P0 数据库语义与缺口 |
| [roadmap/ROADMAP.md](roadmap/ROADMAP.md) | 交付顺序与暂缓范围 |
| [decisions/](decisions/) | 仍有效的长期决策 |
| [changes/](changes/) | 活动跨模块变更 |
| [research/](research/) | 当前未决技术研究 |

## 兼容档案

`milestones/`、`specs/`、`plans/`、`reviews/`、`studies/` 与旧模板保留到独立归档清理 change 决定前，只用于追溯。它们不再为新任务建立默认门禁，也不应被 `ACTIVE` 或 Context Bundle 自动加载。

## 模板

- `templates/change.md`：新跨模块 change；
- `templates/decision.md`：仍有效的长期技术或产品决策。

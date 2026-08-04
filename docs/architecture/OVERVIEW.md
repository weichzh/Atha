# Atha 架构总览

## 入口

产品目标和不可违反的体验原则见 `docs/product/OVERVIEW.md`。本文件只定义系统分层与长期边界；具体阅读和消息语义分别由专门文档维护。

当前仅面向 Windows，采取后端先于前端的顺序。Windows UI 可以采用窄窗口阅读体验，但不得反向污染领域模型、数据语义或阅读内核。范围决策见 `docs/decisions/ADR-0001-windows-backend-first.md`。

## 分层边界

### 产品界面层

负责窗口、书架、阅读页、对话浮层与设置。它调用应用服务，不直接拼接 SQL，也不拥有事实数据。

### 阅读内核

负责导入后 HTML、CSS 和本地资源的呈现、样式覆盖、位置恢复与性能缓存。详见 `docs/architecture/READER-CORE.md`。

### 应用服务层

负责用例边界、输入验证、事务和错误语义。它连接阅读内核、消息语义和本地存储，但不让平台对象或 UI 类型穿透边界。

### 可移植核心层

负责作品、版本、消息、引用、内容快照、关系与版本规则。它不依赖 Windows UI，也不预先固定未来的传输层或 AI 协议。

### 数据层

SQLite 与本地资产是事实源。用户可变数据、全文索引、缓存与未来词典索引应按清晰依赖关系迁移和恢复。

### 平台适配层

未来负责 Windows 文件选择、安全存储、系统分享、生命周期和诊断导出。平台能力经应用服务暴露，不直接侵入核心。

## 共读语义

阅读中的消息、引用、存档和未来 AI 书友边界见 `docs/architecture/MESSAGE-READING.md`。正式 `backend::messages` 已实现本地事实、迁移、查询、快照资产和导出；P0 数据库只保留为历史技术对照。

## 质量边界

- 不可信书源不得执行脚本；外部资源默认阻止，用户确认后才可加载。
- 诊断不得记录书名、原始路径、原文、笔记、查询词或 AI 提示词。
- 静态、本地、真实目标与生产等价证据必须明确区分。

## 相关文档

- 产品定义：`docs/product/OVERVIEW.md`
- 阅读内核：`docs/architecture/READER-CORE.md`
- 消息与共读：`docs/architecture/MESSAGE-READING.md`
- 当前状态：`docs/ACTIVE.md`
- 路线图：`docs/roadmap/ROADMAP.md`
- 代码现状：`docs/codebase/MAP.md`
- 数据库基线：`docs/codebase/DATABASE.md`

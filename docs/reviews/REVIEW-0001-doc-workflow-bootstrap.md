# REVIEW-0001：文档工作流初始化

## 范围

仅文档的工作流脚手架、旧提案迁移和仓库权威记忆初始化。

## Diff 摘要

- 新增项目级 `AGENTS.md` 和工作流技能；
- 新增 `ACTIVE`、`INDEX`、协议和模板；
- 将移动端 v0.1 提案降为历史研究；
- 建立 Windows 后端优先 ADR；
- 记录架构、代码库、数据库和路线图；
- 新增文档同步和长度守卫。

## 已执行检查

- [x] 文档已更新；
- [x] `ACTIVE` 已更新；
- [x] 中文 Markdown `autocorrect --fix` 已运行；
- [x] `autocorrect --lint` 已通过；
- [x] `scripts/doc_guard.py` 已通过；
- [x] `scripts/doc_length_check.py` 已通过；
- [x] RsProxy 环境下 P0 FFI 的 CTest 1/1 与 Rust 2/2 已通过；
- [x] `git diff --check` 已通过。

## 发现

- 原提案共 1,328 行且平台方向已失效，不适合继续作为日常权威入口。
- 当前只有 P0 实验，没有正式项目初始化；路线图已将根 workspace 初始化单列为 M1。
- `doc_guard.py` 默认代码路径不含 `backend/` 和 `p0/`；本次已补充 `backend/`、`p0/`、`scripts/` 和 Rust 根配置。
- RsProxy 配置是在工作流 bootstrap 前产生的初始化改动；本次将其作为无依赖的仓库工具链配置一并记录。

## 后续

- 关闭 M0 后创建 M1 里程碑；
- 起草并自审“Windows 后端工程初始化”规格；
- 起草计划并由独立 reviewer 或子 agent 交叉审阅；
- 用户批准计划后才创建正式后端代码。

## 结论

approved

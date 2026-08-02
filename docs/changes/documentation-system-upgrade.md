# ATHA 文档系统升级

## Status

implemented

## Problem

ATHA 当前把同一变更的范围、计划、评审和状态分散到多份文档，`ACTIVE` 承担过多工作信息，历史档案也进入默认索引。这增加上下文加载与同步成本，并不适合正在并行推进阅读内核和组合式引擎探索的项目。

## Scope

- 增加 ATHA 专属的稳定上下文和文档所有权；
- 将 `ACTIVE` 收敛为当前指针；
- 用单份 `docs/changes/*.md` 承载新跨模块变更的范围、计划、结果和 review；
- 将旧 milestone、spec、plan、review 与 study 降为迁移期兼容档案，不再作为默认工作流；
- 调整文档守卫以接受 `fast` 与 `change` 两条路径；
- 保留产品、架构、代码地图、阅读样本和历史决策。

## Non-Goals

- 不复制 translator 的单书、翻译、EPUB 或工站流程；
- 不删除现有历史文档，不改生产代码，不改变现有阅读器验收；
- 不把当前组合式阅读引擎研究伪称为已接受实现规格。

## Acceptance Criteria

- [x] 新会话可从 `AGENTS.md`、`CONTEXT.md`、`ACTIVE.md` 和按需 Context Bundle 获得最小当前上下文；
- [x] `ACTIVE.md` 不再复制范围、检查和详细状态；
- [x] 新跨模块变更只需一份 change 记录；
- [x] 旧四件套不再是新任务默认要求，仍可从兼容档案或 Git 追溯；
- [x] 文档守卫与长度检查适配新路径；
- [x] 中文文档格式化和结构检查通过。

## Files And Steps

1. 增加稳定上下文、change 模板和本次 change。
2. 收紧 `AGENTS.md`、`ACTIVE.md`、`INDEX.md` 与协议。
3. 更新机械检查；保留旧档案，后续有独立清理 change 才决定是否从工作树移除。
4. 运行针对性检查并记录结果。

## Checks

- `autocorrect --fix` 与 `autocorrect --lint`（本次触碰的中文 Markdown）；
- `python3 scripts/doc_guard.py`；
- `python3 scripts/doc_length_check.py`；
- `git diff --check`。

## Rollback

撤销本 change 的提交即可恢复旧工作流；本次不删除历史档案。

## Approval

用户已明确要求以 translator 的优点为参考、围绕 ATHA 现有项目进行文档与流程优化升级。

## Result

- 已建立 `CONTEXT.md`、精简的 `ACTIVE.md`、单 change 记录和对应文档守卫；
- 已增加 `docs/agents/workflow.md`，让全局 `project-workflow` 从项目文档读取任务类型与真实检查 gate；
- 已移除包含 ATHA 特例的同名本地 skill，项目知识继续由本仓库文档和脚本所有。

## Review

- Blocking: 无。
- Non-blocking: 无。
- Out-of-scope: 历史档案的物理清理另行决定。

## Evidence And Residual Risks

- Highest evidence level: Windows 本地文档检查与全局契约解析。
- Evidence: 契约已解析并批准；Atha 首个只读 audit 以 0 个 claim 启动并在无 commit receipt 的情况下正常关闭；工作流自检、`check docs`、AutoCorrect 与 `git diff --check` 通过。
- Residual risks: 旧档案暂时仍占用工作树，但不会进入默认上下文；全局与项目本机流程日志在迁移期存在重复记录。

# ATHA 统一工程 CLI

## Status

implemented

## Problem

ATHA 的正式检查分散在多个脚本中，现有 `Measure-Workflow.ps1` 又要求人工维护 run 和 phase。检查可以运行，但真实命令、结果和流程日志没有自动绑定，导致记录容易遗漏，也无法可靠比较失败和耗时。

## Scope

- 增加项目内 PowerShell 统一入口，只实现 `check docs`、`station` 和 `report`；
- 复用现有文档守卫与 JSONL，不复制检查逻辑、不新增依赖；
- 让统一入口自动记录命令开始、结束、活动、结果、退出码和耗时；
- 保持 schema v1 可读，并为新记录增加最小 schema v2 字段；
- 让文档守卫遵循当前 accepted change 工作流，不再要求兼容档案中的 full 规格、计划和 review；
- 更新当前工作流、代码地图和使用入口。

## Non-Goals

- 不接入 backend、reader、P0 或长驻服务命令；
- 不提供任意 shell 命令、插件、配置 DSL、数据库、仪表盘或远程遥测；
- 不记录正文、完整命令、输出、绝对路径、书名、样本名或用户数据；
- 不把工程命令成功称为产品验收或生产运行健康。

## Acceptance Criteria

- [x] `check docs` 自动执行现有两个文档守卫并原样传播成功或失败；
- [x] `station` 只记录活动边界，不运行项目检查；
- [x] `report` 兼容 schema v1/v2，显示完成、未结束、失败、median，并只在至少 5 个样本时显示 P95/慢样本；
- [x] 未知命令、未知 target、无效 activity/scope 在执行前失败；
- [x] 新日志只写 Git 忽略的 `artifacts/local/workflow/events.jsonl`；
- [x] CLI 自检、真实 `check docs`、AutoCorrect、文档守卫、长度检查和 `git diff --check` 通过。

## Files And Steps

1. 扩展 `Measure-Workflow.ps1` 的 schema v2 元数据、兼容报告和自检。
2. 增加 `Invoke-Atha.ps1`，用白名单包装 `check docs`，并提供 `station` 和 `report`。
3. 更新 `doc_guard.py` 以验证 ACTIVE 指向的 accepted/implemented change。
4. 更新工作流技能、协议、代码地图、README、研究结论和 ACTIVE。
5. 运行针对性验证并在本文件记录结果与 review。

## Checks

- `pwsh -NoProfile -File scripts/Measure-Workflow.ps1 -Action SelfCheck`；
- `pwsh -NoProfile -File scripts/Invoke-Atha.ps1 check docs -Activity validation`；
- CLI 参数拒绝与 JSON report 探针；
- 本次中文 Markdown 的 `autocorrect --fix` 与 `autocorrect --lint`；
- `python3 scripts/doc_guard.py`；
- `python3 scripts/doc_length_check.py`；
- `git diff --check`。

## Rollback

移除 `Invoke-Atha.ps1`，恢复 `Measure-Workflow.ps1` 和文档守卫即可；schema v2 与 v1 共用追加日志，旧记录不迁移也不删除。

## Approval

用户已明确要求根据 `workflow-station-cli-assessment.md` 实现流程优化，批准本文件 Scope 内的脚本和文档修改。

## Result

- 已增加 `Invoke-Atha.ps1`，日常调用只暴露 `check docs`、`station` 和 `report`；run/phase 由入口自动维护。
- `Measure-Workflow.ps1` 写 schema v2 元数据并兼容读取 v1；P95 与慢样本在少于 5 个样本时返回 null。
- `doc_guard.py` 改为验证 ACTIVE 指向且本次变更的 accepted/implemented change，不再依赖旧 full 四件套。
- 本机账本保留 1 次真实 station 和 3 次成功的 `check.docs`；受控失败样本验证后已清理。

## Review

- Blocking：无。
- Non-blocking：无。
- Out-of-scope：其他检查 target、长驻服务和跨项目抽取留待五次真实试点后决定。

## Evidence And Residual Risks

- Highest evidence level：Windows 本地 PowerShell CLI 与本机 JSONL 链路。
- Evidence：两个 PowerShell 文件语法解析通过；workflow self-check 通过；真实 `check docs` 通过；受控缺失 runner 返回 1，返回 7 的伪 runner 也原样传播并记录 failure；四类非法参数未写日志；混合报告识别 7 个完成 run、0 个未结束 run，3 个 `check.docs` 样本的 P95/慢样本保持 null。
- Residual risks：首版只有 `docs` target，现有真实样本为 3/5，尚不能证明其他检查也值得接入；全局 `project-workflow` 接入后会与本机 JSONL 暂时重复记录；没有产品运行时遥测。

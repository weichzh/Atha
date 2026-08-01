# 文档驱动协作协议

## 目的

本协议使仓库中的协作可恢复、可审阅、可机械检查：

- `AGENTS.md` 定义启动顺序和门禁；
- `docs/ACTIVE.md` 保存短期状态；
- `docs/INDEX.md` 映射长期记忆；
- 规格定义必须满足什么；
- 计划定义怎样实施；
- 评审记录实施是否符合规格和计划；
- 守卫脚本检查文档同步和长度；
- 本地流程日志记录阶段耗时、失败和未完成任务。

## 启动

每次会话按以下顺序读取：

1. `docs/ACTIVE.md`；
2. `ACTIVE` 信息不足时读取 `docs/INDEX.md`；
3. 当前任务明确需要的里程碑、规格、计划、决策、架构或 schema 文档。

禁止先读完整文档树。

## 模式

- `fast`：局部、可回滚且不改变依赖、接口、数据、安全或信任边界的工作流改动和维护修复；
- `discussion`：讨论、比较、记录和提出规格；禁止生产代码修改。
- `specification`：起草、自审和接受规格；禁止生产代码修改。
- `planning`：编写计划、测试、回滚并请求交叉审阅；禁止生产代码修改。
- `implementation`：仅在全部代码门禁通过时实施。
- `review`：检查 diff、运行验证和记录结论；禁止计划外生产修改。

## 快速路径

快速路径在 `ACTIVE` 记录范围、用户批准和针对性检查后直接实施。不创建里程碑、规格、计划、评审或交叉审阅。新增功能、跨模块行为、依赖、公开接口、数据或 schema、迁移、安全或信任边界必须使用完整路径。

## 完整路径规格规则

规格必须说明问题、目标、非目标、用户可见行为、内部行为、验收标准、边界情况、风险和关联文档。

接受前必须完成自审，明确歧义、缺失的验收标准、范围问题和风险。状态只使用 `draft`、`self-reviewed`、`accepted` 或 `superseded`。

## 完整路径计划规则

计划必须引用规格，列出实施方案、预计改动文件、步骤、测试、回滚、风险、文档同步和交叉审阅结果。

实施前必须由独立 reviewer 或子 agent 审阅。状态只使用 `draft`、`cross-reviewed`、`accepted`、`implemented` 或 `superseded`。

## 完整路径实施门禁

以下条件缺一不可：

- active 里程碑；
- accepted 规格；
- accepted 计划；
- 已通过交叉审阅；
- `ACTIVE` 允许修改生产代码；
- 用户已批准实施范围。

实施后必须运行验证、更新 `ACTIVE`、评审记录和结构文档，并运行文档守卫。输入事实修正只有在扩大范围、削弱验收或改变安全边界时才重审计划。

## 流程日志

修改版本控制文件的任务使用 `scripts/Measure-Workflow.ps1`。快速路径记录任务总耗时与 validation；完整路径记录 specification、planning、review、implementation、validation 和 documentation。

日志位于本机忽略的 `artifacts/local/workflow/events.jsonl`。`Task` 使用可重复比较的稳定流程类别，例如 `workflow-maintenance`；日志只含流程类别、阶段、状态、UTC 时间和耗时。禁止记录正文、路径、命令参数、URL、用户数据或秘密。

```powershell
$runId = pwsh -NoProfile -File scripts/Measure-Workflow.ps1 -Action Start -Task workflow-maintenance
pwsh -NoProfile -File scripts/Measure-Workflow.ps1 -Action Begin -RunId $runId -Phase validation
pwsh -NoProfile -File scripts/Measure-Workflow.ps1 -Action End -RunId $runId -Phase validation -Status success
pwsh -NoProfile -File scripts/Measure-Workflow.ps1 -Action Finish -RunId $runId -Status success
pwsh -NoProfile -File scripts/Measure-Workflow.ps1 -Action Report
```

## 收尾

修改版本控制文件或任务状态后更新 `docs/ACTIVE.md`，只保留当前路径、任务、门禁、阻塞、下一动作和最近检查；详细证据只写入评审。纯只读答复不修改 `ACTIVE`。

## 机械检查

```powershell
python3 scripts/doc_guard.py
python3 scripts/doc_length_check.py
```

## 长度限制

- `docs/ACTIVE.md`：最多 80 行；
- `docs/INDEX.md`：最多 250 行；
- 其他 `docs/**/*.md`：默认最多 400 行。

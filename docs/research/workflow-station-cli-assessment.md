# ATHA 统一工程 CLI：最小设计

## 结论

ATHA 应先做一个**项目内统一入口**，不先提取跨项目框架。第一版复用现有 PowerShell 脚本和 `Measure-Workflow.ps1` 的本地 JSONL，只把真实命令包进自动记录流程：调用者选择受支持的命令，CLI 自动记录开始、结果、退出码和耗时，并原样传播子命令退出码。

Translator 真正值得复用的不是手工维护 `start → begin → end → finish`，而是以下闭环：

```text
统一入口 → 自动打开工站 → 执行既有命令 → 自动关闭并落盘 → report 聚合
                     ↑
       station 只补记两次命令之间的人工或 Agent 活动
```

因此 `Start`、`Begin`、`End` 和 `Finish` 应降为 CLI 内部记录能力，不继续作为日常用户流程。`report` 监控的是工程命令和开发流程，不是产品运行时遥测；ATHA 当前还没有可监控的正式应用服务。

## 直接证据

- Translator 的 `tools/translator.py` 在统一入口的 `main()` 中包住真实命令；除 `benchmark` 外，每次调用自动记录机器耗时、结果和工站信息。`station` 本身不执行书籍任务，只建立人工或 Agent 活动边界。`E:\Code\translator\tools\translator.py`
- Translator 使用命令白名单，不提供任意 shell 执行器；`refresh` 和 `finalize` 复用既有检查并记录稳定步骤。它的流程日志、开放标记和 lane 状态都在 Git 忽略的本机目录。`E:\Code\translator\docs\codebase\CONTRACTS.md`
- ATHA 已有 `Measure-Workflow.ps1`，具备 token 校验、追加式 JSONL、未结束 run、median、P95 和失败聚合，不需要重建存储。[`Measure-Workflow.ps1`](../../scripts/Measure-Workflow.ps1)
- 当前本机日志只有 3 个已完成 run、0 个未结束 run，各分组仅 1 至 2 个样本。它证明旧日志可读，但不足以支持 P95、慢样本或流程优化结论。

## 用户命令面

建议入口保持 Windows 原生 PowerShell，不新增依赖：

```powershell
pwsh -NoProfile -File .\scripts\Invoke-Atha.ps1 check docs -Activity documentation
pwsh -NoProfile -File .\scripts\Invoke-Atha.ps1 station -Activity research -Scope workflow-cli
pwsh -NoProfile -File .\scripts\Invoke-Atha.ps1 report
```

第一版只需要三个顶层命令：

| 命令 | 行为 |
| --- | --- |
| `check <target>` | 从内置白名单调用现有正式检查，自动记录并传播退出码 |
| `station` | 不运行检查，只记录自上一工站后的声明活动、范围和结果 |
| `report` | 只读聚合日志；自身不进入统计 |

`check` 的 target 直接映射现有入口：`docs`、`backend`、`reader-slice`、`reader-samples`、`p0-ffi` 和 `p0-sqlite`。首个实现只接 `docs`，跑通五次真实使用后再逐个开放其余 target。不要提供 `run <arbitrary-command>`；它会泄露命令、破坏可比性，并复制 shell 已有能力。

`Serve-ReaderValidation.ps1` 暂不进入第一版。它是人工验收用的长驻进程，强行纳入会先引入进程存活、强制终止和端口状态语义。正式应用服务出现后，再单独设计 `serve/status`；不能把“未写 `run_end`”直接称为“服务仍健康”。

## 自动记录流程

一次 `check` 固定执行：

1. 校验 command、target、activity 和可选 scope 都是限长 ASCII token；
2. 从脚本内白名单解析真实入口，不从配置或用户输入拼接命令；
3. 在启动子命令前写 `run_start`，因此进程被强制终止时 report 能看到未结束 run；
4. 调用既有脚本，不复制其构建、测试或验收逻辑；
5. 在 `finally` 中写 `run_end`，保存状态、退出码、错误类型和经过时间；
6. 以子命令退出码作为 CLI 退出码，日志失败时也不得伪报检查成功。

第一版把一个既有脚本视为一个可比较命令，不拆它的内部步骤。只有实际失败记录证明“只知道 target 失败”不足以定位摩擦时，才让对应脚本发出稳定 step 事件。

## JSONL v2

继续使用 `artifacts/local/workflow/events.jsonl`，读取器兼容现有 schema v1。v2 在原事件上只增加必要字段：

```json
{"schema":2,"timestamp_utc":"UTC ISO-8601","event":"run_start|run_end","run_id":"opaque-id","task":"check.docs","command":"check","target":"docs","activity":"documentation","scope":"workflow-cli|null","status":"success|failure|blocked|cancelled|null","exit_code":"integer|null","error_type":"token|null","duration_ms":"integer|null","previous_run_id":"opaque-id|null","interval_ms":"integer|null"}
```

- `activity` 使用固定枚举：`none`、`research`、`specification`、`planning`、`implementation`、`validation`、`documentation`、`review` 和 `waiting`；`none` 不能与其他活动并用。
- `station` 也写一对零工作量的 run 事件，使旧 report 仍能识别完整 run；其经过间隔只能称为人工投入上界，不能称为专注工时。
- 不记录正文、提示词、完整命令、stdout/stderr、绝对路径、样本名、书名、用户数据或秘密。
- v2 不记录文件清单、哈希、Git 快照、actor 或 session。它们只有在真实并发或范围归因问题反复出现后才值得增加。

现有单文件 append 和短重试已经够用。先不改为每 run 一文件，不增加数据库、锁服务、保留策略或迁移命令；出现可复现的并发写入失败后再处理分片。

## Report

默认人类可读，`-Json` 提供结构化输出。第一版只报告：

- 已完成和未结束 run；
- 按 `task` 聚合的成功、失败、median 和最近失败；
- 连续同 target 失败、`blocked` 结果和未结束 run 这三类摩擦；
- 样本数达到 5 后才显示 P95 和异常慢样本。

Report 只给出观察，不自动修改代码、文档、门禁或命令白名单。检查成功也只代表对应脚本成功，不代表产品验收、用户批准或生产结果。

## 最小试点

实现前应为 CLI 新建一份 `accepted` change；当前研究和用户对设计的讨论不等于批准生产脚本修改。试点按以下顺序停止扩张：

1. 让 `Invoke-Atha.ps1 check docs` 包装现有文档守卫，并用现有 schema v1 日志验证向后兼容；
2. 连续记录 5 次真实文档检查，确认退出码、未结束 run、失败和耗时可信；
3. 若 report 确实减少了遗漏或暴露重复失败，再接入 `backend`；
4. 只有两个不同 target 都证明统一入口有价值，才接入其余检查；
5. 只有第二个真实项目出现相同需求和相同事件语义，才考虑提取跨项目核心。

`PLAN-0003` 目前仍是 draft 且属于兼容档案，不能作为唯一首个试点或实施门禁。组合式阅读引擎后续若形成 accepted change，可以作为普通 `check` 使用者，不应反向决定通用 CLI 结构。

## 停止条件

出现任一项就保留现有脚本，不继续泛化：

- 统一入口必须复制现有检查逻辑才能工作；
- 五次真实使用后仍未发现遗漏、失败或等待信息；
- 使用者频繁绕过 CLI，说明命令面没有降低成本；
- 为接入第二个 target 就需要 DSL、插件、数据库、后台服务或仪表盘；
- 工程流程日志开始承载产品内容、真实书籍信息或产品运行时指标。

## 当前证据与未决项

- 本研究结论已进入 `docs/changes/unified-engineering-cli.md`，不再作为默认执行上下文。
- 项目内 CLI 试点已实现并达到 Windows 本地证据；是否扩展 target 仍需至少 5 次真实使用。

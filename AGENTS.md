# AGENTS.md

## 启动顺序

开始任何工作前依次读取：

1. `docs/ACTIVE.md`；
2. `ACTIVE` 信息不足时再读 `docs/INDEX.md`；
3. `ACTIVE` 指定的当前里程碑；
4. 完成当前任务必需的规格、计划、决策、架构或代码库文档。

不要默认加载整个 `docs/`。

## 上下文纪律

使用能完成任务的最小上下文。优先读取 `ACTIVE`、索引、摘要和明确引用。

文档过长或混合多个主题时，先拆分或摘要，不把超长文档作为日常入口。

## 文档权威顺序

发生冲突时按以下顺序取舍：

1. `docs/ACTIVE.md`；
2. 当前里程碑；
3. 已接受规格；
4. 已接受决策；
5. 架构文档；
6. 代码库地图和 schema 文档；
7. 路线图；
8. 评审；
9. 讨论和历史研究。

## 变更路径

### 快速路径

同时满足以下条件时使用快速路径：

- 改动局部、可回滚且范围明确；
- 不新增依赖、公开接口、数据语义、迁移、权限或信任边界；
- 属于工作流工具、文档，或恢复已接受行为的维护修复；
- 有一项能直接证明结果的针对性检查。

快速路径只需：在 `ACTIVE` 标记 `流程：fast`、记录范围与用户批准，实施后运行针对性检查并更新 `ACTIVE`。不新建里程碑、规格、计划或评审，也不要求交叉审阅。

### 完整路径

新增功能、跨模块行为、依赖、公开接口、数据或 schema、迁移、安全或信任边界，以及难以回滚的改动，使用完整路径。无法确定时按完整路径处理。

已接受行为内的输入事实修正不自动触发计划重审；只有扩大范围、削弱验收或改变安全边界时才重审。

## 完整路径代码变更门禁

完整路径只有同时满足以下条件才能修改生产代码：

1. `ACTIVE` 指向 active 里程碑；
2. 任务规格状态为 `accepted`；
3. 实施计划状态为 `accepted`；
4. 计划列出预计改动文件；
5. 独立 reviewer 或子 agent 已完成交叉审阅；
6. `ACTIVE` 明确允许生产代码修改；
7. 用户已明确批准实施范围。

任一条件缺失时，停止写代码并补齐文档或批准。

## 必需流程

完整路径按以下顺序执行：

1. 在 `docs/specs/` 起草或更新规格；
2. 自审规格并补齐验收标准；
3. 在 `docs/plans/` 起草或更新实施计划；
4. 由独立 reviewer 或子 agent 交叉审阅计划；
5. 门禁全部通过后实施；
6. 运行相称的检查与测试；
7. 更新 `docs/ACTIVE.md`；
8. 结构或语义变化时更新代码库地图或 schema 文档；
9. 在 `docs/reviews/` 记录实施评审；
10. 运行 `python3 scripts/doc_guard.py` 和 `python3 scripts/doc_length_check.py`。

快速路径不得被用于规避本应进入完整路径的风险。

## 流程日志与计时

- 修改版本控制文件的任务使用 `scripts/Measure-Workflow.ps1` 写入本机忽略的 `artifacts/local/workflow/events.jsonl`；
- 快速路径至少记录任务开始、验证阶段和任务结束；完整路径记录 specification、planning、review、implementation、validation 与 documentation 阶段；
- 日志只保存稳定流程类别、阶段、状态、UTC 时间和耗时，不保存正文、路径、命令参数、URL、用户数据或秘密；
- 使用 `-Action Report` 查看各阶段样本数、失败数、median、P95、异常慢样本和未完成任务；
- 日志工具不可用时明确报告，但不得伪造时长或追记未实测数据。

## 会话收尾

修改版本控制文件或任务状态后，结束前必须在 `docs/ACTIVE.md` 记录当前路径、状态、问题或阻塞、下一动作、触碰范围及实际检查。纯只读答复不修改 `ACTIVE`。

## Agent skills

### Issue tracker

Issues 与 PRD 存放在 GitHub 仓库 `weichzh/Atha`。详见 `docs/agents/issue-tracker.md`。

### Triage labels

使用五个默认 triage 标签。详见 `docs/agents/triage-labels.md`。

### Domain docs

采用 single-context 领域文档布局。详见 `docs/agents/domain.md`。

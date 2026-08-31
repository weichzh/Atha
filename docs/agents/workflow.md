# ATHA 项目工作流适配

全局 `project-workflow` 管理任务 claim、工站证据、关闭状态和跨项目日志；本仓库只提供项目事实与真实检查命令。进入任务时仍按 `AGENTS.md` 读取 `CONTEXT.md`、`ACTIVE.md` 和对应 Context Bundle。

`docs/agents/references.md` 是外部技术与标准的项目参考地图。进入任务时先读它；涉及框架行为、API 语义、兼容性、错误或性能时，从地图直达对应版本的官方文档或源码，不凭记忆试错。

## 路由

- `audit` 是有停止条件的只读调查，不 claim 文件，也不要求提交；
- `fast` 直接维护局部实现及其事实所有者；
- `change` 必须对应一份已获批准且状态为 `accepted` 的 `docs/changes/*.md`，关闭前完成独立 review；
- GitHub issue 是外部请求入口，`docs/changes/` 是仓库内跨模块实施记录；具体生命周期由 `docs/workflow/PROTOCOL.md` 定义。

## 工站语义

全局 `project-workflow` 工站由 Atha 新任务主动填写的 activity 只使用 `research`、`specification`、`planning`、`implementation`、`verification`、`review`、`delivery` 和 `waiting`。任务名、检查目标、red/green、重跑次数和模块名属于 scope 或真实命令，不再创造 activity。`start`、`resume`、`cancel` 和 `checkpoint` 等 CLI 自动记录不受此约束。

检查必须从成本最低且最能区分假设的步骤开始：先做配置、语法和静态预检，再做受影响 Module 的目标检查，最后才运行完整 gate、真实浏览器或 benchmark。中间失败即停止扩展；修复后先重跑失败步骤，最终候选提交再运行 required gate。

预期拒绝、受控失败或 red 探针必须由项目脚本解释结果：观察到预期拒绝时脚本返回零；意外接受、错误的失败类型或未完成探针时返回非零。不得把原始预期非零直接登记为失败工站，也不得吞掉真实失败。

任务在首次编辑前 claim 精确路径。`--adopt` 只用于用户明确接管既有改动、已记录 handoff 或取消任务的后继任务，并在 change 中说明来源；不因 gate 扫描范围较大而扩大 claim。

上述规则的证据和计数保存在对应 workflow change；当前只有 Atha 一个仓库样本，因此本地契约与检查脚本优先，全局 CLI 保持不变。

## 迭代修复

只有验证失败后仍需继续修复，或 change 预先包含高成本、真实 GUI、真机或人工验收时，才使用迭代模式。所有轮次留在同一个 task 和 change；不得为每轮创建新 task、复制 change 或扩张 claim。

- `Review`：只读检查当前事实、实际制品与最新验证证据，按 `blocking`、`non-blocking`、`out-of-scope` 输出发现、证据、所有者和下一条最小检查，不修改文件；
- `Repair`：只处理已批准范围内的 `blocking` 与上一轮 `remaining_delta`，使用最小改动，不顺手处理其他候选，也不声称验证已经通过；
- `Validate`：先重跑上一轮失败的最小检查，再按成本顺序扩展；确定性测试、真实 GUI、真机和人工结果是通过依据，模型判断只能整理证据，不能替代执行。

每轮在 change 的 `Result` / `Evidence And Residual Risks` 与对应工站中留下同一份简短收据：`iteration`、`review findings`、`repair`、`validation cases`、`remaining_delta`、`evidence level` 和 `stop reason`。完整输出与含设备或私密样本的材料只链接既有工站或忽略证据目录，不复制进 Git 文档。

每份 change 在 `Iteration Control` 声明 `single-pass` 或 `iterative`。`single-pass` 在一次实施和验证后停止；若失败后还要继续编辑，先改为 `iterative` 并写明最大修复轮数、连续不变差异上限和人工 / 真实目标停止条件。迭代在验证通过、达到轮数上限、`remaining_delta` 达到不变上限或下一步需要人工决定 / 真实目标操作时停止；不得通过扩大范围、降低门槛或重复同一高成本命令强行继续。

只有彼此不共享工作树、候选制品、设备或外部状态的独立样本才能并行。迭代 change 关闭前以 `benchmark --summary --task TASK` 复核本任务；只有影响范围、正确性、安全、证据可信度、重复劳动或后续决策的事件才进入流程改进，并且每项只归属仓库规则、change、既有 skill、检查脚本 / validator、回归测试或 `no process change` 中的一个所有者。

## 检查

`docs` 是可交付任务的默认 gate。修改本仓库流程脚本时，在任务开始时追加 `workflow-self-check`；阅读器继续使用已有正式检查脚本，不为已停止的引擎实验保留 gate。

`scripts/check-docs.sh` 是 `docs` required gate 的 Bash 入口；它通过 `mise exec` 依次运行 `doc_guard.py` 和 `doc_length_check.py`。`scripts/check-workflow.sh` 先拒绝任何 Bash 检查脚本调用 PowerShell，再复用文档入口并运行文档守卫 self-check。旧 `scripts/Invoke-Atha.ps1` 保留为兼容入口，工站事件与报告聚合由全局 `project-workflow` 负责，不在仓库内复制实现。

本项目当前不需要 dotenv 变量；Rust 版本继续由 `rust-toolchain.toml` 固定。

```project-workflow
{
  "schema": 1,
  "task_tracker": {
    "kind": "local-markdown",
    "path": "docs/changes"
  },
  "task_types": {
    "audit": {
      "gates": [],
      "delivery": "none",
      "review_required": false
    },
    "fast": {
      "gates": [
        "docs"
      ],
      "review_required": false
    },
    "change": {
      "gates": [
        "docs"
      ],
      "review_required": true
    }
  },
  "gates": {
    "docs": [
      [
        "bash",
        "scripts/check-docs.sh"
      ]
    ],
    "workflow-self-check": [
      [
        "bash",
        "scripts/check-workflow.sh"
      ]
    ]
  },
  "delivery": {
    "review_required": false,
    "allow_waiver": false
  },
  "environment": {},
  "project_skill_roots": [
    ".agents/skills"
  ],
  "snapshot_excludes": []
}
```

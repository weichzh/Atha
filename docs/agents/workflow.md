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

## 检查

`docs` 是可交付任务的默认 gate。修改本仓库流程脚本时，在任务开始时追加 `workflow-self-check`；阅读器继续使用已有正式检查脚本，不为已停止的引擎实验保留 gate。

`scripts/Invoke-Atha.ps1` 仍是项目检查入口。它现有的本机日志与全局工站日志会暂时重复；在真实流程样本证明全局日志足够前，不为消除重复改写项目 CLI。

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
        "pwsh",
        "-NoProfile",
        "-File",
        "scripts/Invoke-Atha.ps1",
        "check",
        "docs",
        "-Activity",
        "validation",
        "-Scope",
        "project-workflow"
      ]
    ],
    "workflow-self-check": [
      [
        "pwsh",
        "-NoProfile",
        "-File",
        "scripts/Measure-Workflow.ps1",
        "-Action",
        "SelfCheck"
      ],
      [
        "python",
        "scripts/doc_guard.py",
        "--self-check"
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

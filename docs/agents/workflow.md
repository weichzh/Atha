# ATHA 项目工作流适配

全局 `project-workflow` 管理任务 claim、工站证据、关闭状态和跨项目日志；本仓库只提供项目事实与真实检查命令。进入任务时仍按 `AGENTS.md` 读取 `CONTEXT.md`、`ACTIVE.md` 和对应 Context Bundle。

`docs/agents/references.md` 是外部技术与标准的项目参考地图。进入任务时先读它；涉及框架行为、API 语义、兼容性、错误或性能时，从地图直达对应版本的官方文档或源码，不凭记忆试错。

## 路由

- `audit` 是有停止条件的只读调查，不 claim 文件，也不要求提交；
- `fast` 直接维护局部实现及其事实所有者；
- `change` 必须对应一份已获批准且状态为 `accepted` 的 `docs/changes/*.md`，关闭前完成独立 review；
- GitHub issue 是外部请求入口，`docs/changes/` 是仓库内跨模块实施记录；具体生命周期由 `docs/workflow/PROTOCOL.md` 定义。

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

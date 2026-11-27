# Atha 仓库协作与检查入口

开始、恢复或关闭仓库任务前读取 `AGENTS.md`、`CONTEXT.md`、`docs/ACTIVE.md` 和对应 Context Bundle。外部技术与标准的官方入口见 `docs/agents/references.md`；涉及框架行为、API、兼容性、错误或性能时，按需核对对应版本的官方文档或源码。

## GitHub Flow

1. **开短期分支**：独立任务从最新远端 `main` 开分支；依赖未合并工作时，记录所依赖的分支和提交。共享工作区正在使用或有在途改动时，用独立 worktree 开工。
2. **提交并验证**：一个分支只处理一项相关变化，按影响范围跑现有检查。跨会话工作关联 [Issue](issue-tracker.md)。
3. **开 PR**：获准外发后，按实际目标分支核对差异与提交，确认发布范围和材料，再推送任务分支并开 PR；需要早期反馈时可开 Draft PR。PR 写清范围、被测提交、环境、结果和未覆盖项。
4. **审查并合入**：PR 最终合入 `main`。处理阻塞意见和适用检查；候选或目标基线变化时，复核受影响的证据。获得明确授权后才合并。真实验收、发布和设备安装分别确认。
5. **收尾**：合并后更新相应 Issue；仍待验收的 Issue 保持打开。确认无后续分支依赖且获准清理后，删除短期分支。

## 工作路径

按[工作协议](../workflow/PROTOCOL.md)选择 `audit`、`fast` 或 `change`。在获批范围内工作，保留其他在途改动。

## Review、迭代与证据

独立 review 只读检查实现与验收条件，并把发现分为 `blocking`、`non-blocking`、`out-of-scope`。迭代任务在同一份 change 中按 Review、Repair、Validate 记录轮次、剩余差异、停止原因与证据等级；完整命令和输出按需保存在 Git 忽略的 `artifacts/local/`。

检查从成本最低且最能区分假设的步骤开始：配置、语法和静态预检，受影响 Module 的目标检查，仓库 gate，最后才是高成本 GUI、真实目标或 benchmark。低成本检查失败时先停止扩展并修复；预期拒绝必须由项目脚本确认正确失败类型并返回零，意外接受、错误失败类型或未完成检查返回非零。

## 仓库检查

- `bash scripts/check-docs.sh`：文档守卫与长度检查；
- `bash scripts/check-workflow.sh`：仓库流程文档与脚本自检；
- `bash scripts/check-fast.sh`：本地快速检查集合；
- `bash scripts/check-reader-linux.sh`、`bash scripts/check-pct-reader.sh` 和各领域脚本：按任务与目标选择，并记录验证环境。

Issue 保存跨会话交接，change 保存获批范围、技术决策和验收证据，PR 保存候选差异与审查，`ACTIVE` 指向当前工作。关闭任务前更新事实所有者和 `ACTIVE` 指针，运行相称检查，记录实际结果及覆盖缺口。Rust 版本由 `rust-toolchain.toml` 固定。

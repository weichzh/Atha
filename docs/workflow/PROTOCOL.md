# ATHA 工作协议

本协议定义任务分类、change 生命周期与收尾检查。

## 任务分类

### audit

有明确对象和停止条件的只读调查。读取事实所有者和必要证据，交付约定范围内的发现与证据。

### fast

适用于可回滚的局部维护，维持既有依赖、公开接口、数据、安全、信任边界和长期验收规则。按[仓库协作规则](../agents/workflow.md)在短期分支上实施，更新受影响的事实所有者和 `ACTIVE`，并在 PR 中记录范围和验证结果。

### change

涉及产品行为、依赖、架构、数据、阅读兼容性、验收、跨模块实现或难以回滚的变化。实施前必须有一份状态为 `accepted` 的 `docs/changes/<slug>.md`，并获得用户对范围的明确批准。

## 问题分流

先按观察到的信号选择唯一所有者，再决定任务类型。流程优化与产品或架构改动分别建任务、分别验收。

| 信号                                                                             | 路由            | 处理                                                                                       |
| -------------------------------------------------------------------------------- | --------------- | ------------------------------------------------------------------------------------------ |
| 重复失败、长等待、改动无法归因，或任务状态与当前文档漂移                         | Atha 仓库工作流 | 检查当前规则、Git 差异、仓库检查脚本和必要的忽略目录证据；在事实所有者与 change 中记录结论 |
| 用户无法完成打开、阅读、引用消息、讨论、搜索或回顾主循环                         | Atha 产品       | 更新产品事实与路线图，只选择一个可独立验收的纵向切片                                       |
| 质量场景、Module、Interface、Seam、Adapter、数据语义、信任边界或运行拓扑需要变化 | Atha 架构       | 按架构规范评估替代方案，并通过 `accepted` change 和必要 ADR 实施                           |
| 已获批切片内的局部实现问题                                                       | 所属代码 Module | 在既有范围内修复并运行最小相关检查                                                         |

## Change 记录

状态只使用 `proposed`、`accepted`、`implemented`。一份 change 至少保存 Problem、Scope、Acceptance Criteria、Files And Steps、Checks、Result、Review 与 Evidence And Residual Risks；有实际风险时，记录风险与回滚步骤。

每份新 change 必须包含 `Architecture Impact`，值只使用 `none` 或 `present`：

- `none`：不改变产品 Module、Interface、Seam、Adapter、数据语义、信任模型、运行拓扑、依赖或长期质量门槛；
- `present`：至少记录设计目的、关联驱动因素或质量场景、受影响的 Module / Interface / Seam / Adapter、一个合理替代方案及权衡、证据计划，以及适用的 ADR 或复查触发器。

具体架构内容由 `docs/architecture/DESIGN-GUIDE.md` 和对应事实所有者负责。

每份处于 `ACTIVE` 的 change 必须包含 `Iteration Control`，且只能有一个 `Mode`，值为 `single-pass` 或 `iterative`。局部且预期一次验证即可关闭的工作使用 `single-pass`；验证失败后若要继续编辑，必须先切换为 `iterative`。迭代模式在下一轮 Repair 前写明最大修复轮数、连续不变 `remaining_delta` 上限和人工 / 真实目标停止条件。已有 change 恢复为 `ACTIVE` 时补齐这些记录。

迭代修复使用同一份 change：Review 只读并形成有证据的发现，Repair 只处理本轮阻塞与上一轮剩余差异，Validate 实际执行最小检查并按成本扩展。每轮在 `Result` 记录 `iteration`、`review findings`、`repair`、`validation cases`、`remaining_delta`、`evidence level` 和 `stop reason`；完整命令与输出按需保存在 Git 忽略的 `artifacts/local/`，change 汇总证据等级和未覆盖项。检查结果对应实际执行的命令、候选和环境。验证通过、达到轮数上限、差异停滞或需要人工 / 真实目标决定时停止，并如实保留未关闭差异。

实现完成后做一次独立 review：

- `blocking`：违反范围、契约、安全或验收条件，必须关闭；
- `non-blocking`：不影响本次验收的改进候选；
- `out-of-scope`：登记到事实所有者或后续 change。

完成后将 change 标记为 `implemented`。归档时由 Git 保留历史。

## 阅读器特有原则

- 阅读统一用平台 WebView 渲染，平台对应关系见 [AGENTS.md](../../AGENTS.md)；
- 不可信书籍默认拒绝脚本、网络、路径越界和未知资源；
- 外部引擎只有在成熟度发生实质变化，并通过浏览器兼容、选择与重锚、内容无裁切和同机性能预检后，才重新进入技术决策；
- 性能数据必须定义起止点、冷/热状态和样本；跨设备结论使用对应目标上的实测结果。

## 收尾与检查

- `ACTIVE.md` 只保留当前指针；详细范围、检查和风险更新到事实所有者或活动 change；
- `docs/agents/workflow.md` 维护仓库协作规则与检查入口；本协议独占任务分类和 change 生命周期；
- 直接运行仓库 Bash 检查入口；`scripts/check-docs.sh` 和 `scripts/check-workflow.sh` 提供文档及流程自检；
- 预期拒绝类检查由项目脚本把“观察到预期拒绝”转换为成功退出；任何非预期接受、失败类型或未完成检查仍非零退出；
- 检查按静态预检、受影响 Module、required gate、真实目标或 benchmark 的成本顺序推进；前一层出现失败时先修复再扩大检查；
- Markdown 格式化由全局 Git 提交钩子处理；
- 文档改动运行仓库文档门，其他检查按任务选择，并执行 `git diff --check`；
- 验收结果注明环境、实际覆盖和缺口；真实目标的结论以该目标上的实测为据。

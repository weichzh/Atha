# ATHA 工作协议

本协议独占任务分类与生命周期。目标是让阅读器和消息模型以最小当前上下文推进，而不重复记录同一事实。

## 任务分类

### audit

有明确对象和停止条件的只读调查。读取事实所有者和必要证据，输出冻结结论；不修改产品、代码或外部交付，也不因发现问题自动扩张范围。

### fast

局部、可回滚且不改变依赖、公开接口、数据、安全、信任边界或长期验收规则的修复与维护。直接实施，更新受影响事实所有者和 `ACTIVE`；不创建 change。

### change

涉及产品行为、依赖、架构、数据、阅读兼容性、验收、跨模块实现或难以回滚的变化。实施前必须有一份状态为 `accepted` 的 `docs/changes/<slug>.md`，并获得用户对范围的明确批准。

## Change 记录

状态只使用 `proposed`、`accepted`、`implemented`。一份 change 至少保存 Problem、Scope、Acceptance Criteria、Files And Steps、Checks、Result、Review 与 Evidence And Residual Risks；需要时再写非目标、回滚和风险。

每份新 change 必须包含 `Architecture Impact`，值只使用 `none` 或 `present`：

- `none`：不改变产品 Module、Interface、Seam、Adapter、数据语义、信任模型、运行拓扑、依赖或长期质量门槛；
- `present`：至少记录设计目的、关联驱动因素或质量场景、受影响的 Module / Interface / Seam / Adapter、一个合理替代方案及权衡、证据计划，以及适用的 ADR 或复查触发器。

workflow 只负责要求这些输入存在并保存验收证据；具体架构内容由 `docs/architecture/DESIGN-GUIDE.md` 和对应事实所有者负责。不得把流程优化与产品架构重设计合并为同一 change。

同一事实不复制到 `ACTIVE`、计划和评审。实现完成后进行一次独立 review pass：

- `blocking`：违反范围、契约、安全或验收条件，必须关闭；
- `non-blocking`：不影响本次验收的改进候选；
- `out-of-scope`：登记到事实所有者或后续 change，不扩张当前工作。

完成后 change 标记为 `implemented`。确认不再提供当前上下文价值时，从工作树归档或删除，由 Git 保存历史。

## 阅读器特有原则

- WebView2 是当前唯一阅读渲染技术；不维护第二套 HTML/CSS、布局或绘制链；
- 不可信书籍默认拒绝脚本、网络、路径越界和未知资源；
- 外部引擎只有在成熟度发生实质变化，并通过浏览器兼容、选择与重锚、内容无裁切和同机性能预检后，才重新进入技术决策；
- 性能数据必须定义起止点、冷/热状态和样本；本地结果不能伪称为跨设备结论。

## 收尾与检查

- `ACTIVE.md` 只保留当前指针；详细范围、检查和风险更新到事实所有者或活动 change；
- 全局 `project-workflow` 按 `docs/agents/workflow.md` 管理 task claim、工站证据与关闭；本协议仍独占任务分类和 change 生命周期；
- 已接入的正式检查通过 `pwsh -NoProfile -File scripts/Invoke-Atha.ps1 check <target> -Activity <activity>` 运行；`Measure-Workflow.ps1` 只作为兼容记录层，不再手工维护日常 run/phase；
- 预期拒绝类检查由项目脚本把“观察到预期拒绝”转换为成功退出；任何非预期接受、失败类型或未完成检查仍非零退出；
- 检查按静态预检、受影响 Module、required gate、真实目标或 benchmark 的成本顺序推进，不在低成本失败未收敛时运行高成本链路；
- 修改中文 Markdown 后，只处理本次文件的 `autocorrect --fix`、适用格式化和 `autocorrect --lint`；
- 运行相称的测试、`python3 scripts/doc_guard.py`、`python3 scripts/doc_length_check.py` 和 `git diff --check`；
- 未运行或未覆盖的检查必须明确记录，不用静态或本地证据代替真实目标验收。

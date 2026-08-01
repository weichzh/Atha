# ACTIVE

## 当前模式

discussion

## 当前路径

- 流程：fast
- 里程碑：无；M2 已完成。

## 当前任务

- 任务：`WORKFLOW-0001`
- 类型：工作流快速路径、流程日志与计时 benchmark。
- 分支：`main`
- 用户批准：已明确批准。

## 允许动作

- [x] 修改工作流文档与工具脚本；
- [x] 运行针对性自检与文档守卫；
- [ ] 修改生产代码或产品测试；
- [ ] 推送或发布。

本任务不修改生产代码，不新增依赖、接口、数据或信任边界，符合快速路径。

## 当前状态

- 已确认：当前流程缺少小型维护快速路径，同一证据在多份文档重复；
- 已确认：两个阅读验收入口重复执行构建与测试，性能基准不应成为普通 UI 改动的默认门禁；
- 已完成：建立快速与完整两级路径，快速路径不再创建规格、计划、评审或交叉审阅；
- 已完成：建立本地追加式流程日志，报告完成数、失败、未完成任务、median、P95 和慢样本；
- 阻塞：无；
- 风险：日志只保存在本机忽略目录，不作为跨机器共享证据。

## 检查

- `Measure-Workflow.ps1 -Action SelfCheck`：通过，覆盖成功、失败和未完成流程；
- 首次实测 validation：34,336 ms；预完成报告正确识别 1 个未完成流程；
- `python3 -m py_compile`、`doc_guard.py`、`doc_length_check.py`：通过；
- 中文 Markdown `autocorrect --fix`/`--lint` 与 `git diff --check`：通过。

## 本次触碰文档

- `docs/ACTIVE.md`
- `AGENTS.md`
- `.agents/skills/project-workflow/SKILL.md`
- `docs/workflow/PROTOCOL.md`

## 本次触碰代码

- `scripts/Measure-Workflow.ps1`
- `scripts/doc_guard.py`
- `scripts/doc_length_check.py`

## 下一动作

后续版本文件变更按所选路径计时；同类流程累计 5 个样本后检查 P95 与慢样本。

# 软件架构设计规范

## Status

accepted

## Problem

Atha 已有产品、阅读内核、消息与数据边界，但缺少一套统一方法来说明架构应由哪些驱动因素决定、如何分解、如何权衡质量属性，以及如何记录和验证关键决策。项目仍处早期，若不先建立共同准则，后续按局部功能演进容易形成互相冲突的边界和隐含决策。

## Scope

- 研读英文原版 *Software Architecture in Practice*（2021）与 *Designing Software Architectures: A Practical Approach*（本机文件标记为 2024，以版权页为准）；
- 把两书中可操作的共同方法综合为一份中文规范，覆盖架构驱动因素、质量属性、设计流程、原则、模式与战术、文档、评估和治理；
- 结合 Atha 当前本地优先、单 WebView2、不可信书源和可追溯引用等约束，提供可直接执行的项目检查清单；
- 在架构总览和文档索引登记规范入口。

## Non-Goals

- 本次不审计、重构或推倒重写现有实现；
- 不预选微服务、事件总线、插件系统、同步或云端基础设施；
- 不复制两本书的章节摘要或模式目录，也不把书中示例当成 Atha 的既定决策；
- 不以二手资料替代原书，也不执行书籍中的脚本、链接或附件。

## Acceptance Criteria

- [x] 核对两本原书的书目信息、目录和可提取正文，并覆盖全书相关章节；
- [x] 规范明确使用“必须 / 应该 / 可以”的约束级别，并给出例外与升级条件；
- [x] 规范包含驱动因素、质量属性场景、迭代设计步骤、原则、常用模式与质量战术、文档视图、决策记录、评估方法和架构完成定义；
- [x] 规范把通用方法映射到 Atha 当前约束，提供可逐项核验的架构审查清单；
- [x] 关键结论可追溯到书名、章节和 PDF 页码或 EPUB 章节；
- [x] `docs/architecture/OVERVIEW.md`、`docs/INDEX.md` 与 `docs/ACTIVE.md` 指向新规范；
- [ ] 中文 Markdown 排版、仓库文档 gate 和独立 review 通过。

## Files And Steps

1. 安全提取两本原书的元数据、目录与正文，并抽样核对 PDF 页面和 EPUB 章节。
2. 新建 `docs/architecture/DESIGN-GUIDE.md`，先综合通用规范，再加入 Atha 项目化门禁。
3. 更新 `docs/architecture/OVERVIEW.md`、`docs/INDEX.md` 与 `docs/ACTIVE.md` 的入口。
4. 运行排版、文档 gate、diff 检查与独立 review；记录证据和残余风险。

## Checks

- 原书元数据、页数 / 章节数、目录和正文抽样；
- `autocorrect --fix` 与 `autocorrect --lint` 仅作用于本次中文 Markdown；
- `python scripts/doc_guard.py`；
- `python scripts/doc_length_check.py`；
- `pwsh -NoProfile -File scripts/Invoke-Atha.ps1 check docs -Activity validation -Scope project-workflow`；
- `git diff --check` 与独立 review。

## Rollback

删除新增规范，并回退本次对架构总览、索引和活动指针的链接即可；不涉及代码、依赖、数据迁移或外部系统。

## Approval

用户于 2026-08-07 明确要求研读两本英文原版并形成可用于重新审视 Atha 的软件架构标准规范和指南，已批准上述文档范围。

## Result

- 新增 `docs/architecture/DESIGN-GUIDE.md`，以 316 行覆盖架构驱动因素、质量场景、ADD 循环、决策与接口、质量战术、模式、视图、ADR、评估、债务、完成定义和反模式；
- 将原书综合规则与 Atha 适配明确分层，并给出本地优先、WebView2、不可信书源、引用保真和单一事实源的项目基线与审查清单；
- 在架构总览、文档索引和活动 Context Bundle 登记统一入口；
- 本 change 不包含现有架构审计、重构、依赖、代码、数据或外部系统变更。

## Review

- Blocking: 待 review。
- Non-blocking: 待 review。
- Out-of-scope: 待 review。

## Evidence And Residual Risks

- 原书静态证据：SAIP4 EPUB 核对 48 个 spine 项和 26 章；DSA2 PDF 核对 455 个物理页、13 章、战术问卷附录和补充案例。另渲染抽查书名页、版权页、目录、ADD、分析、组织和附录页面，版面与提取文本一致；
- 文档静态证据：`autocorrect --fix` 仅作用于五份本次中文 Markdown；`autocorrect --lint`、`doc_guard.py`、`doc_length_check.py` 和 `git diff --check` 均通过；
- 残余风险：规范是两书方法的综合与项目化推导，不替代针对当前代码的架构评估；具体性能门槛、样本、运行拓扑和迁移策略仍须在各 change 中以真实入口验证。EPUB 没有稳定纸页，故以章、节和源文件哈希定位；
- 书籍是用户提供的本机资料，本次仅做本地静态读取，没有执行书中内容或上传到外部服务。

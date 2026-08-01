# ACTIVE

## 当前模式

discussion

## 当前里程碑

- 文件：`docs/milestones/M1-windows-backend-foundation.md`
- 目标：建立正式根 Cargo workspace、最小后端 crate 和单一验证入口。
- 状态：completed

## 当前任务

- 任务：`BACKEND-INIT-0001`
- 类型：已完成的 Windows 后端工程初始化。
- 规格：`docs/specs/SPEC-0001-windows-backend-foundation.md`，状态 `accepted`。
- 计划：`docs/plans/PLAN-0001-windows-backend-foundation.md`，状态 `implemented`。
- 评审：`docs/reviews/REVIEW-0002-windows-backend-foundation.md`，状态 `approved`。
- 分支：`main`

## 允许动作

- [x] 讨论；
- [x] 起草下一里程碑和规格；
- [ ] 修改测试；
- [ ] 修改生产代码；
- [ ] 后续推送或发布（本次用户明确批准的首次公开发布已完成）。

M1 已关闭。M2 的 active 里程碑、accepted 规格、accepted 计划、交叉审阅和用户批准建立前，生产代码保持冻结。

## 当前状态

- 已完成：用户接受 `SPEC-0001` 并批准实施。
- 已完成：`PLAN-0001` 经独立 reviewer 修订复审后 `approved` 并已实施。
- 已完成：根 workspace、零依赖后端 crate、检查入口和 SQLite 迁移 ADR。
- 已完成：正式后端检查、失败路径探针、P0 FFI 回归和全部文档检查。
- 已完成：创建公开 GitHub 仓库 `weichzh/Atha`，将 `main` 设为默认分支并首次推送。
- 已完成：为工程技能配置 GitHub issue tracker、默认 triage 标签和 single-context 领域文档。
- 进行中：无。
- 阻塞：无。
- 风险：正式后端仍无产品行为；不能在空 crate 中直接追加 M2 代码。

## 当前权威文档

- `docs/milestones/M1-windows-backend-foundation.md`
- `docs/specs/SPEC-0001-windows-backend-foundation.md`
- `docs/plans/PLAN-0001-windows-backend-foundation.md`
- `docs/reviews/REVIEW-0002-windows-backend-foundation.md`
- `docs/decisions/ADR-0002-sqlite-and-migrations.md`
- `docs/codebase/MAP.md`
- `docs/codebase/DATABASE.md`
- `docs/roadmap/ROADMAP.md`

## 下一会话所需上下文

依次只读：

1. `AGENTS.md`；
2. `docs/ACTIVE.md`；
3. `docs/roadmap/ROADMAP.md`；
4. 创建 M2 规格时再读架构、数据库基线、SQLite ADR 和 P0 schema。

## 检查

- 公开发布预检：常见敏感文件名与密钥模式扫描通过，覆盖 5 个提交和 53 条历史路径；`gitleaks` 未安装；
- `gh repo view weichzh/Atha`：公开仓库、默认分支和 URL 验证通过；
- 中文 Markdown `autocorrect --fix`：通过；
- 项目 Markdown formatter：未配置；
- 中文 Markdown `autocorrect --lint`：通过；
- `python3 scripts/doc_guard.py`：通过；
- `python3 scripts/doc_length_check.py`：通过；
- `git diff --check`：通过；
- `git ls-remote --heads origin main`：远端 `main` 验证通过。

## 本次触碰文档

- `AGENTS.md`
- `docs/ACTIVE.md`
- `docs/INDEX.md`
- `docs/agents/domain.md`
- `docs/agents/issue-tracker.md`
- `docs/agents/triage-labels.md`

## 本次触碰代码

- 无。

## 下一动作

创建 M2“首个后端纵向切片”的里程碑和规格，先定义真实用例、输入边界、迁移与事务验收；规格接受前不添加依赖或业务代码。

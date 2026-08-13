---
description: 清理已关闭变更与已解决研究，并解除测试对历史研究文件的依赖。
---

# 当前文档生命周期收口

## Status

implemented

用户于 2026-08-13 批准按审计路线图开始持续 goal，并要求每个纵向切片独立验收后再进入下一项。本 change 接管已取消的 fast task `atha-current-doc-lifecycle-cleanup-20260813` 的现有改动；该任务因文档门发现代码测试依赖研究文件而升级。

## Architecture Impact

none

不改变产品模块、接口、数据、安全边界、运行拓扑、依赖或长期质量门槛；只让当前文档目录符合既有生命周期，并把仓库 Markdown 导入测试改用稳定事实所有者。

## Problem

`docs/changes/` 留有 21 份已实现记录，`docs/research/` 留有 14 份已经形成结论的研究，与 `docs/INDEX.md` 的当前文档契约冲突。其中一份研究还被 Markdown 导入测试和 Windows 检查脚本当作固定输入，使历史研究无法正常关闭。路线图同时仍把已交付能力写成未来目标，并把消息数据库备份称为全库备份。

## Scope

- 删除已关闭 change 与已解决 research，保留两个目录的 `.gitkeep`；历史继续由 Git 和工作流收据追溯。
- 把仓库 Markdown 导入测试与 Windows 检查源改为稳定的 `docs/architecture/READER-CORE.md`。
- 移除当前文档对已删除记录的具体引用，并把路线图校准为收口、完整本地数据生命周期、跨书记忆、桌面工作区和内部候选顺序。
- 不改变导入实现、产品行为、依赖、格式契约或真实设备状态。

## Acceptance Criteria

- `docs/changes/` 除本 change 与 `.gitkeep` 外没有已关闭记录；本 change 关闭后也从当前工作树删除。
- `docs/research/` 只剩 `.gitkeep`，仓库没有指向已删除研究的具体路径。
- Markdown / TXT 后端测试继续覆盖两个真实仓库 Markdown 源，并全部通过。
- 文档门、Rust 格式、中文排版检查与 `git diff --check` 通过。

## Files And Steps

1. 删除已关闭 change 与已解决 research，修正所有当前引用。
2. 将测试源从研究文件切换到阅读内核事实所有者，保留原有单节与目录断言。
3. 校准路线图的 `Now`、`Next`、`Later` 和完成项事实所有者。
4. 运行目标测试、required docs gate 和独立 review；提交并关闭后删除本记录。

## Checks

- `cargo test --locked -p atha-backend --test text_import`
- `bash scripts/check-docs.sh`
- `cargo fmt --all -- --check`
- `autocorrect --lint <本次修改的中文 Markdown>`
- `git diff --check`

## Result

- 删除 21 份已实现 change 与 14 份已解决 research，两个目录只保留当前 change 和 `.gitkeep`。
- 当前文档不再引用被删除的具体文件；许可证 ADR 改由 Git 与工作流收据追溯历史证据。
- 仓库 Markdown 导入测试与 Windows 检查源改用 `docs/architecture/READER-CORE.md`，原有真实 Markdown、单 section 和目录断言保持有效。
- 路线图已校准为项目收口、完整本地数据生命周期、跨书阅读记忆、桌面工作区和内部可安装候选，并明确现有备份只覆盖消息数据库。
- 目标测试通过：17 passed、1 ignored；required docs gate、Rust 格式、中文排版和 diff 检查通过。

## Review

独立 review pass 未发现 blocking、non-blocking 或 out-of-scope finding。测试改用现有事实所有者，没有新增 fixture、依赖或第二套文档归档；路线图中的事实所有者路径均存在。

## Evidence And Residual Risks

当前最高证据为本地后端集成测试与静态文档门。删除的历史文件仍可从 Git 与已关闭 workflow task 恢复；本 change 不重新执行其 Linux GUI、Android 模拟器或 PCT-AL10 验收。Windows PowerShell 入口未在 Windows 上重跑，本地直接执行同一后端测试已经覆盖替换后的仓库 Markdown 源。

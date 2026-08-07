# 项目采用 AGPL-3.0-or-later

## Status

implemented

## Problem

Atha 当前没有根许可证、Cargo / npm 许可证元数据或第三方内容边界。用户已明确选择 `AGPL-3.0-or-later`；如果不先落地，Android 分发、Readest / foliate-js 复用、`libmobi` 链接、MDict 候选和 CSS 社区仓库都会继续依赖含糊的许可假设。

## Scope

- 在仓库根放置未经修改的 GNU Affero General Public License v3 官方英文全文；
- 为正式 Cargo workspace、独立 P0 crate 与前端 package 写入精确 SPDX `AGPL-3.0-or-later`；
- 在 README 与稳定上下文中声明 Atha 第一方代码的许可和第三方代码 / 资产边界；
- 为仓库现有的 Microsoft Fluent System Icons 书签 SVG 保留固定来源、版权与 MIT 许可全文；
- 更新架构决策、官方参考和依赖审查规则，要求核对精确 SPDX 与实际分发义务；
- 修正当前 Android 格式与词典研究中“项目许可证未决”的失效结论，同时保留 LGPL、AGPL-only、字体、书籍和词典各自义务。

## Non-Goals

- 不把第三方代码、字体、书籍、词典、fixture 或其他资产重新许可为 Atha 的许可证；
- 不因 Atha 采用 AGPL 就自动批准 `AGPL-3.0-only`、LGPL 静态链接或来源不明的测试数据；
- 不批量给所有源码增加 SPDX 文件头，不创建重复 `COPYING` 或空的 `NOTICE`；
- 不在本切片引入依赖扫描服务、CLA、贡献者协议或 CSS 社区仓库策略。

## Architecture Impact

present

- Design purpose: 在 Android 与第三方解析库进入产品前建立单一、可机读、可追溯的第一方许可事实；所有 manifest、README、CONTEXT 与根许可证一致时停止。
- Drivers / quality scenarios: `A-LIC-01`（最高业务重要性 / 高技术风险，负责人：项目维护者）；刺激源是维护者、贡献者或依赖更新，刺激是发布源码 / 安装包或接入第三方实现，环境是公开仓库、离线构建及 Android 分发，制品是第一方源码、manifest、分发包与第三方资产边界，响应是声明 `AGPL-3.0-or-later` 并逐项保留第三方义务；度量是根许可证存在且为官方 AGPLv3 文本、所有第一方 package 元数据精确一致、锁文件不变、研究不再把项目许可证标为未决，任何 `-only` / LGPL / 数据许可风险仍显式阻塞接入。
- Modules / Interfaces / Seams / Adapters: 根 `LICENSE` 与 `CONTEXT.md` 拥有稳定法律约束；Cargo / npm manifest 是工具可读投影；研究与依赖规则消费该事实，不另建许可证服务或第二份正文。
- Candidate and tradeoffs: 不继续无许可证；不选择宽松许可证或专有许可，因为用户已决定 AGPL；只维护一份官方全文，避免 `LICENSE` / `COPYING` 分叉。
- Evidence / ADR / review trigger: `ADR-0008`、官方 GNU / SPDX / Cargo / npm 文档、manifest 解析、锁文件不变检查、独立 Spec / Standards review；改变项目许可证、双许可、CLA 或首次正式分发时复查。

## Acceptance Criteria

- [x] 根 `LICENSE` 是未经修改的 GNU AGPL v3 官方英文全文；
- [x] 正式 Cargo workspace 的三个 member、独立 P0 crate 和前端 package 均声明精确 `AGPL-3.0-or-later`；
- [x] README、CONTEXT、ADR、官方参考和依赖审查规则一致，且明确第三方代码与资产不被重新许可；现有 Fluent System Icons SVG 携带上游版权与 MIT 许可全文；
- [x] Readest / 词典研究反映项目许可证已决定，但 `AGPL-3.0-only`、LGPL 链接和本机词典授权仍是独立门槛；
- [x] Cargo / npm 锁文件保持不变，docs gate、AutoCorrect 与独立 review 通过。

## Files And Steps

1. 固定官方许可证正文并写入各 manifest 的最小 SPDX 元数据；
2. 更新稳定事实、架构决策、依赖许可规则和失效研究结论；
3. 验证文本、metadata、锁文件、文档与双轴 review。

## Checks

- 校验 `LICENSE` 与 GNU 官方纯文本一致；
- 校验 `THIRD_PARTY_NOTICES.md` 包含固定上游版本、资产路径与 Microsoft MIT 许可原文；
- `cargo metadata --no-deps --locked` 检查正式 package 许可证，独立检查 P0 Cargo manifest 与前端 `package.json`；
- `git diff --exit-code -- Cargo.lock reader/app/pnpm-lock.yaml p0/ffi/rust/Cargo.lock`；
- `autocorrect --fix / --lint` 本次中文 Markdown；
- required `docs` gate 与独立 Spec / Standards review。

## Rollback

首次对外分发前可删除本次元数据并另做已批准的许可证决定；已经依据某版本许可证取得的权利不能追溯撤销。不得把“回滚 Git 提交”误写成撤销既有授权。

## Approval

用户于 2026-08-08 明确决定本项目采用 `AGPL-3.0-or-later`，并要求继续路线图开发。

## Result

根 `LICENSE` 现为 GNU 官方 AGPL v3 英文全文，正式 Cargo workspace、独立 P0 crate 与前端 package 统一投影 `AGPL-3.0-or-later`。README、CONTEXT、ADR、架构依赖规则与两份当前研究明确区分 Atha 第一方代码、第三方依赖义务和不可再分发的本机资产。

## Review

- Blocking: 0；独立 Spec / Standards review 均确认此前缺失的 Microsoft Fluent MIT 声明已补齐。
- Non-blocking: 0。
- Out-of-scope: 完整传递依赖清单、安装包 notices、CSS 社区仓库贡献条款与 Android LGPL 重新链接材料在对应分发切片处理。

## Evidence And Residual Risks

- 静态 / 本地证据：根 `LICENSE` 与 GNU 官方纯文本 SHA-256 同为 `0D96A4FF68AD6D4B6F1F30F713B18D5184912BA8DD389F86AA7710DB079ABCB0`；`cargo metadata --no-deps --locked` 验证 workspace 3 个 package 与独立 P0 package，JSON 解析验证前端 package，五者均为精确 `AGPL-3.0-or-later`。
- 第三方资产证据：书签 SVG 固定到 Fluent System Icons 提交 `9e9a1766ae48f4a138fed896b25a59a5f6619230`；本地与上游 XML 语义一致，`THIRD_PARTY_NOTICES.md` 包含该版本完整 Microsoft MIT 文本。
- 锁文件证据：`Cargo.lock`、`reader/app/pnpm-lock.yaml`、`p0/ffi/rust/Cargo.lock` 均无 diff。
- 文档证据：本次中文 Markdown 的 AutoCorrect fix / lint 无问题，required `docs` gate 通过，独立 Spec / Standards review 均为 Blocking 0、Non-blocking 0。
- 尚未完成：首次正式分发所需的完整传递依赖 / 资产清单、notices、对应源码提供与 Android LGPL 重新链接材料；这些必须在分发切片关闭前完成。
- 本文提供工程边界，不构成法律意见；`AGPL-3.0-only` 组合兼容性或商业许可仍需专门判断。

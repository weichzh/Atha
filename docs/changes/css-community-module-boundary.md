---
description: CSS 社区暂缓期间的模块包复用边界。
---

# CSS 社区模块边界

## Status

implemented

## Problem

现有 schema 1 CSS 模块 JSON 已能导入导出，但解析、字段校验、大小限制和 CSSOM 校验仍嵌在 Preferences UI 内。用户决定暂不实现 GitHub 社区，只保留将来数据源可复用的模块化接口；若现在加入 OAuth、网络、仓库、PR 或占位页面，会增加未验证的产品和安全负担。提取后的 Linux 真壳回归还稳定暴露了全局防抖 timer 与手动保存重复排队的问题，持久化失败提示会早于当前草稿真正回滚；同一 timer 若简单清除，还会丢失另一模块尚未落盘的草稿。

## Scope

- 把现有 schema 1 模块包的解析、序列化、严格字段校验、大小上限、重复 ID 检查和 CSS 校验提取为独立无网络 module；
- 让现有本地导入、导出、状态恢复和 benchmark 复用该 module，不新增 UI 或持久化 schema；
- 防抖 timer 按模块 ID 隔离，手动保存只取消同模块的待处理预览；删除、导入和重置不留下失效 timer；
- 用一个 Node 测试锁定合法往返及未知字段、重复 ID、不安全 CSS 的拒绝；
- 路线图把完整 GitHub 社区暂缓，并将阅读统计提升为当前切片。

## Non-Goals

- GitHub 登录、OAuth、Contents / Pull Requests API、远程索引、版本追踪、投稿或审核；
- 社区入口、占位页、provider registry、账户、网络权限、缓存数据库或新依赖；
- 修改 CSS 模块 schema、每书状态、ReaderManifest、Locator 或 CSS 安全规则。

## Architecture Impact

present

- Design purpose: 让任意未来社区数据源只能向同一个有界模块包 codec 交付文本，不能绕过现有 schema 与 CSSOM 信任边界。
- Drivers / quality scenarios: `A-CSS-COMMUNITY-01` 要求未来来源不复制解析规则；`A-CSS-SEC-01` 要求本地和远程候选都经过同一严格校验。
- Modules / Interfaces / Seams / Adapters: 新 `style-module-package.mjs` 只暴露 codec；`preferences.mjs` 继续拥有 UI、状态、按模块防抖和组合；`content.validateStylesheet` 继续拥有 CSS 最终判定。
- Candidate and tradeoffs: 不预建 `CommunityProvider` interface。schema 1 文本包已经是足够稳定的交换边界，真正接 GitHub 时再添加一个具体数据源调用 codec。
- Evidence / ADR / review trigger: Node codec 测试、reader bundle 语法、Svelte build 和既有 Preferences 自检；只有恢复完整 GitHub 社区时才重新研究平台 API、登录与远程信任边界。

## Acceptance Criteria

- [x] 本地模块导入、导出、恢复和 32 模块 benchmark 复用同一个独立 codec，现有数据与界面不变；
- [x] 合法 schema 1 往返通过，包含 32 个最大停用模块的包仍可序列化后重新解析；未知字段、重复 ID、越界输入和不安全 CSS 稳定拒绝；
- [x] 手动保存不再与同模块防抖预览重复排队，也不会清除另一模块尚未落盘的草稿；
- [x] 产品没有新增网络、登录、GitHub 或社区占位入口，也没有新增依赖；
- [x] Node、Svelte、Rust、文档检查与独立 review 通过。

## Files And Steps

1. 提取纯模块包 codec，并用最小 Node 测试固定信任边界和最大合法包往返。
2. 让 Preferences 和所有 reader runtime 拼接入口复用新 module，并按模块隔离预览 timer。
3. 更新阅读内核、代码地图、移动 UI、路线图与当前指针，完成检查和 review。

## Checks

- `node --test reader/web/style-module-package.test.mjs` 与 reader modules / bundle `node --check`；
- `pnpm --dir reader/app check`、`pnpm --dir reader/app build`；
- Rust fmt / check、AutoCorrect、文档 gate 与 `git diff --check`。

## Rollback

把 codec 逻辑内联回 `preferences.mjs` 并从各 runtime 列表移除新 module。没有依赖、数据或网络配置需要迁移。

## Approval

用户明确要求暂不真正实现 GitHub CSS 社区，只留下模块化接口。

## Result

`style-module-package.mjs` 现在独立拥有 schema 1 模块包的解析、序列化、6,400,000-byte 包输入、32 模块、单模块 32 KiB、启用组合 64 KiB、重复 ID、精确字段和注入式 CSS 校验。Preferences 的导入、导出、状态恢复与 32 模块 benchmark 复用该 codec；四个 runtime 拼接入口都在 Preferences 前交付新 module。没有 GitHub、登录、网络、社区入口、provider registry、数据迁移或依赖变化。

真壳首次回归暴露全局 180 ms timer 与手动保存重复排队，导致持久化失败状态先被旧任务触发、当前 textarea 尚未回滚。预览任务现按模块 ID 保存，手动保存只取消同模块任务；删除、导入和重置清理失效任务，因此快速切换模块不会丢另一模块草稿。

## Review

- Blocking: 初审发现包上限未覆盖 JSON 最坏转义、全局 timer 会跨模块丢草稿；两项均已修复，最终 Spec / Standards 复核为零发现。
- Non-blocking: 无。
- Out-of-scope: 完整 GitHub 社区按用户决定暂缓。

## Evidence And Residual Risks

- Node codec 测试 3 / 3：普通往返、32 个 32768-byte 停用模块往返，以及未知字段、重复 ID、单模块越界、不安全 CSS 和包越界拒绝均通过；
- Linux Tauri / WebKitGTK 0.55.1：模块导入、真实 CodeMirror、无效 CSS 与持久化失败回滚、重启恢复、桌面 / 600 px 截图及 32 模块 benchmark 通过；62445 bytes 组合 P95 为 3 ms；
- workspace Rust fmt / Clippy / tests、Svelte check / build 已由正式 Linux gate 通过；reader runtime gzip 为 49.22 KiB，相比提取前 48.63 KiB 增加约 0.59 KiB；
- 完整 GitHub 社区按用户决定暂缓；本次没有远程或 Android 证据，也不需要网络验收。

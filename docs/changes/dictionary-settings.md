---
description: 词典面板的设置入口、来源选择与独立字号持久化。
---

# 词典设置

## Status

implemented

## Problem

当前词典面板把导入、删除和来源选择直接堆在结果页，没有明确的设置入口，也不能调整或保存词典释义字号。用户已明确要求参考 Readest 增加设置。

## Scope

- 参考固定 Readest 源码和本地 `RD-24` 原图，在词典结果标题栏增加齿轮入口，并在同一浮层 / 抽屉内打开设置子页；
- 把当前词典选择、导入与移除收进设置页，结果页只保留当前来源标识；
- 复用 Readest 的 85%、100%、115%、130%、150% 和 175% 词典字号档位，只缩放释义，不跟随或改写正文设置；
- 以本地 schema 1 设置保存当前词典和字号；设置损坏、来源已移除或存储不可用时安全回退。

## Non-Goals

- 不增加多词典并发查询、拖动排序、启停 provider、系统词典、发音、网络搜索或同步；
- 不改变后端词典格式、查词、富文本净化、资源与隐私边界；
- 不新增设置框架、状态库或依赖。

## Architecture Impact

none

本变更只在既有 Svelte 词典面板和浏览器本地存储边界增加小型设置投影，不改变后端接口或模块所有权。

## Acceptance Criteria

- [x] 结果页始终有可访问的设置按钮；设置页可返回结果并可通过现有关闭方式退出；
- [x] 当前词典在设置页可选，导入与移除仍可用，结果页显示实际来源；
- [x] 六档词典字号立即作用于释义，并在重载后恢复；
- [x] 无效设置、已删除词典和存储异常不会阻止查词；
- [x] 桌面与移动深浅主题下无溢出、遮挡或滚动陷阱；
- [x] Svelte、设置单测、词典公共门、文档与 diff 检查通过。

## Files And Steps

1. 在 `dictionary.ts` 增加有界设置解析与写入，并用一个 Node 测试固定回退行为。
2. 将现有面板整理为结果 / 设置两页，复用已有设置控件和图标样式。
3. 以 CSS 变量应用词典字号，补齐响应式布局和事实文档。
4. 运行正式检查、真实渲染交互与独立 review。

## Checks

- `mise exec -- node --test reader/app/tests/dictionary.test.ts`；
- `mise exec -- pnpm --dir reader/app check`；
- `mise exec -- pnpm --dir reader/app build`；
- `bash scripts/check-dictionary-source.sh`；
- `bash scripts/check-docs.sh`；
- `git diff --check`；
- `agent-browser` 桌面和移动深浅主题设置交互。

## Approval

用户于 2026-08-13 明确要求“词典现在太简陋，在于没有设置。参考 readest 增加设置。”，批准本文件限定的词典设置范围。

## Result

词典结果页现在以齿轮进入同一浮层 / 抽屉的“词典设置”子页，返回、Escape、遮罩和原关闭入口保持既有行为。当前词典、导入、移除与 85%–175% 六档释义字号集中在设置页，结果末尾显示实际来源；字号通过受控 CSS 变量只作用于释义。schema 1 本地设置保存当前 64 位词典 ID 与允许字号，重载恢复；损坏设置、已移除来源和存储写入失败都有安全回退。没有增加依赖或后端接口。

## Review

独立规格复核未发现阻塞问题：设置入口、来源管理、六档独立字号、重载恢复、失效来源回退和移动布局均符合验收条件。代码标准复核发现的 `Architecture Impact` 协议值和存储警告遮蔽操作状态问题已修复；操作状态与持久化警告现在分别显示。残余边界与本地证据一致，仍需 Linux Tauri / WebKitGTK 或 PCT-AL10 真机覆盖原生导入选择器、移除确认、触摸滚动与重载恢复。

## Evidence And Residual Risks

`mise exec -- node --test reader/app/tests/dictionary.test.ts`、`mise exec -- pnpm --dir reader/app check`、`build`、`bash scripts/check-dictionary-source.sh` 和 `git diff --check` 已通过。公共词典门共通过 2 个选区 / 分派测试、3 个集成测试和 10 个后端单测，3 个私有 / 真机测试按设计忽略。

`agent-browser` 以合成 Tauri IPC 驱动真实 Svelte 组件，验证 1280 × 900 桌面和 390 × 844 移动视口。当前词典从 MDict 切换到 Kindle 后立即重新查词并写入来源；字号从 100% 切到 150% 后计算值为 24 px，重载后仍恢复。把已保存来源改为不存在的合法 ID 后，面板回退首个可用词典、重写设置并保留 175% 字号，计算值为 28 px。移动面板边界为 390 × 633 px，无页面或内容横向溢出。合成阅读入口自身会报告既有 `invalid-state-key` / `active-content` 启动错误，因此本证据只覆盖词典组件，不声明阅读会话控制台干净。

截图位于 `artifacts/local/audits/dictionary-settings/`：桌面设置浅色、桌面设置深色、移动设置深色和移动 150% 结果的 SHA-256 分别为 `6817ad13d26db10cc34324000877030e2e44bf3f2b8a3b5b14975d66ee39402a`、`a25b9893b715db40707896893d670ab87ecd72a5454984f241e76d1048fb57d8`、`9a27f43512384f2514cc8b89f99b3cfc04bae27652e12d25c975eadd66536fd4` 和 `14d765a9a708325c6bb824e28a69cf6415648c94b6fbacd195f24228c8c612c3`。

最高证据是本地构建、公共后端门和合成浏览器组件，不是 Linux Tauri / WebKitGTK 或 PCT-AL10 上的真实词典设置验收。Readest 只作为齿轮入口与字号档位参照；Atha 仍保持单一当前本地词典，不增加其多 provider、网络、系统词典或同步能力。

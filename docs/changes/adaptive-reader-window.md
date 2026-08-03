---
description: 让阅读窗口可自由缩放和最大化，并移除会破坏正文安全区的页边距设置。
---

# 自适应阅读窗口

## Status

implemented

## Problem

当前阅读页固定为 780 × 1680 设备像素，Tauri 窗口只能围绕这张固定画布取初始尺寸。窗口放大后书页不会利用可用空间，无法自然最大化；偏好面板又允许分别修改四边距，用户可以把正文移入工具栏覆盖区或压缩到不可读。

## Scope

- Tauri 阅读窗口允许自由缩放和最大化，初始尺寸使用约 430 × 820 逻辑像素，最小内部尺寸为 360 × 640 逻辑像素；
- 阅读页始终填充 WebView 可用区域，并按 `devicePixelRatio` 换算内部设备像素尺寸，字号、行距、固定四边距和书籍内容继续使用设备像素；
- 窗口尺寸变化经现有 Navigation 队列捕获 Locator、等待停止拖动、重排并恢复当前位置，不引入第二套布局状态；
- 四边距固定为当前移动阅读基线的上 144、右 32、下 144、左 32 设备像素，从设置 UI、应用偏好和持久化输出中移除；
- 旧应用偏好中的四个边距字段在恢复时忽略，其余有效设置继续保留；
- 更新真实浏览器尺寸回归、Tauri 窗口测试和当前事实文档。

## Non-Goals

- 不设计桌面双栏、横屏专用工具栏、可调内容宽度或新的大屏信息架构；
- 不改变字号、行距、主题、亮度、翻页行为和本书样式设置；
- 不增加窗口状态持久化、断点系统、动画库或布局依赖；
- 不改变 EPUB、Locator、搜索、标注和受控资源边界。

## Acceptance Criteria

- [x] Tauri 窗口可拖动调整、可最大化，且不能缩小到 360 × 640 逻辑像素以下；
- [x] 390 × 840、780 × 1680 和 960 × 720 CSS 像素视口下，阅读页均填满可用区域，内部尺寸等于视口 CSS 尺寸乘 DPR；
- [x] 窗口调整后当前 Locator 仍可见，页数和进度刷新，正文无裁切，工具栏只覆盖固定页眉页脚安全区；
- [x] 字体、布局、主题、行为和本书样式设置中均没有四边距输入，应用偏好也不再保存四边距；
- [x] 带旧边距字段的有效偏好可迁移，其余设置不丢失；
- [ ] Tauri/Svelte、正式 reader samples、窗口单元测试和文档 gate 通过，独立 review 无 blocking。

## Files And Steps

1. 调整共享 Windows 初始/最小窗口尺寸，并在 Tauri 与 Wry 基线中启用相同最小尺寸；
2. 把 Pagination 的固定画布尺寸改为当前视口设备像素，并通过 Navigation 串行处理窗口重排；
3. 删除四边距设置 DOM、偏好字段和写入逻辑，保留固定 CSS 排版值与旧记录迁移；
4. 更新诊断、正式浏览器尺寸检查、架构、ADR 和代码地图。

## Checks

- `pnpm --dir reader/app check`
- `pnpm --dir reader/app build`
- `cargo test --locked --workspace`
- `pwsh -NoProfile -File scripts/check-reader-samples.ps1`
- `pwsh -NoProfile -File scripts/check-tauri-reader.ps1`
- `pwsh -NoProfile -File scripts/Invoke-Atha.ps1 check docs -Activity validation -Scope project-workflow`
- Agent Browser：390 × 840、DPR 2 与 960 × 720、DPR 1 的尺寸变化、正文安全区和控制台错误。

## Rollback

按本次提交整体回退；偏好迁移只忽略旧字段，不写入不可逆数据。

## Approval

2026-08-03：用户要求移除四边距设置，并让应用窗口可自由调整、最大化且具有最小尺寸。

## Result

- Tauri 与保留的 Wry 基线统一使用 430 × 820 逻辑像素初始尺寸、360 × 640 最小内部尺寸，并显式允许调整和最大化；
- Pagination 根据当前视口与 DPR 设置内部设备像素画布，窗口变化等待 120ms 后通过既有 Navigation 队列保存 Locator、重排并恢复位置；
- 四边距设置已从 Svelte、兼容 HTML、应用偏好、样式写入和持久化输出删除，固定值只由阅读页 CSS 拥有；
- 旧应用偏好中的四个边距字段恢复时会被丢弃，其余有效设置保持；
- 没有增加依赖、第二套布局状态、桌面双栏、横屏专用布局或窗口状态持久化。

## Review

- Blocking：待独立 review。
- Non-blocking：待独立 review。
- Out-of-scope：桌面双栏、横屏专用布局和窗口状态持久化。

## Evidence And Residual Risks

- 静态证据：Svelte 检查、Vite production build、五份 reader module 的 `node --check`、PowerShell parser、Rust fmt 和 `git diff --check` 通过；Rust workspace 11 个实际测试通过。
- 本地浏览器证据：四套正式 reader samples 的真实 host、明暗主题、偏好迁移与完整阅读交互通过；新增的 390 × 840、DPR 2，960 × 720、DPR 1，以及 780 × 1680、DPR 1 动态重排检查均通过，内部尺寸、固定边距和工具栏安全区符合预期。
- 真实目标证据：当前 Windows 的 Tauri 产品入口可最大化，`WM_GETMINMAXINFO` 返回的原生最小拖动尺寸不低于 360 × 640；EPUB 导入 smoke 通过。
- 性能证据：Tauri 基准 `1785772247252-12508` 在固定 780 × 1680 内部基线上的 10 样本 P95 为冷启动 720.284ms、首稳 160.800ms、热开 28.000ms、翻页 6.800ms、字号重排 48.700ms，均低于既有门槛。
- 残余风险：没有生产安装包、跨设备或长期真实阅读证据；最大化横屏仍使用单列分页，桌面信息架构和双栏明确不在本次范围。

---
description: 修复纯图片页书签无法识别当前位置并在重复点击时使阅读会话失败的问题。
---

# 纯图片页书签切换

## Status

accepted

## Problem

《唯物主义》封面是没有正文文本的图片页。首次点击右上角书签后，持久状态和目录已经新增书签，但按钮仍显示未添加；再次点击会把同一 Locator 当作新书签，随后因去重结果与 UI 假设冲突触发 `sample-boundary`，阅读页进入失败状态。

## Scope

- 在共享书签模块正确识别纯图片页当前位置已有的书签；
- 保持现有 Locator、书签持久化、目录投影和文字页行为不变；
- 增加纯图片页的创建、按钮状态和再次点击删除回归；
- 用《数学及其历史》和《唯物主义》复核真实 Tauri/WebView2 入口。

## Non-Goals

- 不修改分页、Locator schema、书签数据格式或数量上限；
- 不扩展 EPUB 格式兼容性，也不新增 UI；
- 不处理未复现的阅读器功能。

## Acceptance Criteria

- [x] 纯图片页首次点击书签后按钮立即显示已添加，目录只出现一条书签；
- [x] 同一位置再次点击会删除书签，阅读页继续保持 `pass`；
- [x] 文字页既有书签创建、去重、跳转和删除回归继续通过；
- [x] 两本真实 EPUB 的 Tauri/WebView2 打开与正式检查无 blocking。

## Files And Steps

1. 用纯图片页固定失败场景；
2. 在 `reader/web/bookmarks.mjs` 的共享当前位置判断中做最小修复；
3. 运行阅读器样本、两本真实 EPUB、文档和差异检查；
4. 独立复核范围、实现和回归证据后关闭 change。

## Checks

- `pwsh -NoProfile -File scripts/check-reader-samples.ps1`
- `pwsh -NoProfile -File scripts/check-tauri-reader.ps1`
- `pwsh -NoProfile -File scripts/check-tauri-reader.ps1 -Epub 'fixtures/local/唯物主义 (2023).epub' -ExpectedTitle '唯物主义'`
- `pwsh -NoProfile -File scripts/Invoke-Atha.ps1 check docs -Activity validation -Scope project-workflow`
- `autocorrect --fix` 与 `autocorrect --lint` 仅处理本次中文 Markdown
- `git diff --check`

## Approval

2026-08-06：用户批准开始真实阅读稳定化，并授权只修复两本真实 EPUB 验收中实际复现的问题。

## Result

`bookmarks.mjs` 现在先用当前 Locator 精确识别已有书签，再保留原有的可见文字偏移判断。纯图片页或只有不可见字符的页面即使没有可定位字符矩形，也能在同一位置正确切换书签；文字页仍可在书签所在页面显示已添加状态。

## Review

待实现后独立复核。

## Evidence And Residual Risks

- 复现证据：修复前《唯物主义》封面首次添加后目录已有一条书签但按钮 `aria-pressed=false`，第二次点击后页面状态变为 `fail`、错误为 `sample-boundary`。
- 真实浏览器：四个既有样本的明暗主题、目录、文字页书签、选择、标注、搜索、持久化和截图 runner 全部通过。
- 真实 Tauri/WebView2：修复后用独立 `WEBVIEW2_USER_DATA_FOLDER` 打开《唯物主义》，确认初始 DOM 存储为空；封面首次点击得到一条书签且按钮点亮，第二次点击删除后为零条，页面保持 `pass`。两本指定真实 EPUB 的完整 Tauri 检查均通过。
- 环境说明：首次探针只修改了 `LOCALAPPDATA`，没有隔离显式 WebView2 UDF，因而误写入一条测试书签；已按本次生成的 UUID 精确删除并确认不存在，未修改其他书签或偏好。后续探针依据 WebView2 官方覆盖变量改用独立 UDF，并在进程退出后删除。
- 剩余风险：Rust 检查继续报告 Windows incremental 目录无法收尾，但未影响构建、测试或运行结果。

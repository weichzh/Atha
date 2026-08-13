---
description: 桌面阅读工作区的范围、验收、实现与关闭证据。
---

# 桌面阅读工作区

Status: accepted

## Problem

当前目录、书内搜索和消息投影在所有视口都使用移动底部面板；宽屏上打开工具会覆盖正文，切换工具也会打断阅读。现有 ReaderManifest、Locator、目录、搜索、Message 投影和分页内核已经提供所需事实，桌面端缺的是一个与书页并存的稳定工作区。

## Scope

- 在至少 `1100px` 的宽视口启用左侧工作区：目录、书内搜索与笔记 / Message 投影继续使用现有 panel 和 store，同一时刻只打开一项；
- 工作区打开时书页占用剩余宽度，分页以真实阅读 frame 尺寸重排，并使用当前 Locator 恢复同一正文位置；
- 宽屏默认打开目录，鼠标可切换三个工具；`Ctrl/Cmd+F` 打开书内搜索并聚焦输入，工具标签支持左右方向键、`Home` 和 `End`，`Escape` 把焦点还给书页；
- 保留已有方向键、`PageUp` / `PageDown` 和空格翻页，工具内的输入、列表与按钮仍阻断书页快捷键；
- 低于宽屏阈值时保留当前沉浸式工具栏和底部面板，不分叉移动状态、内核或数据。

## Non-goals

- 不新建桌面专用阅读状态、搜索索引、Message 视图或 Locator；
- 不做多侧栏同时打开、拖拽改变宽度、脱离窗口、多窗口或侧栏偏好持久化；
- 不重设对话编辑器、词典、设置、进度或移动抽屉；
- 不把 Linux WebKitGTK 结果称为 Windows WebView2、Android 或 PCT-AL10 验收。

## Acceptance

- `DESK-LAYOUT-01`：Linux Tauri 真壳在 `1280x800` 和 `1600x900` 打开书籍；目录工作区默认可见，书页位于其右侧，两者无重叠、裁切、横向溢出或布局错误。
- `DESK-TOOLS-01`：依次用鼠标切换目录、搜索和笔记；每次只有一个工作区 panel 打开，目录可导航，书内搜索可返回并跳转结果，笔记继续投影当前 MessageStore 并可打开对话。
- `DESK-KEYS-01`：宽屏书页获得焦点；`Ctrl+F` 打开搜索并聚焦查询框，左右方向键在工具标签间移动，`Escape` 恢复书页焦点；在非控件焦点下 `PageDown` 仍导航到下一页。
- `DESK-LOCATOR-01`：记录当前章节与可见 Locator；切换工具、在宽屏尺寸间重排并返回；当前章节不变，Locator 仍可解析且正文无终止错误。
- `DESK-NARROW-01`：在 `600x760` 重复打开目录、搜索和笔记；工作区属性不存在，原有底部面板、中心轻点收起和移动尺寸保持不变。
- `DESK-PRIVACY-01`：桌面工作区切换、搜索和 Message 导航后，console 无错误，AppLog 不包含书名、查询、正文、消息或本地路径。

## Files And Steps

1. 让分页尺寸以真实 reader frame 为准，保留全屏时的等价行为与现有 Locator 重排链路。
2. 在现有 `details` panel 上增加宽屏工作区绑定、默认目录、单 panel 状态和最小键盘导航。
3. 使用 CSS 将现有目录、搜索和笔记 panel 投影为左侧工作区；窄屏规则保持原样。
4. 扩充 Linux 真壳门覆盖宽屏工作区、工具行为、键盘、重排、窄屏回归和隐私，更新事实所有者。

## Checks

- `pnpm --dir reader/app check && pnpm --dir reader/app build`；
- `node reader/web/conversations.test.mjs`；
- `bash scripts/check-reader-linux.sh`；
- `autocorrect --fix/--lint` 仅针对本次中文 Markdown；
- `project_workflow.py station <task> --activity verification --gate docs`。

## Result

实施后回填。

## Review

候选提交前执行 Standards 与 Spec 两轴独立审查并回填。

## Evidence And Residual Risks

实施后回填；至少区分 Svelte / Node 静态与单元证据、Linux Tauri 真壳证据，以及未执行的 Windows / Android 真实目标边界。

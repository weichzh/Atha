---
description: 阅读器图片、设置、笔记与词典入口的交互修正和验收证据。
---

# 阅读界面导航修正

## Status

implemented

## Problem

图片全屏预览只能依赖右上角按钮退出；资料库把备份、恢复和存储操作塞在搜索框旁的三点菜单里，且没有稳定的应用设置入口或独立应用主题；移动端笔记返回图标不可靠，低频导出长期占用标题栏；词典又以常驻顶栏按钮出现，而真实查词动作已经来自选中文字。这些入口没有按使用频率、作用域和任务上下文组织。

## Scope

- 图片全屏预览点击图片外的黑色区域立即返回阅读，图片、缩放控件和其他内容预览保持原行为；
- 资料库增加一级“设置”入口，搜索框只负责搜索；独立的跟随系统 / 浅色 / 深色应用主题、全书阅读默认恢复、备份、恢复、存储占用和离线词典管理进入设置页；
- 应用主题只改变书架、阅读记忆和应用设置，不复用或改写阅读页主题；阅读排版和本书覆盖继续只在阅读界面调整；
- 阅读顶栏使用明确的设置图标，不再显示常驻词典按钮；选中文字后的“查词”继续打开现有离线词典结果；
- 笔记全屏页使用应用现有图标库提供明确返回按钮，导出收进右上角更多操作；
- 更新受影响的正式 UI 检查和当前事实所有者。

## Non-goals

- 不改变词典格式、查询、安全净化、阅读偏好 schema、备份格式或既有阅读状态语义；
- 不新增网络词典、同步、账号、通用路由框架或把阅读面板复制到应用设置；
- 不借本次调整重做书架、阅读记忆、笔记内容或阅读布局视觉系统。

## Architecture Impact

- 新增独立 schema 1 `atha.app.appearance.v1` 本地记录，只保存本机应用主题；它不进入资料库备份，阅读偏好继续使用原记录。

## Acceptance

- `NAV-IMAGE-01`：打开普通图片后，点击图片外黑色区域关闭预览并回到原阅读位置；点击图片和缩放控件不会误关闭。
- `NAV-SETTINGS-01`：资料库一级导航可进入设置页，应用主题、阅读默认恢复、备份、恢复、存储占用和词典管理均可访问；书架搜索栏旁不再出现管理菜单。
- `NAV-SETTINGS-02`：应用主题可在跟随系统、浅色和深色间切换并恢复，改变应用主题不会改写阅读主题记录；移动和桌面设置页均无横向溢出。
- `NAV-NOTES-01`：窄屏打开笔记后可用可见返回按钮回到阅读；标题栏只保留返回、标题和更多操作，导出位于更多菜单内。
- `NAV-DICTIONARY-01`：阅读顶栏不显示词典按钮；选中文字并点击“查词”仍打开现有词典结果，设置页可导入、选择、调整字号和移除词典。
- `NAV-REGRESSION-01`：受影响桌面与移动视口无横向溢出、遮挡或控制台错误，既有阅读设置、书签、目录、搜索、进度和本地资料操作保持可用。

## Files And Steps

1. 复用原生 `dialog` 和现有词典、资料库操作，只调整触发入口与关闭行为。
2. 参考 Readest 的设置作用域，增加独立应用主题；不把阅读排版或本书设置复制到应用设置页。
3. 将词典管理从查词结果组件移到资料库设置页，保留选区事件驱动的查词浮层。
4. 用现有 Lucide 图标和一个原生 `details` 菜单修正笔记标题栏。
5. 更新正式资料库、阅读器和词典检查，记录事实所有者与真实证据边界。

## Checks

- `mise exec -- pnpm --dir reader/app check`；
- `mise exec -- pnpm --dir reader/app build`；
- `mise exec -- node --test reader/app/tests/dictionary.test.ts reader/app/tests/library.test.ts`；
- `mise exec -- node --test reader/web/conversations.test.mjs`；
- 受影响正式 Linux Tauri / 浏览器入口；
- `autocorrect --fix/--lint` 仅针对本次中文 Markdown；
- `project_workflow.py station reader-interface-navigation --activity verification --gate docs`。

## Result

- 图片预览只在普通图片模式点击视口黑边时关闭，并通过既有 dialog close 路径恢复阅读焦点；图片、缩放控件、表格和其他预览不误关。
- 资料库搜索旁的管理菜单已移除，一级设置页提供独立应用主题、全书阅读默认恢复、离线词典管理和本地资料维护。应用主题与阅读主题使用不同记录；恢复阅读默认不改变应用主题和本书覆盖。
- 阅读顶栏只保留返回、书签和阅读设置；词典由选区“查词”按上下文打开。笔记页有明确返回按钮，导出收进更多菜单。
- Svelte 产品入口提供选区查词；保留的静态阅读入口没有词典面板，共享标注模块会跳过不存在的查词控件，不影响复制、标注和笔记绑定。

## Review

首轮 Spec 审查发现静态阅读入口没有查词控件，而共享标注模块无条件绑定该控件；已改为可选绑定，避免 Windows 静态入口在初始化时中断。首轮 Standards 审查发现应用主题记录解析前没有长度限制；已增加 1024 字符上限和超限回退测试。两项修复后的 Standards 与 Spec 复审均为零 finding。

## Evidence And Residual Risks

- 静态 / 本地：`svelte-check` 为 0 error / 0 warning；词典、资料库和对话共 13 项 Node 测试通过；PowerShell / Bash 脚本语法、Vite production build 与 `git diff --check` 通过。
- 本地 Chromium：360 × 800 与 1280 × 900 的浅色应用设置页无横向溢出，主题切换和持久化通过；截图位于 `artifacts/local/audits/reader-interface-navigation/`。
- 真实目标：`scripts/check-reader-linux.sh` 在 Linux Tauri / WebKitGTK 0.55.1 通过，覆盖图片黑边退出与焦点、笔记返回 / 导出菜单、顶栏词典移除、选区查词、应用主题与阅读默认作用域、360 / 1000 / 1280 / 1600 宽度回归及 AppLog 隐私。
- 未覆盖：本次未在 Windows WebView2、Android 模拟器或 PCT-AL10 重跑交互；PowerShell 全样本门在 Linux 无法执行 Windows `atha-reader-host.exe`，因此不把脚本静态解析和 Linux Tauri 结果表述为这些平台验收。

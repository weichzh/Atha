---
description: 统一移动阅读壳主题、正文安全区、目录、进度和分层设置的实施记录。
---

# 移动阅读壳可用性升级

## Status

implemented

## Problem

当前移动阅读壳存在四类直接影响使用的问题：部分颜色绕过主题变量；顶部和底部工具栏会遮挡正文；目录不是填充页面且导航后仍停留在目录；进度与更多菜单的信息和层级过于简陋。界面需要继续贴近微信读书的移动竖屏交互，同时保留 Atha 固定设备像素书页和遵循系统缩放的壳层控件。

## Scope

- 统一壳层和书页的语义颜色变量，支持系统、浅色、纸张和深色主题；
- 把固定书页的上下正文安全区调整到任何受支持系统缩放下都不会被 48 CSS px 工具栏覆盖；
- 目录填充阅读视口，章节与书签继续共用目录，导航成功后返回沉浸阅读；
- 补充全书近似进度、当前章节和本节页码，不预加载全书布局；
- 右上角更多入口先显示列表，再进入字体、布局、主题和行为设置；
- 保持 Tauri、Svelte 与既有无框架 reader kernel 的单 WebView 边界。

## Non-Goals

- 不实现听书、统计、阅读时长、AI、插件、同步或远程资源；
- 不增加 UI 组件库、状态框架、动画框架或第二份阅读内核；
- 不做桌面横屏布局、纸张纹理、阅读标尺或全书预分页；
- 不改变 EPUB、Locator、标注和搜索的既有信任边界。

## Acceptance Criteria

- [x] 系统、浅色、纸张和深色主题下，壳层、目录、进度、设置、原生表单和错误状态均使用同一组语义令牌；
- [x] 在 390 × 840 CSS px、DPR 2 及 780 × 1680、DPR 1 下，工具栏只覆盖页眉页脚，不覆盖正文，显隐工具层不触发正文重排；
- [x] 目录填充视口，书签仍位于对应章节之后；点击章节或书签后完成导航并关闭目录和工具层；
- [x] 进度面板显示当前章节、全书近似百分比、本节页码，并能通过滑块跳转而不预布局其他章节；
- [x] 更多菜单以列表进入字体、布局、主题和行为设置，现有偏好持久化、重排与恢复继续有效；
- [x] Tauri/Svelte 正式检查、阅读器正式检查、Agent Browser 明暗主题与关键交互验收通过；
- [x] `design-qa.md` 最终结果为 `passed`，独立 review 无 blocking 问题。

## Files And Steps

1. 在 `reader/atha-reader.css` 和 `reader/app/src/shell.css` 统一主题令牌与 48px 工具栏几何，调整默认上下安全区；
2. 复用隐藏的原生 TOC 数据源投影可点击目录列表，并在现有 Navigation 队列稳定后返回阅读；
3. 用现有 section 与 pagination 状态计算 O(1) 的全书近似进度；
4. 用现有 DOM 状态和控件拆分更多菜单及四类设置，不增加状态依赖；
5. 同步兼容 HTML、诊断、代码地图和架构事实，完成真实浏览器与正式 gate。

## Checks

- `pnpm --dir reader/app check`
- `pnpm --dir reader/app build`
- `pwsh -NoProfile -File scripts/check-tauri-reader.ps1`
- `pwsh -NoProfile -File scripts/check-reader-samples.ps1`
- `pwsh -NoProfile -File scripts/Invoke-Atha.ps1 check docs -Activity validation -Scope project-workflow`
- Agent Browser：390 × 840、DPR 2 的沉浸态、工具态、目录导航、进度、分层设置及明暗/纸张主题。

## Rollback

按本次提交整体回退即可；偏好解析继续为旧记录补默认值，不执行不可逆数据迁移。

## Approval

2026-08-03：用户审核并批准既定方案后要求开始实施。

## Result

- 将阅读壳颜色集中到四套主题的语义令牌，工具栏固定为 48 CSS px，书页默认上下安全区固定为 144 设备 px；
- 复用隐藏 TOC 作为单一数据源投影全屏目录，章节和书签导航成功后关闭工具层并把焦点交还阅读页；
- 进度面板使用 0–1 连续值映射已有 section 和当前分页，显示近似全书百分比、当前章节和本节页码，不预布局其他章节；
- 右上角更多菜单按字体、布局、主题与亮度、阅读行为和本书样式分层，兼容 HTML 与 Svelte 正式入口保持同一 DOM 契约；
- 没有新增依赖、状态框架、全书统计、自动翻页或第二套阅读内核。

## Review

- Blocking：独立规格与标准审查先后发现进度显示/跳转公式不互逆、整数刻度量化跨章节和浮点章节边界三项问题；改为 0–1 连续范围、统一换算并对边界使用机器精度容差后，补充单章首页、173 章 30 页中页、首章末页和后续章节末页断言，最终复核无 blocking。
- Non-blocking：审查发现导航失败仍关闭目录、设置下钻焦点遗留和兼容 HTML 仍为平铺表单；均已修复，最终复核无剩余 non-blocking。
- Out-of-scope：听书、桌面横屏、重型 Readest 功能和专项性能压榨。

## Evidence And Residual Risks

- 静态证据：`svelte-check`、Vite production build、模块语法检查和 `git diff --check` 通过；所有非令牌颜色均已从壳层 CSS 清除。
- 本地证据：四套 reader samples 的 Rust、WebView2 host、明暗主题、目录/书签、进度往返、偏好持久化、搜索与标注正式链路通过。
- 真实目标证据：当前 Windows 的 production Svelte build 在 Agent Browser 中以 390 × 840、DPR 2 验证，书页正文为 y=72–768，工具栏为 y=0–48 与 y=792–840，无横向溢出或控制台错误；同图对照记录在 `design-qa.md`，最终为 `passed`。
- 性能证据：Tauri 基准 `1785769614666-29748` 的 10 样本中位数/P95 为冷启动 563.409/575.663ms、首稳 133.200/142.900ms、热开 20.800/21.000ms、翻页 6.600/6.800ms、重排 27.800/41.700ms，均低于既有门槛。
- 残余风险：没有跨设备、生产安装包、桌面横屏或长期真实阅读证据；全书进度按 section 等权近似，不代表字数或阅读时长。

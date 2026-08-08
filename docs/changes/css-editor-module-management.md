---
description: 每书 CSS 可视编辑、实时预览与有界模块管理纵切。
---

# CSS 编辑器与模块管理

## Status

`implemented`

## Problem

Atha 现有每书样式只有一个普通 textarea、一次性“应用”按钮和总开关。它已经复用浏览器 CSSOM 做安全校验，但缺少编辑反馈、即时预览、可组合模块、排序、筛选、批量操作和可恢复导入导出，无法承载后续 GitHub PR CSS 社区。

## Research And Decision

- Tauri 产品壳采用 CodeMirror 6 的 CSS language、补全、搜索、撤销、括号匹配与 lint gutter；其模块化包远小于 Monaco 0.56.0 的约 98 MB unpacked 发布包。原生 host 验证 adapter 保留同一 textarea 数据入口作为渐进 fallback，不分叉状态或安全规则；
- CSS 的最终语法与信任边界继续只由现有 `CSSStyleSheet.replaceSync()` 和 `content.validateCss()` 决定；CodeMirror 只提供编辑反馈，不能绕过 `@import`、子资源、转义和 Shadow DOM 穿透限制；
- 每书状态直接扩展现有 book preferences：书源样式、页面边距、段首缩进、段距、模块总开关和有序模块列表。旧 `userStylesheet` 无损迁移为一个本地模块，不建立数据库、插件运行时或通用配置框架；
- 模块上限 32 个，新模块单个 32 KiB、启用组合 64 KiB；旧 `userStylesheet` 仍按原 32768 UTF-16 字符上限完整保留，超过 64 KiB 时以停用恢复副本呈现。名称、分组、ID 和 JSON schema 有界；增删改、排序、批量启停和导入先组合校验，失败恢复上次有效状态、渲染与 Locator；
- UI 固定 Readest `cf413b2b` 的源码原图、2026-08-08 公开 Web 截图和 PCT-AL10 微信读书真机原图；Readest `RD-03` / `RD-08` 提供安静图标工具与侧栏工作面，微信读书 `WR-06` / `WR-07` 提供主题色块、滑杆和分段排版。常用排版保持可视控件，原始 CSS 与模块 JSON 放在高级路径，不加入社交、音频、排行或 AI 入口。原图、逐图观察和哈希分别保存在 `fixtures/local/readest/` 与 `fixtures/local/weread/`。

## Scope

- 字号、字体、行距、段首缩进、段距、左右页边距、主题、亮度和翻页行为均使用原生选择、滑块、开关或分段控件并即时重排；
- CodeMirror CSS 编辑器提供语法高亮、行号、搜索、补全、括号匹配、撤销 / 重做、lint 与 180 ms 防抖实时预览；
- 模块支持新增、重命名、分组、启停、搜索、分组过滤、上下排序、删除、批量启停及 schema 1 JSON 导入导出；
- 旧状态迁移、每书持久化、重启恢复、无效模块回退、CSS 安全边界和 Linux Tauri / WebKitGTK 正式 GUI gate。

## Out Of Scope

- GitHub 登录、仓库、PR、审核、版本兼容和社区发现；这些属于下一 CSS 社区 change；
- 网络 CSS、字体上传、脚本模块、书籍 DOM 扩展 API、账户同步和跨书全局模块库；
- Monaco worker、Language Server、自研 CSS parser 或第二套渲染器；
- Android 模拟器日常验收；移动专项仍留到发布前。

## Architecture Impact

present

- Design purpose: 在既有每书 preferences 与 closed Shadow DOM 样式层上增加一个有界编辑和组合面，不改变 ReaderManifest、BookRoot、Locator、消息快照结构或渲染器，只提高消息快照 `user_css` 的字节上限。
- Drivers / quality scenarios: `A-CSS-01` 要求任一可视项或有效模块在 250 ms 内预览并保持 Locator；`A-CSS-SEC-01` 要求任何单模块或组合绕过安全规则时不写状态、不改变当前渲染；`A-CSS-PERF-01` 要求 32 个 / 64 KiB 上限组合与校验 P95 小于 50 ms。
- Modules / interfaces: `createPreferences` 继续拥有状态、迁移、组合和控件绑定；`createContent.setStyles` 继续拥有唯一 CSS 安全判定；CodeMirror 只同步既有 textarea；`createReaderState` 继续保存同一个 book record。
- Review trigger: 若 CodeMirror 使 reader 初始 gzip JS 增长超过 300 KiB、Linux 首次稳定布局回归超过 10%，或 WebKitGTK 无法稳定输入，则退回原生 textarea 加 CSSOM 诊断，不引入 Monaco。

## Acceptance Criteria

- [x] 现有旧 CSS 无损迁移为保留 ID 模块，超出 64 KiB 时停用但不丢弃；新建、编辑和 JSON 导入严格拒绝单模块越界、重复 ID、未知字段和不安全组合；
- [x] 可视排版与模块编辑均实时预览并保持阅读 Locator；无效输入定位到模块且保留上次有效组合；
- [x] 新增、编辑、分组、搜索、过滤、排序、删除、批量启停及 JSON 导入导出完整可用；
- [x] 常用设置使用分段控件、主题色块与开关；CSS 模块在宽屏为列表 / 编辑器双栏，在窄屏为保留正文上下文的底部面板，不再显示原生多行选择框和成排后台式按钮；
- [x] 同一 CSS 安全规则贯穿编辑、组合、保存、恢复、消息快照与渲染，没有平台分叉；
- [x] 32 模块 / 64 KiB 本地 benchmark P95 小于 50 ms，reader 初始 gzip JS 增量小于 300 KiB，Linux GUI 完成编辑、回退、持久恢复和非空截图；
- [x] Rust / Svelte / Node checks、AutoCorrect、required docs gate 与独立 Spec / Standards review 通过。

## Files And Steps

1. 扩展 preferences 数据契约、旧状态迁移、模块组合和失败回退，并先补可执行 self-check。
2. 接入 CodeMirror 渐进增强与可视排版、模块管理 DOM；静态 host 保留同一 textarea fallback。
3. 扩展 diagnostics 和现有 Linux FB2 GUI gate，记录组合 P95、bundle gzip 增量、交互、恢复和日志隐私。
4. 更新 reader 架构、代码地图、移动 UI、第三方声明、参考地图和路线图，完成独立 review、提交与 task closure。

## Checks

- `node --check` reader modules、reader bundle check 与 diagnostics self-check；
- `pnpm --dir reader/app check`、`pnpm --dir reader/app build` 及 bundle size 比较；
- `scripts/check-fb2-source.ps1 -VerifyLinuxGui` 的真实 Linux Tauri CSS 编辑、回退、恢复和截图；
- workspace fmt / Clippy / tests、AutoCorrect、`git diff --check`、required docs gate 与独立 review。

## Rollback

移除 CodeMirror 组件与新增 book preference 字段，保留旧 textarea、`userStylesheet` 迁移兼容和原有安全校验。ReaderManifest、BookRoot、Locator、消息数据库和书籍源文件均不迁移或改写。

## Approval

用户已批准按 Atha 路线图持续交付强 CSS 编辑器与模块管理，并明确要求性能和成熟度优先；本 change 是 Kindle 格式纵切关闭后的路线图 `Now`。

## Result

在现有 preferences / content / reader-state 链路内加入三档左右边距、段首缩进、段距和最多 32 个有序 CSS 模块。模块支持新增、编辑、分组、搜索、过滤、排序、删除、批量启停及 schema 1 JSON 导入导出；旧 `userStylesheet` 完整恢复为 `legacy-user-css`，超过新组合上限时保留为停用恢复副本。CodeMirror 6 只在模块页可见时按需加载，隐藏 textarea 继续作为同一状态入口；100 ms lint、180 ms 预览与所有写入最终仍由 `CSSStyleSheet.replaceSync()` / `content.setStyles()` 拒绝不安全组合。防抖草稿绑定输入时的模块 ID，持久化或重排失败统一恢复上次状态、渲染与 Locator。

界面按 Readest 与微信读书原图改为阅读器内的渐进设置面：字体、布局、主题和布尔项分别使用分段控件、色块、亮度滑杆和开关；宽屏 CSS 模块页同时显示筛选列表、当前模块与编辑器，窄屏收敛为底部面板。熟悉命令使用 Lucide 图标与 tooltip，批量状态只在模块列表区域出现。

消息快照的 `user_css` 上限同步为 64 KiB，恢复与备份验证使用同一字节上限。没有数据库迁移、第二套 parser、插件运行时或 Android 日常门。

## Review

独立 agent 以 `02d3b7e` 为基线完成三轮 Spec / Standards / UX 只读审查。首轮与复审发现的状态回滚、legacy 非 ASCII CSS、真实 CodeMirror 输入、窄屏门禁、防抖元数据、lint gutter、严格 JSON 导入及文档契约问题均已修复；最终复核为零发现。

## Evidence And Residual Risks

- Linux Tauri / WebKitGTK 0.55.1:2 个导入模块、真实 WebDriver CodeMirror 键入、lint gutter、180 ms 防抖元数据、无模块只读、无效子资源与持久化失败回退、搜索、列表投影、分段控件、开关尺寸、可视边距及重启恢复通过；宽屏与 600 px 窄屏截图分别为 1394 / 1493 色。32 模块、62445 bytes 唯一组合与 CSSOM 校验 P95 为 2 ms。
- 相对接受变更的基线，入口 JS gzip 从 12975 增至 16245 bytes，reader runtime gzip 从 44626 增至 48631 bytes；CodeMirror 保持 78.21 KiB gzip 按需 chunk，均低于 300 KiB 初始增量门槛。
- workspace Rust 测试、Svelte check / build、Node syntax、`git diff --check`、required docs gate 与最终独立 review 均通过；提交收据由 task closure 记录。
- Android ARM64 不在本切片证据范围内；发布前仍需移动专项验收。

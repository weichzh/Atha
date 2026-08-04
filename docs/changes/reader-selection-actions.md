---
description: 选中文字后的复制、标注、笔记动作与笔记列表跳转实施记录。
---

# 选中文字与笔记列表

## Status

implemented

## Problem

现有标注事实、重锚与跳转链路已经存在，但创建入口被放在底栏笔记面板中，既不贴合选中文字的上下文，也把“创建笔记”和“浏览笔记”混在一起。需要参考 Readest 的选择后就地动作与列表跳转，但不引入它的同步、复杂编辑器和平台桥接。

## Scope

- 选中 1–4096 个非空字符后，在选区附近显示复制、标注、笔记三个动作；
- 复制保留浏览器原生选择与剪贴板语义，不持久化或传输原文；
- 标注和笔记复用现有 Annotation Store、SourceAnchor、重锚与 CSS Custom Highlight；
- 笔记只使用一个聚焦的纯文本输入对话框，最长 2000 个字符；
- 底栏笔记面板只列出现有标注与笔记，点击列表项跳转并返回沉浸阅读。

## Non-Goals

- 不实现颜色、样式、编辑、删除、搜索、导出、同步或富文本；
- 不实现词典、翻译、分享、听书或可配置动作栏；
- 不替换系统选区手柄，不引入 Readest 的移动端桥接和自动翻页状态机；
- 不增加依赖、第二份标注状态或新的持久化 schema。

## Acceptance Criteria

- [x] 鼠标、触摸或键盘形成有效正文选区后，三个动作可见、可聚焦且不会触发翻页；无效或已处理选区不显示；
- [x] 复制得到选区原文，不创建记录，也不通过 IPC 或网络发送内容；
- [x] 标注创建 highlight，笔记经纯文本对话框创建 note；二者都立即投影并在重载、切章和重排后恢复；
- [x] 底栏笔记面板没有新增入口，只显示标注与笔记列表；点击任一项完成跨页或跨章跳转、关闭工具层并聚焦正文；
- [x] 现有损坏记录保护、长度限制、唯一原文重锚和软删除底层能力不回退；
- [x] Svelte 与兼容 HTML 保持同一 DOM 契约，正式阅读器检查和真实浏览器关键交互通过；
- [x] 独立 review 无 blocking 问题。

## Files And Steps

1. 在现有 Annotations 控制器中保留选区 Range，定位原生动作条并连接复制、标注和笔记创建；
2. 把笔记面板改为可点击列表，复用现有 `go` 导航并在成功后关闭工具层；
3. 同步 Svelte 壳与兼容 HTML 的最小 DOM/CSS，更新正式 Agent Browser 验收；
4. 更新阅读内核和代码地图，完成独立 review 与关闭证据。

## Checks

- `pnpm --dir reader/app check`
- `pnpm --dir reader/app build`
- `pwsh -NoProfile -File scripts/check-reader-samples.ps1`
- `pwsh -NoProfile -File scripts/check-tauri-reader.ps1`
- `pwsh -NoProfile -File scripts/Invoke-Atha.ps1 check docs -Activity validation -Scope project-workflow`

## Rollback

按本次提交整体回退；持久化 schema 不变，不执行数据迁移。

## Approval

2026-08-04：用户要求参考 Readest 开始实现复制、标注、笔记，并明确底栏笔记只负责列表与跳转。

## Result

- 复用现有 Annotation Store、SourceAnchor、CSS Custom Highlight 和 Locator 跳转，只把创建入口移到原生选区附近；
- 选择动作条固定为复制、标注和笔记，切章、重排、清空选区或打开阅读工具时撤销失效动作；
- 笔记使用一个必填纯文本 dialog；底栏面板只投影 highlight 与 note，点击记录直接跳转并返回沉浸阅读；
- 兼容 HTML 与 Svelte 壳保持同一 DOM 契约，没有新增依赖、持久化 schema 或第二份标注状态。

## Review

- Blocking：首轮规格评审发现失效选区未监听 `selectionchange`，触摸、键盘、焦点与无效选区缺少验收；补充选区撤销和浏览器自检后复核通过。
- Non-blocking：标准评审发现代码地图仍称工具开关“只切换”属性，并指出正式脚本重复三段拖选；同步事实并提取局部 helper 后复核通过。
- Out-of-scope：颜色、编辑、删除、搜索、导出、同步、词典、翻译、分享、听书和平台桥接。

## Evidence And Residual Risks

- 静态证据：`svelte-check`、Vite production build、两个 reader module 语法检查、PowerShell 解析和 `git diff --check` 通过。
- 本地证据：`scripts/check-reader-samples.ps1` 四样本正式回归通过；验证真实鼠标选区、三个动作的位置与焦点、受信任 copy 事件零写入、highlight/note 创建、重排与明暗重载恢复、两项列表跳转、工具层关闭、持久化和既有损坏记录保护；浏览器自检另覆盖 touch `pointerup`、键盘 `keyup` 和无效选区撤销。
- 真实目标证据：当前 Windows 的 WebView2 host 与 Agent Browser 明暗链路通过；选择动作截图为 `artifacts/local/screenshots/math-history-r1-light-selection-actions.png`，笔记列表截图为 `artifacts/local/screenshots/math-history-r1-light-annotation.png`。Tauri 产品检查通过真实 EPUB import、窗口行为和 production Svelte build。
- 性能证据：Tauri 基准 `1785818772540-28968` 的 10 样本中位数/P95 为冷启动 542.608/559.453ms、首稳 133.100/142.400ms、热开 20.700/20.900ms、翻页 6.700/10.000ms、重排 27.700/48.500ms，均低于既有门槛；未执行同时间旧代码对照，不能归因于本次改动。
- 残余风险：未在真实手机或触摸屏硬件上验收；安全策略不授予剪贴板读取权限，因此自动化以同一 Range 长度和受信任 copy 事件证明复制链路，没有读取系统剪贴板内容。若 WebView2 未来移除 `execCommand("copy")`，正式 copy 事件断言会失败并需要迁移。

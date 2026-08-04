---
description: 选中文字后的复制、标注、笔记动作与笔记列表跳转实施记录。
---

# 选中文字与笔记列表

## Status

accepted

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

- [ ] 鼠标、触摸或键盘形成有效正文选区后，三个动作可见、可聚焦且不会触发翻页；无效或已处理选区不显示；
- [ ] 复制得到选区原文，不创建记录，也不通过 IPC 或网络发送内容；
- [ ] 标注创建 highlight，笔记经纯文本对话框创建 note；二者都立即投影并在重载、切章和重排后恢复；
- [ ] 底栏笔记面板没有新增入口，只显示标注与笔记列表；点击任一项完成跨页或跨章跳转、关闭工具层并聚焦正文；
- [ ] 现有损坏记录保护、长度限制、唯一原文重锚和软删除底层能力不回退；
- [ ] Svelte 与兼容 HTML 保持同一 DOM 契约，正式阅读器检查和真实浏览器关键交互通过；
- [ ] 独立 review 无 blocking 问题。

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

待实现。

## Review

- Blocking：待评审。
- Non-blocking：待评审。
- Out-of-scope：待评审。

## Evidence And Residual Risks

待验证。

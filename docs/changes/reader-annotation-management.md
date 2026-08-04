---
description: 已有标注的正文命中、范围更新、笔记编辑删除与全屏笔记页实施记录。
---

# 标注与笔记管理闭环

## Status

accepted

## Problem

当前阅读器只能从新选区创建标注或笔记。已经保存的标注无法从正文重新选中、调整范围、编辑笔记或删除，笔记列表也仍是底部浮层，不能承载完整管理操作。WebView 的右键菜单还会暴露浏览器默认入口，破坏阅读器自己的交互边界。

Readest 的有效经验是：命中正文已有标注时恢复其选区；笔记列表正文负责跳转，编辑和删除使用独立动作。Atha 复用已有 SourceAnchor、锚点替换、软删除和纯文本对话框，不引入 Readest 的自绘选区手柄、颜色系统、同步或富文本编辑器。

## Scope

- 在整个阅读器 WebView 中禁止浏览器默认右键菜单；
- 点击或轻触正文已有标注时恢复原选区，显示复制、更新范围、笔记和删除动作；重叠标注命中最近更新的一条，其余记录仍可在笔记页管理；
- 范围修改复用浏览器原生选区手柄，并通过现有 SourceAnchor 与 `replaceAnchor` 保存；
- 同一个纯文本对话框同时负责新建笔记、为标注添加笔记和编辑已有笔记；
- 删除继续写入现有 tombstone，并立即移除正文投影和列表项；
- 笔记页改为与目录一致的全屏页面；点击项目正文跳转并回到阅读，编辑和删除动作不触发跳转。

## Non-Goals

- 不实现标注颜色、下划线、标签、搜索、导出、同步、撤销或批量管理；
- 不实现富文本、Markdown、自绘范围手柄、跨页拖拽或自动翻页；
- 不增加依赖、第二份标注状态或新的持久化 schema。

## Acceptance Criteria

- [ ] 阅读正文和壳层任意位置的右键操作都不会打开 WebView 默认菜单，也不触发翻页或工具层切换；
- [ ] 鼠标或触摸命中已有标注后恢复其正文范围，动作栏可复制、更新范围、添加或编辑笔记和删除；
- [ ] 更新范围后原记录身份和笔记保持不变，重排、切章和重载后投影到新范围；
- [ ] 编辑笔记时显示已有内容，保存后列表与正文状态立即更新；删除后记录成为 tombstone，正文投影和列表项消失；
- [ ] 笔记页填充整个视口，项目正文跳转后关闭工具层，项目编辑和删除不会误触发跳转；
- [ ] 新建选区、损坏记录保护、长度限制、唯一原文重锚和持久化能力不回退；
- [ ] Svelte 与兼容 HTML 保持同一 DOM 契约，正式阅读器检查和真实浏览器关键交互通过；
- [ ] 独立 review 无 blocking 问题。

## Files And Steps

1. 在现有 Annotations 控制器保存当前投影 Range，命中已有标注并复用选区动作栏连接范围更新、笔记编辑和软删除；
2. 把笔记列表项拆成跳转正文与独立编辑、删除动作，并让笔记页复用目录的全屏布局；
3. 在阅读器入口统一取消 `contextmenu` 默认行为，同步 Svelte 与兼容 HTML；
4. 扩展正式 Agent Browser 回归，更新阅读内核和代码地图后完成独立 review。

## Checks

- `pnpm --dir reader/app check`
- `pnpm --dir reader/app build`
- `pwsh -NoProfile -File scripts/check-reader-samples.ps1`
- `pwsh -NoProfile -File scripts/check-tauri-reader.ps1`
- `pwsh -NoProfile -File scripts/Invoke-Atha.ps1 check docs -Activity validation -Scope project-workflow`
- `autocorrect --fix` 与 `autocorrect --lint` 仅处理本次中文 Markdown
- `git diff --check`

## Rollback

按本次提交整体回退；持久化 schema 不变，不执行数据迁移，既有记录继续可由旧版本读取。

## Approval

2026-08-04：用户要求禁用 WebView 右键，参考 Readest 完成已有标注的选择、修改和删除，并让笔记页与目录一样全屏。

## Result

待实施。

## Review

待实施。

## Evidence And Residual Risks

待实施。

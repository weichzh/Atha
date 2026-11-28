---
description: 从阅读体验 V2 独立交付笔记气泡、紧凑输入栏与窄屏笔记面板。
---

# 笔记对话与编辑器 V2

## Status

implemented

## Problem

笔记线程用时间线和全宽卡片陈列回应，常驻排版工具挤占窄屏对话空间。直接移植本地 V2 会覆盖已合入的草稿保护、富文本首版和历史快照修复，也可能在选定对话读不到时将回复投给其他原文。

## Scope

按用户批准的未交付改动拆批范围，从主线独立交付原文后的 IM 气泡、按需排版的药丸输入栏、窄屏贴底的笔记与对话两页、缺失对话重读。保留 A/B/C 顺序、原文与历史快照入口、消息真实关系及输入模式。沿用 [Issue #1](https://github.com/weichzh/Atha/issues/1) 的阅读链路验收；本批不改 ReaderView、阅读引擎、Rust、IPC、快照资源和历史数据。

## Architecture Impact

none

## Acceptance Criteria

- 原文在先，回应为靠右的气泡；回复关系、跨消息引用、修订和操作入口可见，不引入虚构身份。当前对话发送成功后可看到新回复，切到章节或全书范围从起始处回看。
- 桌面紧凑可视化输入用 Enter 发送、Shift + Enter 换行；Markdown、展开编辑和粗指针环境的裸 Enter 继续用于换行。输入法组字键不发送，保留 Ctrl/Cmd + Enter 与发送按钮。
- 窄屏 B→C→B 使用同一贴底边界并还原列表滚动位置；输入栏及主要触控动作可达。显式选中对话缺失时不展示其他线程或输入器，重读只能找回原目标。
- 保持 #4 的草稿、修订、锚点和快照合同；复验浏览器真实输入、转场和已有集成路径。

## Files And Steps

1. 对照本地 V2 和主线，移植 NotesWorkspace、MessageComposer 的交互及局部样式；不重复交付主线已有的消息 JSON 解析、渲染和测试。
2. 扩充现有原创样本的 Playwright 场景，运行前端与文档检查并查看截图。

## Iteration Control

- Mode: `iterative`
- Maximum repair iterations: `3`
- Unchanged-delta limit: `1`
- Human stop: 需要改动其他批次所有者接口或写入真实书库或设备时停止受影响部分。

## Checks

- `pnpm --dir reader/app check`：Svelte 检查无错误、无警告；`pnpm --dir reader/app build` 通过。
- `pnpm --dir reader/app test`：154 项 Node 单测通过；`bash scripts/check-workflow.sh`、`git diff --check` 通过。
- 构建后运行 `node reader/app/tests/reader-app.browser.mjs`（Playwright 和 Chromium 来自本机现有缓存），由系统分配 loopback 端口。完整套件通过，覆盖 A｜B｜C、选区富文本首版、链接校验、发送期间继续写草稿、历史快照图片、阅读记忆刷新与后续章节锚点。
- 新场景在 1440 × 900 验证 Enter、Shift + Enter、组字键不发送、Markdown 保留空行、展开态换行、回复滚到视口内、模拟选中线程读回缺失与重读、聚合范围内回复指向所选线程及实际修订。在粗指针 360 × 760 检查 B→C→B 同边界及同数据滚动位置、可点目标、工具展开的换行布局和点击发送。

## Result

原文和跳转入口保持在线程最前；回应以靠右气泡陈列，保留回复 / 引用、时间与操作。首屏输入栏不再常驻排版工具。窄屏笔记列表和对话共享贴底面板；维持已有布局断点。只有成功发送当前线程的回复才滚到底部，既有线程入场和聚合范围从开头查看。读回失败时不切到别的线程；重读成功后进入原选中线程的“当前”。

- iteration 1：验证发现合成组字事件后的编辑器尾部空段落，以及保存改变笔记索引条目的排序和高度，不能用于比较相同列表滚动位置。组字断言对实际文本去除尾空白，B→C→B 放回同一列表数据验证；复核深色截图并提高引用标签对比度。新增缺失目标重读和消息修订场景。剩余差异：无本批阻塞项；证据等级：Linux Chromium 与本地检查；停止原因：当前候选达到交付范围。

## Review

本地复核源差异和最终候选。未复制原始 V2 对发送布尔契约、草稿身份、异步发送守卫或快照资源读取的回退；消息适配器、富文本解包、第一次修订的实现及单测留在主线。独立审查由父任务在 PR 阶段进行。

## Evidence And Residual Risks

受测基底为 `188df1a`。已查看浅色桌面、深色桌面及 360 px 默认 / 展开工具的实际截图。收据放在忽略目录 `artifacts/local/audits/conversation-editor-v2/`。

一次中间构建的完整套件记录到 `locator-offset` 页面异常，最终构建的完整套件未复现；两次日志均保留，未归因或修改阅读引擎。

本批使用原创浏览器样本和合成组字事件；原生 WebView、自然触控和硬件输入法未验收，完整真实书库链路仍在 Issue #1 跟进。回滚可单独撤销本批提交，无数据迁移。

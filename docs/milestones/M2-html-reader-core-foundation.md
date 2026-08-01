# M2：HTML 阅读内核基础

## 状态

completed

## 日期

- 开始：2026-08-01
- 目标完成：2026-08-01
- 完成：2026-08-01
- 重开：2026-08-01（三样本与夜间模式验收扩展）

## 目标

建立可分页的本地 XHTML 阅读切片，以三个不同内容结构的样本验证受控资源加载、公式缩放、无行裁切分页和系统夜间模式。

## 范围

- 本地 XHTML、CSS 与直接资源的受控加载；
- 固定页尺寸下的浏览器级分页呈现；
- 行内、行间公式的首个适配规则；
- Agent Browser 对照验收。
- 可复用的 EPUB section 样本提取与三样本明暗模式验收命令。

## 非目标

- 产品内 EPUB、MOBI 或 AZW 导入；
- 数据库、笔记、消息、AI、同步或滚动阅读；
- 自动修复书源结构。

## 退出条件

- [x] `SPEC-0002` 已接受，`PLAN-0002` 已实施；
- [x] 1.2 本地样本在固定页中可读；
- [x] 公式、资源安全和分页边界均有可重复验收；
- [x] 实施评审记录已完成。
- [x] 宏观经济学 5.2 与范畴论 5.6 最小样本已由正式脚本生成；
- [x] 三个样本的实际 Windows host 自检与明暗模式截图验收通过；
- [x] 可复用验收脚本、实施评审与项目记忆已更新。

## 活跃文档

- 规格：`docs/specs/SPEC-0002-html-paged-reader-slice.md`
- 计划：`docs/plans/PLAN-0002-html-paged-reader-slice.md`
- 决策：`docs/decisions/ADR-0003-webview2-reader-host.md`
- 评审：`docs/reviews/REVIEW-0003-html-paged-reader-slice.md`

## 风险

实际 WebView2 与 Agent Browser 可能存在排版差异；最终结论以实际 Windows host 为准。

## 说明

本里程碑只建立阅读内核的第一条验收链路。

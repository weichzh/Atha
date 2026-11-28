# 阅读工作区回跳与长回复

## Status

accepted

## Problem

Linux WebKitGTK 中，约 1000 CSS px 的窗口从阅读记忆跳转到非首屏的唯一引文后，关闭对话会显示本节末页；会话引用正确，正文却不在原处。同一宽度下，约 1600 字的单段回复还会把对话栏和发送按钮撑出屏幕。两项都用原创 EPUB 与隔离资料目录复现。

## Scope

用户批准同时修这两项。保留现有窄屏、B+C 和 A+B+C 布局、消息富文本、源锚点契约和操作位置；不重新设计对话，不改书籍数据或权限。

## Architecture Impact

present

涉及阅读界面显示与共享导航重排的衔接；优先保留负责当前书／章节的有效正文位置，让现有导航在正文可测量时恢复，而不是在每次关闭面板时无条件重跳历史引文。对话溢出由现有组件布局消化，不改消息内容、编辑器协议或视口断点。

## Acceptance Criteria

- 360、约 1000 和 ≥1440 CSS px 从阅读记忆恢复会话，展开可见正文后，原文唯一高亮在实际阅读页内。同节另一页不能冒充恢复成功；不能因历史引文覆盖用户后来主动翻页或跳章。
- 约 1000 CSS px 的同类单段长回复不会撑宽对话，内容能在栏内换行或按原语义局部滚动，发送按钮可见、可继续输入并提交。多段、普通回复和既有窄／宽屏布局不退化。
- 先在真实复现证据与测试中对照失败，再以集成候选的目标 WebView 确认。完整 AppImage 验收使用对应产品包。

## Files And Steps

按阅读位置和对话几何拆分修改；合并后用同一原创 EPUB 检查真实选区、回复、返回、恢复可见原文和继续发送。

## Checks

受影响的导航与浏览器回归、仓库本地提审门、GNOME Wayland／WebKitGTK 完整功能门及 AppImage 包的实际启动和验收。

## Iteration Control

- Mode: `iterative`
- Maximum repair iterations: `4`
- Unchanged-delta limit: `1`
- Human stop: 当前显示不可用时保留本地结果并等待重连；不读取真实书库、不安装或发布。

## Result

修复前 360 CSS px 能在第 11/22 页看到高亮；1000 CSS px 关闭面板后显示第 13/13 页，原文 Range 的左边界约 −4690 CSS px，正文页可视范围为 120–880 CSS px。另有约 1600 字单段将对话宽度撑到约 27,214 CSS px 的截图与日志。

## Review

确保恢复位置来自有效的当前正文或原文锚点，不跨导航代次；确保修复几何后仍能看见原文、使用发送控件并保留书内安全边界。

## Evidence And Residual Risks

本机隔离证据在 `artifacts/local/audits/repository-readiness/`。本轮只验证实际运行的平台与候选。

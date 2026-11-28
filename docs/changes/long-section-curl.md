---
description: 在保留长节原生分页的前提下，为 Linux 桌面对页接入可见页卷曲。
---

# 长章节卷页

## Status

implemented

## Problem

超长章节切到原生横向分页以规避 WebView 的平移限制，此时选了卷页也会直接换页。《数理逻辑导引》第 22、23 屏复现。

## Scope

把用户确认的卷页效果接进 Linux 桌面长章节对页。纸面包括正文和沉浸态页眉、页脚；可展开的阅读工具栏保持独立，打开时只快翻。保留原生分页、阅读位置、章节导航，以及减少动态效果和触拖路径。

## Architecture Impact

present

- Design purpose: 正文继续由原生分页定位，临时卷页层只画两个可见画面。
- Drivers / quality scenarios: 长节翻页不平移或复制整章，公式与目标页文字自然落位。
- Modules / Interfaces / Seams / Adapters: Linux 主阅读 WebView 取已合成的纸面像素；原生同级遮盖层隔离前后页取样。纸面以 CSS 像素取样（最多约 160 万像素），以无损压缩的像素流通过 IPC 传递，压缩不阻塞窗口线程。取目标纸面只等待目标页资源，下一页继续在后台准备；沿用 WebGL 卷曲。
- Candidate and tradeoffs: 不提升整章平移阈值，不申请系统截屏或开放书籍脚本；像素短存内存。取图失败继续显示真实目标页。
- Evidence / ADR / review trigger: 本地 WebKitGTK 用长节公式样本检查双向动作、快速翻页及遮盖清理，扩大平台前重新验证。

## Acceptance Criteria

- 长节仍使用原生横向分页；收起工具栏后，卷页能双向翻动且完整到达目标位置。
- 卷页只取当前与目标的纸面；页眉、页脚与公式随各自纸叶弯折，落地页码正确，不中途闪出目标页再退回。
- 不改变短节、连续滚动、减少动态效果、触摸拖动及其他平台；失败不锁住正文。

## Files And Steps

1. 在 Linux 阅读主窗捕获当前与目标阅读框，限期解除原生遮挡。
2. 布置短命卷页图层，复用原滑动及定位结果。
3. 在临时资料库检查对应页对。

## Checks

WebKitGTK 定向交互、Svelte 检查、Linux 本地构建及仓库文档门。全量测试继续暂停。

## Iteration Control

- Mode: `iterative`
- Maximum repair iterations: `3`
- Unchanged-delta limit: `1`
- Human stop: 需要操作用户的真实资料库、扩大取样权限或发布时停止。
- Per-iteration receipt: 在 Result 汇总，截图本地保存。

## Approval

用户在看过第 22、23 屏独立试用后批准本地实现；实测后要求改善卡顿，并把沉浸态的两条读数按页眉和页脚纳入纸面。

## Rollback

回退该切片即可恢复长节快翻，不涉及数据迁移。

## Result

- 隔离 Linux WebKitGTK／X11 的真实长节有 55 屏：第 22 → 23 → 22 屏都保留原生分页，往返横向偏移同为 25,620 CSS px。正反向实拍可见纸页卷起、公式随纸面转动；打开上下栏后同一输入只快翻。
- 第 1 轮出现交接前闪出目标页。第 2 轮先确认原生遮盖绘制，但首次纹理上传仍可能晚于初始遮盖时限。第 3 轮把取样、贴图与揭盖分阶段，交接时重新确认原生罩层；逐帧检查另发现 WebKit 静态取样漏掉已显示的公式，改成从 GTK 已合成的 WebView 图层取样。最终录屏中未再见提前跳到目标页或准备期间公式变空。
- 桌面试用时，鼠标点书后 Tauri GTK 回调崩溃：插入同级遮盖时额外增加了一层容器，回调把 WebView 的上两层误当成窗口。改为替换单一内容容器，保留原层数和同级遮盖；存在其他原生内容时不安装遮盖。
- GNOME Wayland 以隔离资料库用真实鼠标打开书并正反向卷动第 22 → 23 → 22 屏。录屏复核公式仍随页卷动，未见先闪出目标页；往返横向偏移同为 51,240 CSS px，显示上下栏后仍走快翻。
- 页眉页脚纳入后，GNOME Wayland（1707 × 884 CSS px、DPR 2）同一屏对用真实鼠标录屏，页码随对应纸面进入，停在 23/55、22/55。光改取景并减小 PNG 未减少等待；最终缩小取景缓冲区、改用压缩像素流并移开下一页预取后，从抬起鼠标到卷层准备好由正向 753 ms、反向 564 ms 降到 582 ms、418 ms。录屏中单次绘制间隔仍有约 79 ms，流畅度尚不足。
- 书架调用取样指令被拒绝。Svelte 检查、锁定的 Linux 构建和导航／交互定向单测通过。

## Review

- 传给网页的是请求的 WebView 阅读框，像素只短存在内存中，不能读其他窗或系统屏幕。不访问整章截图，不改正文安全策略。
- 未拿到有效图层时不回放旧页。切换文档会撤销旧遮盖，各阶段有独立清理时限；之后沿用原生页面位置。

## Evidence And Residual Risks

- X11 的录屏与测量在 `artifacts/local/page-turn/long-section-product/`。软件渲染加录屏时，两次输入到卷层出现约 0.97 和 0.53 秒；不能外推为硬件流畅性。修复后的 GNOME Wayland 实录与交互收据在 `artifacts/local/page-turn/long-section-native-click-final/`。
- 页眉页脚的前后对照在 `artifacts/local/page-turn/paper-curl-baseline/` 与 `artifacts/local/page-turn/paper-curl-compressed/`。各为同一屏对一次往返；准备时间不含随后揭开原生遮盖及卷曲过程。输入后现拍并跨进程交接两张纸面仍在关键路径上。

---
description: 分离交付滑动、覆盖、柱面卷页与连续滚动，保留阅读位置和 WebView 资源边界。
---

# 翻页方式与卷页降级

## Status

implemented

## Problem

主线已有横向滑动和连续滚动，V2 原工作区另有覆盖和柱面卷页。卷页的图形能力、临时图层和手势不能改变正文的真实分页、阅读位置或返回路径。

## Scope

按用户已批准的阅读 V2 #19 拆分实现，关联 [Issue #1](https://github.com/weichzh/Atha/issues/1)。原工作区 `1a3da43` 加本地 V2 改动只读；以已合入 #15、叠加 Android 阅读桥候选的 `e963248` 为基线。只改翻页状态转移、渲染器与阅读内设置入口，不改设置导航、笔记、脚注、原生桥和开书动效。

## Architecture Impact

none

现有 Svelte 和平台 WebView 继续显示、处理和写入正文。只在当前章节内用短命视觉图层呈现翻页；沿用 `uiPrefs.pageTurnStyle` 和每书 `readingMode`，不新增应用依赖或信任路径。

## Acceptance Criteria

- 滑动沿用现有分页和拖动；覆盖、卷页支持按钮 / 键盘导航与跟手拖动；连续滚动使用既有本书正文滚动。
- 卷页优先使用 WebGL 柱面网格，图形能力不足降级到 Canvas2D，再降级到 DOM 覆盖或既有分页。减少动态效果时直接到达目标页。
- 切换滚动和分页保留原文位置；跨章、返回书架再打开、取消拖动及快速连续翻页不留下临时图层或错误阅读位置。
- 图层只消费已验证、已渲染的书籍内容；不更改 CSP、资源白名单及快照 Blob 支持。

## Files And Steps

1. 在分页的手势和单页导航接缝接入视觉图层，保留原导航与阅读状态写入。
2. 从 V2 提取覆盖、柱面网格与 Canvas2D，并修正方向、浏览器上下文降级和图层生命周期。
3. 复用既有阅读设置与本书布局恢复；以合成书验证主要交互。

## Iteration Control

- Mode: `iterative`
- Maximum repair iterations: `3`
- Unchanged-delta limit: `1`
- Human stop: 需要修改安全边界、真机安装或外部写入时停止。

## Checks

- `pnpm --dir reader/app check`：0 errors / 0 warnings；`pnpm --dir reader/app build`：通过。
- `node --test reader/web/{interaction,reader-state,navigation}.test.mjs`：59 项通过。
- 在 production preview 运行 `node reader/app/tests/page-turn.browser.mjs`，通过。依赖与 `reader-app.browser.mjs` 相同：可用的 Playwright 与 Chromium；若默认 Chromium 未安装，用 `CHROMIUM_PATH` 指定已有可执行文件。测试只在回环地址启动临时预览，使用 Tauri 配置中的 CSP 和合成章节。
- `bash scripts/check-docs.sh`、`bash scripts/check-workflow.sh`、`git diff --check`：通过。共享 `ACTIVE` 和代码地图留给父任务在集成时更新。

## Result

滑动使用原分页，覆盖使用隔离且不可交互的 DOM 图层，卷页优先使用 WebGL 柱面网格与透纸、投影，在图形能力不足、上下文中途丢失或纹理无法上传时使用独立 Canvas2D；光栅化无法保真时改用 DOM 覆盖。跨章和超长章使用既有导航路径，减少动态效果时不建立覆盖或卷页图层。

在「本书覆盖 → 阅读方式」选连续滚动，只改当前书的布局。选滑动、覆盖或卷页时当前书恢复分页，动效偏好用于其他分页书籍；其他已连续滚动的书不会被改动。V2 存量 `scroll` 动效值按既有本书布局解释：已滚动书保持滚动，分页书按滑动处理。

Chromium 1120×820 和 390×844（DPR 2）验证了按钮与键盘翻页、快速连续键盘翻页、双向真实浏览器触拖、短拖取消、SwiftShader 柱面和中途丢失上下文、拒绝 WebGL 后的 Canvas2D、复杂内容覆盖、减少动态、超长章原生横向分页和连续滚动。跨章向前到下章首屏、向后回上章末页；在同一位置切滚动再切回、返回书架再打开均恢复 Locator。检查了柱面背面的透字、阴影及窄屏回退截图。

集成基底为 Android 适配正式合入后的 `9d6c5c6`，阅读器源码与上述受测候选一致；Vite 构建和完整 Chromium 书架、阅读、脚注、笔记回归通过。

## Review

实现自审：确认书籍 CSP、资源白名单和快照 Blob 支持均无差异；临时画布只读取已通过资源检查的正文 DOM，不序列化、不写入定位，DOM 备份图层不参与输入。切换、取消、跳转、重新排版和卸载清理图层。V2 旧实现的单画布多上下文、向后折叠方向和在切页后才取纹理的次序未沿用。候选仍由父任务独立审核。

首次浏览器验证时测试焦点留在设置触发器上，键盘不翻页；修正为聚焦可定位的阅读区域。跨章后一帧的断言同步到目标页码，避免把节加载完成当作上节末页已呈现。最终两视口通过。

## Evidence And Residual Risks

动态截图位于忽略目录 `artifacts/local/page-turn/`：`webgl-desktop.png`、`cover-desktop.png`、`canvas2d-touch-390.png`。WebGL 证据来自 Chromium 软件 GPU；无 Windows WebView2、Linux WebKitGTK、Android 系统 WebView 或物理设备性能证据。

Canvas2D 光栅化只绘制普通横排文字及已解码、无需变换的图片；复杂版式会在卷页时按原 DOM 覆盖呈现。超长章和边界换章保持原导航，不跨章节抓取/合成纹理。这两点有意不追求 V2 原实现的动画全覆盖。可在设置选择滑动以停用附加效果；回退提交不会丢失现有书籍滚动模式或 Locator。

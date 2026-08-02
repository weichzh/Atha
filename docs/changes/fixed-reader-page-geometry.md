# 固定阅读页设备像素几何

## Status

implemented

## Problem

当前宿主把 1264 × 1680 的页面验收坐标直接作为 Windows 逻辑窗口尺寸，并让整张阅读页按窗口执行 `transform: scale(...)`。在 4K、200% 系统缩放下，窗口客户区因此达到 2528 × 1944 设备像素，页面字体、边距和临时工具栏也被同一变换缩放。页面坐标、应用窗口和系统 DPI 三者没有分层。

## Scope

- 应用窗口和上一页、下一页、字号等壳层控件继续使用 Windows 逻辑像素，遵循系统 DPI 缩放；
- 阅读页内部继续使用固定 1264 × 1680 坐标，32px 字号、页面边距、栏宽和公式尺寸均是固定页面像素；
- 阅读页显示倍率只抵消 `devicePixelRatio`，使页面外框在不同系统缩放下保持 1264 × 1680 设备像素，不再根据窗口大小自适应缩放；
- 将工具栏和错误提示移出阅读页缩放树；窗口不足以容纳页面时由壳层滚动承载，不改变页面几何；
- 原生默认窗口根据页面设备像素、当前显示器缩放和屏幕逻辑尺寸计算，并把最大宽高限制为屏幕的 80%。

## Non-Goals

- 不承诺跨不同面板 PPI 的毫米或英寸物理尺寸一致；本变更固定的是设备像素；
- 不设计正式 Windows 前端壳、工具栏视觉或自定义窗口缩放设置；
- 不改动书源 CSS、自定义样式协议、分页算法、字号档位、公式规则或 benchmark 页面坐标；
- 不为低分辨率屏幕再次缩小阅读页。

## Acceptance Criteria

- [x] 真实 WebView2 在当前 200% DPI 下报告页面外框为 1264 × 1680 设备像素，误差不超过 1px；
- [x] 工具栏不受页面变换影响，按钮保持至少 44px Windows 逻辑高度；
- [x] 页面内部 1264 × 1680、24/32/40px 字号、112px 横向边距和 96/128px 纵向边距不因窗口或系统 DPI 改写；
- [x] 当前 4K、200% 环境中的默认窗口不超过屏幕逻辑宽高的 80%，页面不足部分通过壳层滚动访问；
- [x] 现有安全、分页、公式、明暗主题、三样本和 benchmark 检查继续通过。

## Files And Steps

1. 在阅读页自检中加入设备像素页面几何和壳层控件逻辑尺寸断言，先证明当前 200% DPI 实现失败。
2. 分离壳层、页面外框和固定页面，使用 `1 / devicePixelRatio` 作为唯一页面显示倍率。
3. 调整宿主默认窗口公式并保留页面固定坐标。
4. 运行正式 WebView2、Agent Browser、文档检查和真实窗口测量，完成独立 review。

## Checks

- `pwsh -NoProfile -File scripts/check-reader-slice.ps1`；
- `pwsh -NoProfile -File scripts/check-reader-samples.ps1`；
- `pwsh -NoProfile -File scripts/Invoke-Atha.ps1 check docs -Activity validation -Scope fixed-reader-page-geometry`；
- 当前显示器上的真实窗口和 WebView2 页面几何探针；
- 本次中文 Markdown 的 `autocorrect --fix` 与 `autocorrect --lint`；
- `git diff --check`。

## Rollback

回滚本 change 的提交可恢复原窗口与页面缩放。页面坐标和分页数据不迁移。

## Approval

用户明确要求开始优化：应用及控件遵循系统缩放，页面大小、字体和边距使用不受系统缩放改变的绝对值，并要求不同电脑的页面不因系统参数而改变。

## Result

- 阅读页新增独立壳层和显示外框；页面仍在 1264 × 1680 坐标中布局，唯一显示倍率改为 `1 / devicePixelRatio`，工具栏与错误提示移出页面变换树。
- 当前 4K、200% DPI 下，真实页面自检为 1264 × 1680 设备像素，按钮为 44px 逻辑高度；默认客户区由 1264 × 972 DIP 降为 680 × 816 DIP，窗口外框由屏幕高度的 92.8% 降为 78.3%，占工作区高度由 97.1% 降为 82.0%。
- 三份困难样本的实际 Windows host、24/32/40px、明暗主题、公式、普通图片和分页检查全部通过；交互检查完成翻到第 2 页和字号切换到 40px。
- 最终 10 样本 median/P95：冷启动 636.207/672.043ms、首个稳定页 135.850/137.400ms、热打开 20.800/22.200ms、翻页 6.300/6.400ms、字号重排 20.800/21.000ms。本变更不把单次机器差异称为性能提升。

## Review

- Blocking：Standards 首轮发现 change 已写 `implemented` 但 Review 尚未落档，以及无消费者的 `data-device-scale`；Spec 首轮发现 80% 限制只作用于客户区而未包含窗口边框。已删除无用 dataset，并为系统边框预留 48 DIP 后复测外框高度为屏幕的 78.3%，本节同时完成 review 落档。
- Non-blocking：只在当前 200% DPI 真实环境和 `devicePixelRatio = 1` 的浏览器验收环境运行；缩放公式覆盖其他 DPI，但尚无第二台真实设备证据。
- Out-of-scope：毫米或英寸物理尺寸一致、书源相对单位和正式 Windows 前端壳仍不在本 change 内。

## Evidence And Residual Risks

- 最高证据等级：真实目标证据；当前 Windows WebView2 在 200% DPI 下通过设备像素页面几何自检，Win32 探针确认真实窗口尺寸。
- Evidence：窗口公式单测、Rust fmt/Clippy/test、正式 reader slice、三样本明暗验收、Agent Browser 翻页与字号交互、页面截图均通过。
- Residual risks：尚未在另一台不同 PPI、不同 DPI 的真实显示器上复测；本变更固定设备像素而非毫米或英寸物理尺寸；书源 CSS 仍可按其自身规则使用相对单位。

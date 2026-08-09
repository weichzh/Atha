---
description: 微信读书与 Readest 证据驱动的阅读方式、字号、首行缩进和设置菜单改造。
---

# 阅读操控与排版设置

## Status

implemented

## Problem

Atha 已有分页、点击和基础字号 / 缩进设置，但当前移动端主要靠点击，字号档位少且没有连续拖动反馈；CSS 字号、系统 DPI 与设备像素的关系也没有向用户提供稳定的绝对显示结果。右上设置菜单仍是桌面浮层压缩到手机，层级、动效和关闭行为与微信读书、Readest 的成熟体验有明显差距。

## Scope

- 阅读方式只提供“左右翻页”和“上下滚动”两个互斥模式；左右模式支持连续横向手势反馈并在阈值后翻页，上下模式使用原生纵向滚动，不维持第三套翻页模式；
- 字号改为滑块，提供覆盖微信读书同级范围的多个稳定 CSS 档位；每档通过现有 DPR 隔离换算成设备像素，Linux GUI 与 PCT-AL10 对照验证默认值和两端绝对大小；
- 参考微信读书真机设置实现首行缩进卡片，切换后通过现有 Locator 重排并恢复位置；
- 右上设置入口参考 Readest 的安静图标、分层页面、进入 / 返回 / 关闭动效和窄视口边界，复用现有 Svelte 状态与 CSS，不增加通用动效框架；
- 日常回归只使用 Linux Tauri / WebKitGTK；最终触摸、拖动、滚动、字号绝对大小与安全区在 PCT-AL10 实测，不启动 Android 模拟器；
- 原始对照图继续保存在 `fixtures/local/weread/` 与 `fixtures/local/readest/`，设计结论必须引用 WR-* / RD-* 并打开原图复核。

## Non-Goals

- 不重做书架、消息、词典后端、CSS 模块数据契约或统计 schema；
- 不增加在线字体、同步、云端设置、手势库、通用 UI 框架或动效框架；
- 不实现 PDF、双页 spread、仿真翻页、垂直分页或第三种阅读方式；
- 不在本切片实现 CSS 社区，只保留现有模块包接口。

## Architecture Impact

present

- Design purpose: 用一个明确阅读方式和一套可校准排版参数替代互相独立的点击 / 滑动开关与稀疏字号按钮。
- Drivers / quality scenarios: `A-CTRL-01` 要求触摸拖动连续反馈且不误触选区、链接、表格或对话；`A-TYPE-01` 要求同一字号档在 DPI / DPR 不同的目标上保持可解释的设备像素尺寸和 Locator 稳定。
- Modules / interfaces: `preferences` 拥有模式、字号与缩进记录；`interaction` 只解释横向翻页手势；`pagination` / `navigation` 继续拥有布局与 Locator 恢复；Svelte Preferences 只投影设置与动效。
- Candidate and tradeoffs: 复用浏览器原生滚动、Pointer Events、CSS transition、`input[type=range]` 与现有 Navigation 队列；不引入 Swiper、手势状态机或动画依赖。只有真机证明原生滚动不能满足章节切换与恢复时才增加最小 adapter。
- Evidence / review trigger: Node 状态测试、Linux 真 GUI 的分页 / 滚动 / 重排 / reduced-motion、PCT-AL10 的真实拖动与截图、独立产品和 Standards review。系统 WebView 或 DPR 模型变化时重新校准绝对字号。

## Acceptance Criteria

- [x] 设置中只显示“左右翻页”和“上下滚动”，切换、重启与按书恢复均稳定；
- [x] 左右手势连续跟手且不会劫持竖向意图、文本选区、链接、表格、代码、dialog 或多点触控；上下模式可原生滚动并跨章节继续阅读；
- [x] 字号滑块具有足够档位，默认值和两端在 Linux GUI / PCT-AL10 的 CSS px、DPR 与设备像素证据明确，文本不裁切；
- [x] 首行缩进卡片提交后 Locator 恢复到原文位置，书源样式开关和 CSS 模块保持有效；
- [x] 右上设置在桌面和窄视口层级清楚、可返回 / 关闭、动画克制且尊重 reduced-motion，不遮挡安全区；
- [x] Node、Svelte、Rust、Linux GUI、PCT-AL10、AutoCorrect、文档 gate 与独立 review 通过。

## Files And Steps

1. 先从 WR-* / RD-* 原图与真机行为冻结默认字号、档位、缩进、两种模式和设置层级；不从旧报告猜尺寸。
2. 用现有偏好迁移和 Node 测试固定两种模式、字号档位、缩进与 Locator 恢复，再实现最小状态变化。
3. 复用原生 range、Pointer Events、滚动和 CSS transition 改造 Preferences 与 Interaction；先 Linux GUI，再 PCT-AL10。
4. 记录绝对尺寸和交互基准，独立复审后更新事实所有者与路线图。

## Checks

- `node --test reader/web/reader-state.test.mjs` 与受影响 reader 单测；
- `pnpm --dir reader/app check`、`pnpm --dir reader/app build`；
- `pwsh -NoProfile -File scripts/check-fb2-source.ps1 -VerifyLinuxGui`；
- PCT-AL10 原生拖动、左右翻页、上下滚动、字号 / 缩进和设置动效实测；
- workspace Rust、AutoCorrect、文档 gate 与 `git diff --check`。

## Rollback

恢复旧偏好投影和设置 DOM 即可；书籍、消息、词典、CSS 模块与统计数据不迁移或删除。未知或旧应用记录必须回退到稳定的左右翻页与默认排版，而不是阻断打开。

## Approval

用户已明确要求在词典与性能完成后参考微信读书和 Readest 实现首行缩进、字号 / DPI 滑块、左右滑动 / 纵向滚动两种阅读方式，以及更成熟的右上设置菜单、拖动和动画；日常验证改用 Linux GUI，PCT-AL10 只做真机专项。

## Result

设置改为 Readest 风格的移动底部抽屉和分层页面，复用原生 range、分段控件、CSS transition 与 reduced-motion。应用字号范围固定为 16–40 个逻辑 CSS px，默认 19；内部阅读画布按 `字号 × DPR` 写入设备像素，PCT-AL10 的 DPR 3 对应默认 57 设备像素。行距使用 1.55 / 1.8 / 2.05 三档无单位倍率，首行缩进提供顶格和 2em 两张卡片。

阅读方式只保留左右翻页和上下滚动。分页手势提供实时横移与 170ms 收束，滚动模式使用浏览器原生纵向滚动；模式状态同时写入外层 reader 与闭合 Shadow DOM 内正文，避免样式选择器跨边界失效。华为 WebView 114 的空 `pointerType` 归一为 touch，纵向原生手势在 `pointercancel` 后由 `touchend` 处理章节边界。

## Review

独立产品审计先以 PCT 原图指出默认字号和设置页密度问题；代码复审又发现闭合 Shadow DOM 的受保护目标可能被外层 touch 重定向，修复为与 pointer 相同的 inside / outside 去重并加入链接起手不导航诊断。最终复审未发现剩余 P1 / P2。

## Evidence And Residual Risks

Linux Tauri / WebKitGTK 正式门通过 4 个 section、3 条目录、CodeMirror 6、CSS 模块 P95 2ms、阅读统计 P95 2ms 与宽窄视口截图。PCT-AL10 真机验证字号 19→28 的拖动与 57→84 设备像素换算、首行缩进、两种模式、水平跨章、原生纵向滚动 `scrollTop` 0→1738.7，以及滚动底部从 section 3 / 4 进入 4 / 4。纵向长内容由验收探针注入，证明目标 WebView 的手势、Shadow DOM 布局和跨章链路，不冒充真实长书内容验收。

剩余风险是未签名 debug APK 不等同于发布验收；系统 WebView、DPR 模型或安全区策略变化时仍需重新校准。

---
description: 保持不同显示密度下的有序列表编号完整，并保留已验证的括号数字书源样式。
---

# 有序列表裁切与括号编号

## Status

implemented

## Problem

同一 1.3 节使用 EPUB 自定义括号数字编号。PCT-AL10 的窄页只呈现编号右侧；Linux 上同一编号被显示为带点的默认十进制编号。Chromium 像素回归重现分页边缘裁切；原生 WebKitGTK 2.54.3 对照表明同一计数定义放在文档级生效，放入正文 Shadow DOM 后不生效。

## Scope

- 缩进按正文画布像素比例与字号留出空间，保留书源显式缩进、HTML 列表结构和编号。
- 针对 Linux WebKitGTK 补偿已确认的括号十进制计数样式，不对其他计数定义推测等价表达。
- 正文仍隔离；只允许应用定义的固定计数样式出现在页面全局，不把书源 CSS 移出 Shadow DOM。

## Architecture Impact

none

## Acceptance Criteria

- [x] PCT-AL10 同一段的编号完整；新旧包对照。
- [x] Linux 原生可见编号与书源和手机的括号数字形式一致。
- [x] 覆盖高、低 DPR、原生分页滚动、左右书写方向、书源缩进和不匹配规则。

## Files And Steps

1. 在 Tauri 阅读 CSS 留出编号所需缩进。
2. 对确切的书源样式使用应用生成的临时别名，保留 CSS 规则和自然计数。
3. 用同一段书源与两端原生像素复验。

## Checks

有序列表像素回归、既有 SVG 主题回归、Linux 原生阅读检查、PCT-AL10 同签名候选同页对照。

## Iteration Control

- Mode: `iterative`
- Maximum repair iterations: `3`
- Unchanged-delta limit: `1`
- Human stop: 需要扩大兼容范围或任务授权时停止。
- Per-iteration receipt: 在 Result 和 Evidence And Residual Risks 记录。

## Rollback

回退本次独立变更，书籍和阅读状态格式不变。

## Approval

用户明确将下一项限定为 Linux 与手机有序列表显示不一致、手机编号裁切。

## Result

- Iteration: 1
- Review findings: 原生像素对照确认两种根因；已通过同一页真机与 Linux 原生复验。
- Repair: 按字号和视觉像素留缩进；只为确切的括号十进制格式补 Linux 别名，不重写编号。
- Validation cases: DPR 1/3、右侧与左侧编号、分页滚动、原书同段。
- Remaining delta: 审查发现零特异性与分层书源缩进会被覆盖，固定全局别名也可能影响未匹配的规则。
- Evidence level: Chromium、Linux 原生 WebKitGTK、Android 真机。
- Stop reason: 转第二轮修复审查发现。

- Iteration: 2
- Review findings: 修复最低优先级与别名隔离；快照不应保存运行时别名。
- Repair: 默认缩进置于书源 CSS 之前的匿名层。只在 Linux WebKitGTK 确实命中时注册应用生成的临时别名，先排除书源与用户 CSS 中的重名；快照保留原始 CSS，由 Tauri 在回放时传入兼容逻辑，保留独立宿主的平铺模块入口。
- Validation cases: 同特异性、分层书源、同名用户规则、快照实际编号、原书同页、Linux 正式门。
- Remaining delta: 无阻塞项。
- Evidence level: Chromium、Linux 原生 WebKitGTK、Android 真机。
- Stop reason: 约定回归均通过。

## Review

- Blocking: 已处理默认缩进优先级和文档级固定别名两项。
- Non-blocking: 其他自定义计数样式和含条件的定义仍由原生引擎处理。

## Evidence And Residual Risks

- PCT-AL10 / Huawei WebView 114：同签名测试包保持书库和阅读位置。同一段中，基线编号左侧被裁切；采用匿名默认层的候选完整呈现 `(2)` 至 `(8)`。随后只调整了 Linux 快照兼容逻辑的注入入口，Android 不启用该逻辑。
- Linux / WebKitGTK 2.54.3：最终候选以原生独立书库打开同段，带点编号变为括号编号；完整 Linux 原生阅读门通过（220 次手势采样）。
- Chromium 回归对比分页裁切与无裁切的可见编号像素，覆盖书源开关、同特异性和分层缩进、重名规则、快照回放。原生 WebKitGTK 快照对照确认括号编号实际宽度与直接文本相同；重名及无定义规则维持原回退。原生截图只留在本机忽略目录。

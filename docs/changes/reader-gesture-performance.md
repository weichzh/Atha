---
description: 用可信指针基准修复媒体与表格上的翻页仲裁，并压缩重内容滑动热路径。
---

# 阅读手势与翻页性能

## Status

implemented

## Problem

分页模式把图片、公式、表格和代码整体视为受保护目标，导致这些内容占据页面时点按与横向拖动不能翻页。拖动热路径还在每个 `pointermove` 同步读取样式和几何；松手后又在 170ms 收束动画中重复扫描整章 Locator，并为同尺寸公式的成功解码执行完整重排，重内容章节因此比普通书更慢。

现有 Linux 门只用不可信的合成 `PointerEvent` 确认进入 dragging 状态，没有覆盖真实命中、逐帧跟手、媒体误开、表格边界移交或动画稳定。Readest 0.11.20 真机与固定源码证明其图片拖动优先于点击，但横向表格即使到边界仍永久截获手势；Atha 借用前者，不复制后者。

## Scope

- 分页模式使用一次手势一个 owner 的最小仲裁：多点、选区、表单、链接、弹窗与纵向意图保持受保护，图片、公式、表格和代码允许横向翻页；
- 左右区域点按优先翻页，中心点按继续打开媒体 / 结构查看器或切换工具栏；横向拖动提交后抑制兼容 `click` 与 `dblclick`，避免误开预览；
- 横向溢出容器在起手方向仍有空间时滚动自身；已处于对应边界时把该次手势交给翻页，不在一次序列中反复换 owner；
- 用 `requestAnimationFrame` 合并拖动更新，缓存分页步长和显示比例，逐帧只写 transform；
- 固定尺寸公式成功校验、解码和显现不触发重排；只有失败替换导致布局变化时才捕获 Locator、重排并恢复；
- 缓存同一稳定页面的内容偏移，避免控制、进度和书签在一次翻页后重复全章扫描；
- 在现有 Linux Tauri / WebKitGTK 门中增加 W3C Pointer Actions 可信输入、rAF 时序与表格边界矩阵，不增加 WebDriver、手势或动画依赖；录屏只做感知复核，Performance API 是数值门槛；
- 继续只提供左右分页和纵向滚动两种阅读方式；CSS 社区继续只保留模块包接口。

## Non-Goals

- 不引入 Readest 的截图覆盖层、Canvas / WebGL 卷页、邻章预热或整套 Foliate 架构；
- 不允许书内脚本、网络资源或新的路径权限；
- 不在本切片调整字号、设置页视觉、词典、书架、CSS 模块数据或阅读统计 schema；
- 不因合成压力样本改写 DPR / brightness 模型；只有 Linux WebKitGTK A/B 证明它是主瓶颈时另立 change。

## Acceptance Criteria

- [x] 图片、公式和普通表格上的左右区域点按及明确横向拖动均恰好翻一页，且不误开查看器；媒体中心点按仍预览，表格 / 代码中心点按仍切换工具栏，双击和键盘预览不变；
- [x] 横向溢出表格中部拖动只改变 `scrollLeft`，对应边界向外拖动恰好翻一页；
- [x] 多点、选区、链接、表单、弹窗和纵向意图不被翻页劫持；纵向滚动模式仍使用原生滚动；
- [x] 拖动帧没有 geometry / layout read，每次输入序列只缓存一次几何；成功公式显现不重排，失败替换仍恢复同一 Locator；
- [x] Linux Tauri / WebKitGTK 门请求 W3C touch Actions，事件必须 `isTrusted`，并分别记录请求与实际 `pointerType`；每类 5 次预热、20 次测量均单步正确；
- [x] 横拖首个视觉反馈 P95 不超过 33.4ms、实际视觉更新间隔 P95 不超过 25ms、最大间隔不超过 50ms、松手到稳定 P95 不超过 220ms，点按松手到首个视觉变化 P95 不超过 50ms；
- [ ] 候选交给用户在 PCT-AL10 上亲自复核图片、公式、表格、代码、边界移交和滑动手感；结果由用户确认后回填，不以 Linux 自动化替代。

## Architecture Impact

present

- Design purpose: 把内容激活与翻页从静态 target 黑名单改为按区域、方向和溢出边界仲裁，并让翻页热路径不随整章复杂度重复做无关工作。
- Drivers / quality scenarios: `A-CTRL-02` 要求媒体覆盖页面时仍可完成单页导航；`A-PERF-02` 要求重图片 / 公式 / 表格章节的跟手帧不做布局读，释放后在 220ms 内稳定。
- Modules / interfaces: `interaction` 拥有 owner、区域和 click 抑制；`pagination` 拥有 rAF transform、步长与 Locator 缓存；`content.loadVisible()` 报告 `loaded` 与 `layoutChanged`；`navigation` 继续串行化最终翻页；diagnostics 与 Linux runner 只暴露无内容的测试时序。
- Candidate and tradeoffs: 复用 Pointer Events、W3C Actions、CSS transition 和现有 Navigation 队列，不引入手势库；Readest 的 6 / 8px 方向认领和拖动优先语义可借鉴，但其表格永久截获与截图动画管线被拒绝。
- Evidence / review trigger: 合成 DOM red / green、Linux Tauri 可信输入与帧基准、用户在 PCT-AL10 的手动验收和独立 review；只有 WebKitGTK trace 证明整章 transform 仍是主瓶颈时才研究原生 scroll 或分片渲染。

## Files And Steps

1. 先以现有 FB2 门中的诊断专用媒体 / 表格目标和 W3C Actions 固定当前必红语义，不用私人书籍建立功能 oracle；
2. 在 `interaction` 与 `pagination` 做最小 owner、边界、rAF 和缓存改动，再使可信输入矩阵转绿；
3. 把 `content` 的成功显现与失败替换分开，只为真实布局变化保留 Locator 重排；
4. 在 Linux WebKitGTK 跑 5 + 20 次帧基准，独立评审后把候选交给用户在 PCT-AL10 复核真实触摸；只有数值无法解释主观差异时再补局部录屏。

## Checks

- reader module 语法、现有 Node 测试、Svelte check / build 与 workspace Rust 检查；
- `pwsh -NoProfile -File scripts/check-fb2-source.ps1 -VerifyLinuxGui` 的可信指针矩阵、帧基准、截图与日志隐私；
- 固定公式压力入口的公式 / 页数下限与 5 + 20 逐场景指针指标；
- 用户在 PCT-AL10 上手动复核图片、公式、表格、代码和边界移交；
- AutoCorrect、文档 gate、`git diff --check` 与独立 review。

## Rollback

恢复旧 target 保护和同步 transform 即可；本切片不迁移书籍、偏好、消息、词典、CSS 模块或统计数据。

## Approval

用户已明确要求在 EPUB 兼容完成后研究 Readest 的源码、控制与动画，用录屏和更好的 benchmark 修复图片、公式和表格上的点击 / 滑动翻页失效及重内容卡顿，并要求日常验证使用 Linux GUI、最终可使用 PCT-AL10 真机。

## Result

`Interaction` 现在按一次序列一个 owner 仲裁分页、横向溢出和内容激活。图片、公式、表格与代码不再整体截断页区点按和横拖；宽表在中部滚动自身，在起手方向已经到边界时由新手势翻页。多点、选区、链接、表单、弹窗和纵向意图保持受保护，已提交横拖同时抑制兼容 `click` 与 `dblclick`。

分页在起手时缓存视口、DPR 换算、页步长与稳定页 Locator 偏移；move 热路径只更新内存并由单个 rAF 写 transform 或 `scrollLeft`，收束动画缩短为 150ms。`content.loadVisible()` 显式返回 `loaded` 与 `layoutChanged`；固定尺寸公式成功显现不再捕获 Locator 或重排，只有失败替换在首次 DOM 变化前捕获一次并恢复。

Linux runner 增加 13 场景 W3C Actions 矩阵和普通 / 公式压力章节的逐场景 P95。它只在显式诊断查询下安装匿名目标，不给产品增加手势库、动画依赖或第二阅读模型。私密样本身份和章节只来自忽略 sidecar，输出与 AppLog 不包含路径、标题、作者、正文或哈希。

## Review

独立评审先发现文档仍沿用旧 Windows 十样本事实，以及内容自检比必要范围更重；前者已改为 Linux 5 + 20 逐场景门，后者已收敛为一次成功与一次失败的最小同批探针。再次复核 owner、DPR、溢出边界、兼容事件、多点、纵向意图、rAF、Locator 缓存和四个 `loadVisible()` 调用点后，没有剩余 P1 / P2 功能问题。

## Evidence And Residual Risks

正式 Linux Tauri / WebKitGTK 门完成 13 个场景，每个 5 次预热、20 次测量。普通章节最差聚合值为横拖首个视觉反馈 32ms、点按松手到首个视觉变化 7ms、视觉帧 P95 / 最大帧 17 / 17ms、松手稳定 212ms；公式压力章节对应为 30ms、7ms、25 / 25ms 与 216ms，均在门槛内。workspace Rust、Svelte check / production build、Tauri build、书架 / 阅读纵切、截图非空和日志隐私同时通过。

该门请求 touch Actions，事件均为可信，但当前 WebKitGTK 实际报告 `pointerType=mouse`，所以最高证据是 Linux 真实 GUI 与可信自动化指针，不是实体触摸。代码块与表格共用 `table, pre` 仲裁分支，既有结构化检查覆盖其中心操作和预览，但本轮 13 场景矩阵没有复制一组等价的代码块横拖；该项连同真实滑动顺滑度、手指方向容错和内容触摸由用户在 PCT-AL10 亲自验收。数值无法解释主观差异时再补真机录屏。

本机另已构建仅含 `arm64-v8a` 的优化 APK，并以本地 Android 调试证书完成 16 KiB ZIP 对齐和 v2 / v3 签名验证。它没有被安装或启动到 PCT-AL10；设备安装、真实触摸和主观性能仍由用户完成。

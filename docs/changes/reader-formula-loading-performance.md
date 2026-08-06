---
description: 用公式密集型真实 EPUB 建立可重复基准，并按当前页面优先加载 SVG 公式。
---

# 公式密集章节加载性能

## Status

implemented

## Problem

《数理逻辑导引 (2017)》SHA-256 为 `c316559b6428d05b7ba81228879606e05f9adf6f3e67df917f6c90ce77ff6708`；其中 `EPUB/text/ch095.xhtml` 含 1332 个公式 SVG，是当前最强的真实公式压力章节。

固定优化前基准 `1786026645854-21064` 的 P95 为：冷启动 2137.424ms、首个稳定页 1590.200ms、热打开 148.200ms、翻页 7.000ms、字号重排 112.600ms。前三项超过既有门槛。

保持 1332 个公式节点、文字与排版不变，只让公式复用一个 SVG 后，基准 `1786026269909-13516` 的对应 P95 降为 792.682ms、242.400ms 和 89.900ms。差异证明主要瓶颈是首次打开时取回、校验和解码整章全部 SVG，不是分页或公式缩放本身。

## Scope

- 增加一个固定源哈希、固定章节、十样本的公式密集型 benchmark 入口；
- 只对具有合法显式宽高的 SVG 公式延迟设置 `src`，先以同尺寸占位完成分页；
- 当前页及下一页的公式在进入稳定状态前完成既有 SVG 安全校验和解码；其他公式在真正接近视口时再处理；
- 同一章节内相同 SVG 只校验一次，关闭章节时释放短期缓存；
- 安全失败界面显示具体错误代码与发生阶段，不暴露书籍路径或内容；
- 保持现有 WebView2、单 section DOM、分页、公式缩放、Locator 与安全失败语义。

## Non-Goals

- 不增加持久缓存、数据库、service worker、预热系统、worker 或 DOM 虚拟化；
- 不改变普通图片、EPUB importer、配置与用户数据；
- 不放宽脚本、网络、未知资源、SVG 外部引用或样式安全规则；
- 不把本机数字外推为跨设备承诺。

## Acceptance Criteria

- [x] 固定样本 benchmark 可重复生成压力章节并保存十样本 median/P95；
- [x] 同机优化后首稳 P95 至少比 1590.200ms 降低 50%，并回到 750ms 门槛内；
- [x] 公式压力章的冷启动、热打开、翻页和字号重排 P95 分别不超过 1500、200、50 和 150ms；普通章节继续使用 2000、120、50 和 150ms 门槛；
- [x] 当前页和下一页公式在报告稳定前已经解码，远端公式不阻塞首稳；
- [x] 未通过既有 SVG 校验的公式在设置可呈现 `src` 前失败；
- [x] 公式宽高、基线、居中、明暗过滤、无裁切和 Locator 恢复不回退；
- [x] 既有困难样本与真实 Tauri/WebView2 入口继续通过。

## Files And Steps

1. 固定公式压力样本与 benchmark 命令，保留优化前 CSV 作为本机对照；
2. 在 Content 中分开公式占位、按页校验解码与普通资源加载；
3. 让 Pagination 在首屏、翻页和重排后等待当前页及下一页公式，不复制导航状态；
4. 补安全、几何和加载范围回归，再运行同机前后基准与正式 reader gate；
5. 更新架构、代码地图、路线图和活动指针，完成独立 review。

## Checks

- `pwsh -NoProfile -File scripts/check-reader-formula-performance.ps1`
- `pwsh -NoProfile -File scripts/check-reader-samples.ps1`
- `pwsh -NoProfile -File scripts/check-tauri-reader.ps1`
- `pwsh -NoProfile -File scripts/Invoke-Atha.ps1 check docs -Activity validation -Scope project-workflow`

## Rollback

回退本 change 的提交即可恢复整章同步 SVG 校验与解码；本次不改变书籍、配置或用户数据格式。

## Approval

2026-08-06：用户要求暂缓本地数据恢复，继续按此前方向优化性能，并指定《数理逻辑导引 (2017)》作为公式密集型测试样本，要求建立 benchmark 比较提升效果。

## Result

`scripts/check-reader-formula-performance.ps1` 锁定源 EPUB 哈希与 `ch095`，构建当前 Tauri 前端、导出忽略版本控制的压力 fixture，并复用正式十样本 WebView2 benchmark。Content 只延迟具有显式合法宽高的 SVG 公式，在设置 `src` 前完成原有 SVG 校验；首屏和翻页等待当前页与下一页，其他公式进入相邻视口时再加载。完整自检和热打开 benchmark 才以 16 个一批的空闲任务补齐整章。章节重新渲染复用本节已校验、已解码 URL，离开章节即释放。

最终运行 `1786031994940-34120` 与固定基线的 P95 对比如下：

| 阶段 | 优化前 | 优化后 | 变化 | 门槛 |
|---|---:|---:|---:|---:|
| 冷启动 | 2137.424ms | 1250.823ms | -41.5% | 1500ms |
| 首个稳定页 | 1590.200ms | 509.800ms | -67.9% | 750ms |
| 热打开 | 148.200ms | 111.300ms | -24.9% | 200ms |
| 翻页 | 7.000ms | 6.000ms | -14.3% | 50ms |
| 字号重排 | 112.600ms | 107.200ms | -4.8% | 150ms |

每次加载相邻页公式前先捕获当前文本偏移，加载后刷新页数并恢复该偏移；空白锚点改取相邻可见文字，窗口尺寸切换期间暂时没有文字矩形时保留已校验偏移和当前页。安全失败界面显示具体错误代码与阶段，不再只显示统一文案。产品态不在后台预热整章，避免任务与章节切换竞争；没有引入持久缓存、worker、虚拟化、配置项或数据迁移。

普通章节 Tauri gate `1786031760991-19040` 的冷启动、首稳、热打开、翻页与字号重排 P95 分别为 779.382ms、214.700ms、22.000ms、7.200ms 和 48.500ms，均在原门槛内。`scripts/check-reader-samples.ps1` 同时通过三类困难样本的原生 WebView2、浏览器双主题、选择、标注、搜索与跨进程状态链路。产品运行只检查当前页与下一页；正式验证补齐整章公式后再执行全局裁切检查。

## Review

- Blocking：无；
- Non-blocking：无；
- Out-of-scope：持久缓存、跨设备比较和其他格式。

独立 Standards review 与 Spec review 均确认：窗口重排后的相邻页加载、校验后设置 `src`、后置整章补齐、全局裁切、Locator 恢复、claims 和最终证据一致。

## Evidence And Residual Risks

本机真实 Tauri/WebView2 公式 benchmark、普通章节 Tauri gate 和完整 reader 样本 gate 已通过。数值只代表当前机器；普通图片仍沿用同步加载，短期校验缓存只在当前章节内有效。后续只有在新真实样本再次超门槛时，才研究持久缓存、worker 或 DOM 虚拟化。

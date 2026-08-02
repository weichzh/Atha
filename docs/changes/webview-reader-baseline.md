# WebView2 阅读基线收敛

## Status

implemented

## Problem

组合式 XHTML 引擎实验未通过三样本浏览器视觉门，继续维护第二套 HTML/CSS、布局和绘制链没有产品价值。现有 WebView2 基准又把多字号验收、安全探针和整章裁切扫描计入用户可见阶段，无法准确指导后续优化。

## Scope

- WebView2 作为 ATHA 唯一阅读渲染技术；不再保留或构建自研组合式引擎，外部引擎足够成熟且有新的实测依据后再单独决策；
- 移除本轮尚未提交的组合式引擎 crate、依赖、host、样本清单、检查脚本与活动研究路由；
- 首个稳定页在初次真实布局和裁切检查完成后立即记录，后续多字号、安全验收不计入该阶段；
- 冷启动在宿主收到首个稳定页事件时结束，不等待其余验收完成；
- 翻页计时只覆盖实际翻页处理与下一绘制帧，整章裁切检查保留为计时外验收；
- 更新稳定目标、阅读架构、代码地图、项目工作流 gate 与当前指针。

## Non-Goals

- 不增加持久缓存、预处理数据库、后台预热、DOM 虚拟化或性能模式；
- 不改变书籍信任边界、资源协议、CSP、分页算法、公式规则或 UI；
- 不预设跨机器性能阈值，也不因本轮数字调整 WebView2 运行时参数。

## Acceptance Criteria

- [x] workspace 只保留正式后端与 WebView2 reader host，不含 Blitz、组合式绘制或自研阅读引擎依赖；
- [x] `first_stable`、`cold_start` 与 `page_turn` 的起止点不包含计时外验收；
- [x] 现有 Rust、WebView2 安全、三字号分页和三样本明暗验收继续通过；
- [x] 重新采集至少 10 个冷启动、首个稳定页、热打开、翻页和字号重排样本并报告 median/P95；
- [x] 文档守卫、中文排版检查与独立 review 通过。

## Files And Steps

1. 取消旧组合式引擎 task，删除其未提交实现与项目 gate，恢复正式 workspace。
2. 调整阅读页和宿主计时边界，不改变实际渲染路径。
3. 更新稳定事实、运行 benchmark，并完成独立 review。

## Checks

- `pwsh -NoProfile -File scripts/check-reader-slice.ps1`；
- `pwsh -NoProfile -File scripts/check-reader-samples.ps1`；
- `pwsh -NoProfile -File scripts/Invoke-Atha.ps1 check docs -Activity validation -Scope webview-reader-baseline`；
- 本次中文 Markdown 的 `autocorrect --fix` 与 `autocorrect --lint`；
- `git diff --check`。

## Rollback

回滚本 change 的提交可恢复原计时边界；组合式引擎只是未提交实验，不作为正式产品路径恢复。若未来重新评估外部引擎，应从新的研究和 change 开始。

## Approval

用户明确决定只使用 WebView 技术，不再使用自研引擎；待外部库足够成熟后才重新考虑。本轮只批准最基本、最常规的 WebView2 性能优化。

## Result

- 已取消并释放旧组合式 XHTML 引擎 task 的 claims，移除未提交的 Blitz/组合式 crate、host、依赖、检查脚本和活动研究路由；正式 workspace 恢复为 `atha-backend` 与 `atha-reader-host`。
- 阅读页在初次布局与裁切检查后立即发送 `first_stable`；宿主收到该事件时记录 `cold_start`；翻页在下一绘制帧上报耗时后再执行整章裁切验收。
- 新 10 样本 median/P95：冷启动 658.205/715.111ms、首个稳定页 135.800/155.500ms、热打开 20.800/21.000ms、翻页 6.250/6.500ms、字号重排 20.800/21.000ms。旧数据包含额外验收工作，因此差值只证明计时边界已纠正，不称为运行时加速。
- 三份困难样本的实际 Windows host、24/32/40px、明暗截图、公式/普通图片分类、对比度和网络限制继续通过。

## Review

- Blocking：首次 review 发现 `ACTIVE` 未声明 Context Bundle、`READER-CORE` 同时声明与否定 WebView2 选型、`MAP` 把缺少跨日期运行误写为缺少重复样本；均已修正。
- Non-blocking：工作树仍有本 change 之前的文档体系和统一 CLI 改动；本 change 只接管与稳定目标、当前指针、事实所有者和项目 gate 重叠的文件，不把其他既有改动计入阅读器实现结论。
- Out-of-scope：持久 UDF、资源缓存、预处理、后台预热、视口渐进解码、DOM 虚拟化和性能模式由后续真实瓶颈决定。

## Evidence And Residual Risks

- 最高证据等级：真实目标证据；正式脚本在当前 Windows 系统 WebView2 上运行真实本地样本、实际窗口 host 与浏览器截图链路。
- Evidence：Rust fmt、Clippy、测试与构建通过；三样本 host 和明暗验收通过；10 个冷进程与同一 WebView 热路径样本齐全；文档守卫、AutoCorrect 和 `git diff --check` 通过。
- Residual risks：没有设备指纹、跨日期或跨机器统计；基准只校正阶段边界，不证明代码执行速度提升；真实产品尚未实现跨书籍会话与缓存生命周期。

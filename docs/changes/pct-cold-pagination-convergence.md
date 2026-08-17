---
description: 修正 PCT-AL10 未缓存公式加载后分页总数不收敛的问题。
---

# PCT 冷加载分页收敛

## Status

implemented

## Problem

PCT-AL10 冷进程进入未缓存的公式密集长章节时，首次分页得到 74 页；连续加载公式后，正文到第 67 页已经结束，但第 68 至 74 页仍重复显示同一段尾部内容。相同章节缓存后重开只有 67 页。共享资源加载路径只把失败替换视为布局变化，成功加载及其延迟完成没有触发锚点恢复和分页重算。

## Scope

- 比较可见资源从占位状态转为真实内容前后的实际几何，只把真实变化视为布局变化；
- 在资源写入前保存当前文字锚点，加载后通过现有分页重排恢复该锚点；
- 延迟完成的资源继续走现有合并重排入口；
- 在 PCT-AL10 上用未缓存长章节连续翻页复测首帧和章节尾部。

## Non-goals

- 不预加载整本书或整个长章节；
- 不改变公式尺寸、阅读字号、页边距或缓存边界；
- 不增加新的加载界面、计时器或分页算法。

## Architecture Impact

会话、导航和分页接口不变。共享内容加载器在资源写入前捕获一次文字锚点，成功加载后只在书页、资源或父元素几何变化时设置 `layoutChanged`；失败及几何变化的延迟完成继续进入现有合并重排，分页器仍在一个入口恢复锚点。

## Acceptance

- `COLD-LAYOUT-01`：冷进程进入未缓存章节后，第一个可见正文帧不再因公式显现而收缩；
- `PAGE-CONVERGENCE-01`：公式密集长章节首次连续翻页时，页数随真实布局收敛，不出现重复或空白尾页；
- `PAGE-CONVERGENCE-02`：最后一页之后直接进入下一章节，缓存前后章节尾部一致；
- `REGRESSION-01`：内容加载自检、Linux 阅读器正式门、PCT 构建及同包更新检查通过。

## Files And Steps

1. 修正共享可见资源加载结果和延迟完成通知。
2. 复用现有分页锚点恢复与重排，不新增旁路。
3. 扩充现有内容加载自检并在 PCT-AL10 重放长章节冷路径。

## Checks

- `node --check reader/web/content.mjs`；
- `bash scripts/check-reader-linux.sh`；
- `bash scripts/check-reader-formula-performance.sh --epub fixtures/local/数理逻辑导引\ \(2017\).epub`；
- `bash scripts/check-pct-reader.sh build`；
- `bash scripts/check-pct-reader.sh install --device 5ENDU19917001679`；
- PCT-AL10 冷进程进入未缓存公式密集长章节并连续翻至下一章的高频录屏；
- 仅对本次中文 Markdown 运行 `autocorrect --fix` 与 `autocorrect --lint`。

## Result

共享内容加载器在写入当前可见图片或公式前记录书页、资源及其父元素几何，并在成功终态后比较；只有几何真正变化时才复用现有 `layoutChanged`、Locator 恢复和分页重排。超过 50ms 终态窗口的成功加载也采用同一几何判断并复用现有合并延迟重排；失败占位继续无条件重排。既有自检同时覆盖几何变化的成功加载与几何稳定的迟到加载。

## Review

复核了所有 `loadVisible()` 调用方、分页锚点恢复、失败替换、迟到终态和关闭 generation 边界。改动只扩充共享内容加载结果，不增加分页旁路、整章等待或缓存状态；正式 Linux 门和最终 APK 的两轮 PCT 冷进程长章节重放均覆盖了实现路径，未发现阻塞问题。

## Evidence And Residual Risks

- 真实目标红灯：`artifacts/local/audits/pct-reader-unseen-long-section-20260817T122017Z/` 显示冷首轮 74 页中第 68 至 74 页重复同一尾部内容；同章缓存后为 67 页。
- 静态 / 本地：`node --check reader/web/content.mjs` 与完整 `bash scripts/check-reader-linux.sh` 通过；后者包含实际 WebKitGTK GUI、自检、响应式工作区、220 次手势测量和 AppLog 隐私检查。公式压力门的公式、边界和手势诊断已完成，但整个命令随后因隔离资料库的最近阅读数量为 2、旧断言期望 1 而失败；该晚期失败未改动，本次不扩大到阅读记忆测试。
- 真实目标：最终 APK SHA-256 为 `74a0e0f3d52846278809bd4d011039dd05749680b8b165884c8d9e0e57a4ddf3`，已同包更新到 PCT-AL10；`artifacts/local/audits/pct-reader-install-20260817T130947Z-2495231/` 确认签名、16 KiB 对齐、非降级更新、进程存活和数据保留边界。
- 真实目标：最终 APK 上的未缓存第 10 章冷首轮证据位于 `artifacts/local/audits/pct-reader-unseen-long-section-20260817T131717Z/`。首个长段落从 24 页收敛为 22 页，`22 / 22` 后只出现明确的三点加载状态，下一可见内容属于下一段；随后 14 页收敛为 13 页，并从 `13 / 13` 进入下一段。90 次快速翻页继续跨过后续多个长段落，没有出现页码继续增长的重复尾页或空白页。
- 真实目标：同一录屏按 50ms 间隔抽取 2472 帧；未缓存正文第 232 至 234 帧中公式由忙碌状态转为真实内容，标题、正文和下方段落坐标保持不变，第 235 帧已经开始下一次主动横划。此前功能相同构建的另一未缓存长段落也从 48 页收敛为 43 页，并从 `43 / 43` 进入下一段，证据位于 `artifacts/local/audits/pct-reader-unseen-long-section-20260817T125122Z/`。
- 剩余边界：系统录屏与 ADB 快速手势是真机视觉证据，不等于自然手指观感；最终体验仍由用户直接确认。

# REVIEW-0003：本地 XHTML 分页阅读切片

## 范围

审阅 `SPEC-0002` 与 `PLAN-0002` 定义的 M2：Windows WebView2 阅读 host、受控本地资源、内容隔离、公式缩放、CSS 多栏分页、布局自检、受限遥测和性能记录。

## Diff 摘要

- 根 workspace 新增独立 `atha-reader-host`，以固定版本 Wry/Tao 承载系统 WebView2；
- 后端新增规范化书根、资源边界和非内容遥测消息校验；
- 新增唯一 ATHA 阅读页实现，校验 XHTML/CSS/SVG 后导入闭合 Shadow DOM；
- 固定 1264 × 1680 逻辑页，以 CSS 多栏分页并检查文字、公式和原子内容裁切；
- 行内公式保留来源比例，行间公式使用独立 `1.5` 倍率、覆盖书源边距并按内容列居中；
- 新增实际 Windows host 检查与本地结构化性能记录。

## 已执行检查

- [x] `node --check reader/atha-reader.js` 通过；
- [x] `pwsh -NoProfile -File scripts/check-reader-slice.ps1` 通过：Rust 集成测试 3/3，实际 Windows host 在 24/32/40px 下完成安全、公式和分页自检；
- [x] 10 个独立冷进程与同一 WebView 热路径样本齐全；最近一次中位数为冷启动 1427.612ms、首个稳定页面 547.950ms、热打开 20.700ms、翻页 13.850ms、字号重排 20.800ms；
- [x] 修复前回归稳定报告 `wrong_scale=6`、`off_center=6`；修复后 32px 下 6/6 行间公式为 `3×`、中心偏差 0px、宽高比误差 0；
- [x] Agent Browser 使用仓库同一阅读页完成首尾翻页、返回首页、40px 重排、截图、控制台与请求复核；
- [x] 符合规格；
- [x] 遵循经 `/root/m2_plan_review` 独立交叉审阅为 `approved` 的计划；
- [x] 文档、`ACTIVE` 和代码地图已更新；
- [x] `autocorrect --fix`、`autocorrect --lint`、`scripts/doc_guard.py`、`scripts/doc_length_check.py` 与 `git diff --check` 通过。

## 发现

- 行间公式偏小来自与行内公式共用倍率；偏左来自书源内联 `margin: 0` 覆盖 ATHA 样式。修复集中在共享缩放函数和一条阅读样式规则。
- 书源将行内 `s` 明确写为 `8×8`，小于相邻公式；当前实现按规格保留该相对尺寸，不做单字符特判。
- Agent Browser 的环回 HTTP 链路出现本机 AdGuard 注入请求与 favicon 404；阅读器控制台无错误，CSP 外联探针被阻止，实际产品的自定义协议链路不包含这些验证环境请求。
- Cargo 报告一次非致命 incremental 目录访问拒绝；后续 check、test、构建和实际 host 验收均成功。

## 后续

如跨书源样本证明大量行内公式不可读，应另立规格决定通用最小可读尺寸；不要按字符或 SVG 文件名添加例外。

## 结论

approved。最高证据等级为实际 Windows host 与系统 WebView2 的真实目标证据；未覆盖安装包、CI、跨机器或生产等价环境。用户要求的视觉复核使用应用生成截图完成，未做手动打开应用的交互验收。

## 三样本与夜间模式扩展

- `scripts/export_reader_sample.py` 使用 Python 标准库安全、可重复地导出整页或指定 section；宏观经济学 5.2 与范畴论 5.6 样本已生成，源 EPUB 哈希在导出前后不变；
- `scripts/check-reader-samples.ps1` 一次运行三个实际 Windows host，并由 Agent Browser 对同一阅读页生成明暗截图；
- 逻辑样本为 4 页、154 个公式；宏观经济学样本为 5 页、58 个公式和 1 张普通图；范畴论样本为 4 页、0 个公式、8 个代码块和 2 张普通 PNG；
- 明暗正文对比度分别为 15.94 和 13.84；暗色下公式滤镜为 `invert(0.88) hue-rotate(180deg)`，所有普通图在明暗模式下均为 `filter: none`；
- 三个样本在 24/32/40px 下均无文字、公式或原子内容裁切，浏览器无阅读器错误，外部探针被 CSP 与域白名单阻止；
- 复用回归 `scripts/check-reader-slice.ps1` 通过；最近性能中位数为冷启动 1391.530ms、首个稳定页面 512.400ms、热打开 20.750ms、翻页 13.900ms、字号重排 20.800ms；
- 计划差异经 `/root/m2_plan_review` 补充复审为 `approved`：范畴论源 section 没有公式标记，必须精确断言零公式、至少一个代码块和恰好两张可加载普通 PNG，不能把图示伪装成公式。

扩展结论仍为 approved。最高证据等级为实际 Windows host 的真实目标证据；Agent Browser 提供同代码的明暗视觉与控制台复核。未覆盖安装包、CI、跨机器或生产等价环境。

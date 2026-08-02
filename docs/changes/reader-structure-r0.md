# R0 阅读器结构整理

## Status

implemented

## Problem

当前 Windows 阅读 host 的 `reader/atha-reader-host/src/main.rs` 同时承担参数、窗口、WebView、受控协议、IPC、网络探针和 benchmark；阅读页的 `reader/atha-reader.js` 同时承担内容安全、加载、公式适配、分页、交互、自检和 benchmark。继续在两个入口中增加能力，会使后续多章节、Locator、偏好和交互相互耦合，并重演 Readest 巨型编排器的问题。

## Scope

- 保持 WebView2、受控书根、CSP、固定页面几何、遥测格式和现有交互行为不变；
- 将 Rust 入口缩为启动与组合，把参数和窗口计算、应用与书籍协议、诊断与 benchmark 放入各自 module；
- 将阅读页入口缩为组合，把内容加载与安全校验、分页与公式布局、验证与 benchmark 放入各自 module；
- 为正式浏览器验收提供窄的只读诊断 interface，不再依赖页面脚本的全局内部变量；
- 更新真实资源清单、验证服务器和脚本语法检查，以覆盖拆分后的文件。

## Non-Goals

- 不实现多章节 manifest、Locator、位置持久化、书签、搜索或标注；
- 不改变视觉、控件、分页算法、公式倍率、字号档位、错误码、benchmark 阶段或性能口径；
- 不增加依赖、前端框架、构建器、插件系统、trait、adapter 或为后续阶段预留的空 interface；
- 不借机重写 `BookRoot`、遥测 schema、正式验证脚本或样本内容。

## Acceptance Criteria

- [x] Rust 和页面入口只负责启动、组合与顶层失败处理，不再拥有协议、安全、分页或 benchmark 实现；
- [x] 每个新增 module 只拥有一种现有职责，interface 小于其隐藏的实现，并且没有单实现 adapter；
- [x] 原生 host 继续只提供允许的应用资源和当前书根资源，导航、窗口、下载、权限、脚本与网络策略不放宽；
- [x] 现有 24/32/40px、公式、无裁切、DPI 页面几何、明暗主题、普通图片、代码块和三样本验收继续通过；
- [x] 冷启动、首个稳定页、热打开、翻页和字号重排仍各产生原有数量与格式的 benchmark 记录；
- [x] 代码地图和 `ACTIVE` 与最终 module 结构一致，独立 review 没有 blocking 项。

## Files And Steps

1. 记录现有 Rust、页面和验证脚本的真实调用链，按既有职责确定最少 module；
2. 拆分 Rust host，先通过 fmt、Clippy、测试和构建；
3. 拆分页面脚本并切换 HTML、宿主资源清单和验证服务器；
4. 运行正式 reader slice 与全部样本验收，修复行为或验证 interface 回归；
5. 更新代码地图、本 change 和 `ACTIVE`，完成独立 review。

## Checks

- `cargo fmt --all --check`；
- `cargo clippy --workspace --all-targets --locked -- -D warnings`；
- `cargo test --workspace --all-targets --locked`；
- 拆分后每个 JavaScript module 的 `node --check`；
- `pwsh -NoProfile -File scripts/check-reader-slice.ps1`；
- `pwsh -NoProfile -File scripts/check-reader-samples.ps1`；
- `pwsh -NoProfile -File scripts/Invoke-Atha.ps1 check docs -Activity validation -Scope reader-structure-r0`；
- 本次中文 Markdown 的 `autocorrect --fix` 与 `autocorrect --lint`；
- `git diff --check`。

## Rollback

回滚本 change 的提交即可恢复单文件入口；本变更不迁移数据、不改变书籍内容或外部协议。

## Approval

用户确认新的 reader-only 路线图并明确要求开始实现。R0 是路线图唯一下一项，本 change 以只重组、不增加产品行为为批准范围。

## Result

- Rust `main.rs` 从 664 行降为 13 行，只选择平台入口；Windows 事件循环、启动参数与窗口、应用/书籍协议、诊断与 benchmark 分别进入 `windows.rs`、`launch.rs`、`protocol.rs` 与 `diagnostics.rs`；
- 删除 536 行的单文件阅读脚本，新增 `content.mjs`、`pagination.mjs`、`diagnostics.mjs` 与 79 行的 `app.mjs`；内容安全、分页、验证和组合各自拥有明确 module；
- 应用协议与验证服务器按固定顺序把四份源码交付为一个 `atha-reader.mjs`，保留源码 Locality，同时不增加多次页面脚本请求；
- 正式浏览器检查改用仅在 `verify` 模式暴露的 `__athaReaderDiagnostics.snapshot()`，不再读取页面脚本的全局 `book` 和 `state`；
- 原生 reader slice、三样本明暗验收和 Agent Browser 控件检查均通过；未增加依赖、数据库、产品能力或 adapter。

## Review

- 规格审查确认网络探针、benchmark 样本完整性、重复指标与 cold-start 均由 `diagnostics.rs` 持有，`windows.rs` 只组合事件和处理顶层失败；无 blocking、non-blocking 或 scope creep；
- 标准审查确认页面内容 module 不再读取或修改外部 `fail` 状态，应用资源响应头只在 `protocol.rs` 构造一次；无 blocking 或 non-blocking 项。

## Evidence And Residual Risks

- 最高证据等级：真实目标证据；Windows WebView2 host 完成 10 次冷启动、一次热会话及 24/32/40px、公式、安全、无裁切和固定页面几何自检；
- 三份样本均在实际 host、Agent Browser 明暗主题下通过，普通图片、代码块、公式过滤和正文对比度与既有断言一致；真实浏览器点击下一页、切换 40px、返回上一页成功且无控制台错误；
- R0 最终 10 样本中位数为冷启动 791.483ms、首个稳定页 161.200ms、热打开 20.750ms、翻页 6.200ms、字号重排 20.800ms；指标格式与数量保持有效；
- 冷启动与首个稳定页高于代码地图中的历史最近记录；本轮没有在同一时间运行旧代码对照，不能判断是环境波动还是结构变化。热打开、翻页和重排保持原量级；
- Cargo 多次报告无法最终保存一个 incremental compilation session，检查和构建仍成功；这影响后续增量复用，不影响本次产物正确性。

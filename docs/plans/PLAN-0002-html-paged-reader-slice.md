# PLAN-0002：本地 XHTML 分页阅读切片

## 状态

implemented

## 对应规格

`docs/specs/SPEC-0002-html-paged-reader-slice.md`

## 实施方案

首个运行时采用 `wry 0.56`、`tao 0.36` 与系统 WebView2，放在独立的 Windows host crate。阅读页保持原生 HTML、CSS 和 JavaScript，不引入 Tauri、Tokio、前端框架、产品内本地 HTTP 服务或 HTML 重写器。

建立一份 ATHA 阅读页代码，以本地 XHTML 为内容文档，并由应用控制资源解析和样式注入。实际 Windows 承载与 Agent Browser 必须加载同一份阅读页；后者只自动执行翻页、截图和网络检查，不包含任何临时的渲染规则，也不替代实际应用验收。

WebView2 承载预检已经通过，结果记录在 `ADR-0003`。产品资源使用 `atha` 与 `atha-book` 自定义协议；书籍内容先校验再导入；分页使用固定逻辑页上的原生 CSS 多栏和布局后矩形检查。唯一宿主消息是受限的非内容性能事件。

## 预计改动文件

- `Cargo.toml`
- `Cargo.lock`
- `backend/atha-backend/src/lib.rs`
- `backend/atha-backend/src/reader/mod.rs`
- `backend/atha-backend/src/reader/resources.rs`
- `backend/atha-backend/src/reader/telemetry.rs`
- `backend/atha-backend/tests/reader_slice.rs`
- `reader/atha-reader-host/Cargo.toml`
- `reader/atha-reader-host/src/main.rs`
- `reader/atha-reader.html`
- `reader/atha-reader.css`
- `reader/atha-reader.js`
- `reader/samples.json`
- `scripts/export_reader_sample.py`
- `scripts/Serve-ReaderValidation.ps1`
- `scripts/check-reader-samples.ps1`
- `scripts/check-reader-slice.ps1`
- `docs/ACTIVE.md`
- `docs/INDEX.md`
- `docs/decisions/ADR-0003-webview2-reader-host.md`
- `docs/milestones/M2-html-reader-core-foundation.md`
- `docs/plans/PLAN-0002-html-paged-reader-slice.md`
- `docs/specs/SPEC-0002-html-paged-reader-slice.md`
- `docs/codebase/MAP.md`
- `docs/reviews/REVIEW-0003-html-paged-reader-slice.md`

## 步骤

1. 记录独立交叉审阅；仅在审阅批准或只剩非阻塞建议后，将本计划标记为 `accepted`、M2 标记为 `active`，并把 `ACTIVE` 切换到 `implementation`、明确允许本计划内测试与生产代码修改；此前不修改代码。
2. 采用预检已确认的 Wry/Tao 承载、自定义资源协议、安全策略、公式选择器和 CSS 多栏分页模型；建立独立 Windows host crate，只组装 WebView、协议、导航拦截和受限遥测，非 Windows 构建给出明确的不支持提示。
3. 会话启动时只规范化一次书根。请求路径只解码一次；拒绝非法编码、NUL、绝对路径、盘符、UNC、反斜杠、父目录和未知 MIME。候选文件规范化后必须仍位于规范化书根内，并只读取该规范化后的普通文件；集成测试覆盖明文及编码越界、符号链接越界和书根外文件。
4. 阅读器响应设置拒绝优先的 CSP，书籍响应只向精确阅读器来源返回 CORS；导航、新窗口、下载和外部请求全部拒绝。遥测 IPC 只接受精确阅读器来源、固定事件枚举、固定数值字段、长度及范围限制，不能承载路径、正文或宿主命令。
5. XHTML 必须在 detached `DOMParser` 文档中完成校验后再导入；拒绝脚本、事件属性、表单、框架、主动导航及非书内协议。加载前校验 CSS 的 `@import`/`url()` 和 SVG 的脚本、事件属性及外部引用；负向片段由阅读页自检即时构造，不新增测试框架或夹具目录。
6. 编写 ATHA 原生阅读页代码，在隔离的内容树内建立书源、ATHA 与用户样式的顺序；按 `.math-inline` 实现源尺寸倍率缩放与行内基线对齐；按 `.math-display` 应用独立 `1.5` 显示倍率、覆盖书源内联边距、在内容列中居中并保留超宽约束。
7. 实现固定逻辑页的 CSS 多栏和翻页状态；以实际文字 `Range`、公式和图形矩形检查断页，不搬运 XHTML 字符串或节点，发现裁切时自检非零失败。
8. 性能事件使用单调时钟。冷启动采集 10 个独立新进程，从进程入口到首个稳定页事件；热打开在同一 WebView 已成功访问样本后，从再次打开命令到稳定页，采集 10 次；首个稳定页面从书籍入口请求开始，到边界检查通过且连续两个绘制帧页数与内容矩形不变；首次翻页从翻页处理器入口，到页码和位移更新、边界检查通过后的下一绘制帧；字号重排从字号处理器入口，到边界检查通过且连续两个绘制帧稳定。各阶段保留至少 10 个原始 CSV 样本；中位数对偶数样本取中间两值平均，P95 使用 nearest-rank `ceil(0.95 × n)`。
9. `scripts/check-reader-slice.ps1` 构建并运行实际 `atha-reader-host` 自检模式；公式、分页、资源拒绝、遥测校验或性能样本不足时必须非零退出。另在真实窗口检查首尾页、前后翻页及 24/32/40px 状态。
10. `scripts/Serve-ReaderValidation.ps1` 直接提供仓库中的 `reader/atha-reader.html`、`.css`、`.js`，不得复制、改写或注入渲染规则；`scripts/check-reader-samples.ps1` 只通过启动参数把书籍入口映射到环回验证来源。固定 1264 × 1680 逻辑视口复核首尾页、字号、控制台和请求清单；实际 WebView2 结果仍为权威结论。
11. 写实施评审、更新 `ACTIVE` 与代码地图，运行文档守卫。

## 本轮样本与夜间模式扩展

12. 使用 Python 标准库实现通用 EPUB section 提取器；输入 EPUB、XHTML 入口、可选 section id 和 `fixtures/local/` 输出目录，保留相对路径并只复制直属 CSS 与目标内容引用的资源；禁止 `extractall`，逐项拒绝绝对路径、盘符、UNC、反斜杠、父目录、符号链接和输出根越界；内置最小自检覆盖整页提取、section 边界、资源闭包、恶意 ZIP 项与重复执行。
13. 新增三样本清单；逻辑样本沿用现有目录，宏观经济学使用 `EPUB/text/ch042.xhtml`，范畴论从 `EPUB/text/ch008.xhtml#余积` 截取到该 section 结束。清单只记录仓库相对输出、入口与边界文本，不记录用户 EPUB 绝对路径。
14. 移除阅读页对单一样本公式数量和“1.3”文本的硬编码；清单显式标记样本是否应含公式，逻辑与宏观经济学样本分别断言至少一个公式，并对全部实际公式执行倍率、宽高比、居中和裁切断言；范畴论源 section 没有公式标记，断言零公式、至少一个代码块和恰好两张可加载的普通 PNG，不伪造公式选择器。
15. 使用原生 `prefers-color-scheme` 添加最小夜间样式；只反色 `.math-inline` 与 `.math-display`，普通插图保持原色，不新增主题状态管理或设置界面。
16. 把环回服务、实际 Windows host 三样本自检、Agent Browser 明暗截图、对比度、控制台和请求检查固化到 `scripts/`；环回服务只绑定 `127.0.0.1`、只读，并对规范化仓库根与书根执行路径归属校验；脚本负责启动与停止自己的服务和浏览器会话，并把截图写入忽略的 `artifacts/local/screenshots/`。

## 测试与检查

- Rust 集成测试覆盖非法百分号编码、NUL、绝对/盘符/UNC/反斜杠/父目录、符号链接越界、书根外文件、未知 MIME，以及遥测来源、长度、枚举和数值边界；
- 阅读页自检覆盖 XHTML/CSS/SVG 主动内容、公式选择器、行内倍率、行间 `1.5` 倍率、源宽高比与内容列中心偏差，以及 24/32/40px 分页边界、CSP 外联和稳定判据；
- 公式倍率、宽高比和中心偏差统一在 1264 × 1680 逻辑页坐标中断言，不直接使用受 `--fit-scale` 影响且未归一化的屏幕矩形；实际 Windows host 使用屏幕矩形时必须按当前 fit scale 还原逻辑坐标，或直接读取未受祖先 transform 影响的逻辑尺寸；实际 host 与 Agent Browser 检查所有实际公式，清单标记为含公式的样本零公式时失败；
- `pwsh -NoProfile -File scripts/check-reader-slice.ps1`；
- `python3 scripts/export_reader_sample.py --self-check`；
- 使用 `scripts/export_reader_sample.py` 生成两个新增本地样本，并静态断言标题边界和资源存在；
- `pwsh -NoProfile -File scripts/check-reader-samples.ps1`：一次构建后循环三个样本，在实际 host 验证 24/32/40px，并由 Agent Browser 生成明暗截图、断言正文对比度与页面状态；
- `scripts/check-reader-samples.ps1` 断言暗色下全部且仅公式类图片启用反色，普通图片在明暗模式下始终为 `filter: none`；
- 实际 Windows 承载自检必须以进程退出码报告成败；另做真实窗口交互检查；
- Agent Browser 只加载仓库内同一份阅读页文件，并检查截图、控制台和请求清单；
- 性能基准每项至少 10 次，记录冷启动、热启动、首个稳定页面、首次翻页和字号重排的中位数与 P95；
- 检查性能基准与运行日志分离，且都只含非内容性的时间、版本、样本稳定标识、页面参数、冷/热状态与错误类别，不含书籍或读者内容；
- `python3 scripts/doc_guard.py`、`python3 scripts/doc_length_check.py`、`git diff --check`。

## 回滚方案

新阅读模块和依赖保持在独立提交中；回滚该提交即可恢复 M1 空后端，不触碰本地样本或原 EPUB。

## 风险

- Wry 或 WebView2 升级可能改变自定义协议和布局行为；
- 行盒测量与最终运行时实现可能有差异；
- 本地样本不入库，CI 不能直接覆盖其视觉验收。

## 必需文档同步

- `docs/ACTIVE.md`
- `docs/reviews/REVIEW-0003-html-paged-reader-slice.md`
- 必要时更新代码地图或 schema：`docs/codebase/MAP.md`
- 决策：`docs/decisions/ADR-0003-webview2-reader-host.md`

## 交叉审阅结果

- Reviewer：`/root/m2_plan_review`。
- 状态：`approved`；本轮首次复审的三项阻塞和范畴论非公式样本的非空验收均已修正并通过复审。
- 阻塞问题：无；首次阻塞为旧临时脚本依赖、ZIP/环回安全边界与零公式空通过。
- 非阻塞建议：首次复审建议仍有效。
- 必须修改：无。

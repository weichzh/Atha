---
description: Atha 书架、阅读记忆和阅读工作区的统一 UI 重设计与本地验收。
---

# UI 重设计

## Status

implemented

## Problem

用户明确要求重新设计并实现，而不只是避免既有界面出错。现有书架把导航、搜索、标题和排序纵向堆叠，阅读记忆缺少主次，笔记索引截断原文，阅读工具和应用界面没有一致的视觉语言。

## Scope

- 用户本轮批准 UI 实施，并明确允许接管相关 UI 和文档、保留既有改动。接管已取消的文档任务的 `ACTIVE` 指针；在既有阅读 CSS 和移动 UI 文档改动上继续，不覆盖前轮结果。
- 重设计生产书架导航、搜索与操作层级、书封网格与列表、应用设置、阅读记忆，以及阅读工具、笔记索引、对话和输入器的视觉层级。
- 用户续做明确指出阅读主界面未完成；继续同一 task 和 change，重设计正文页的章节 / 进度层级、工具条、目录 / 搜索 / 进度面板，并将字号与阅读主题前移至阅读设置首页。保留前轮书架设计；不把只调整笔记样式视为阅读界面完成。
- 采用白色 / 石墨灰、松绿色强调、细分隔线、衬线书籍内容和无衬线工具文字；桌面侧边导航、窄屏底部导航。保留真实书封，不把生成封面写入用户资料库。
- 保留 `P1-P3`、`G0/G1`、`C1-C3`；不改书籍渲染、领域数据、IPC、导入、删除、备份、引用和持久化逻辑，不新增依赖或生产演示数据。
- `H1-H4` 仍是未决假设；本轮只交付具体视觉选择，不宣称长期用户价值或跨平台最优。

## Architecture Impact

none

复用现有 Svelte、Lucide、CSS、WebView、MessageStore 与正式验收入口。只修改既有 UI 投影，不建立第二套主题 / 状态 / 组件框架。

## Acceptance Criteria

- `UI-01`：书架、阅读记忆、设置和笔记使用统一视觉系统；桌面导航与内容分区，360px 时主导航、导入、搜索、排序与选择均可达。
- `UI-02`：网格 / 列表、搜索、选择 / 取消、删除取消、主题写入 / 重载 / 恢复继续工作；书封完整显示，长标题不能撑开网格。
- `UI-03`：阅读器保留 `A`、`A|B`、`B|C`、`A|B|C` 和窄屏 `B→C→B`；原文、自己的回应和引用关系可以区分，输入器可见，主题作用域不串写。
- `UI-04`：本轮主要触控动作是应用导航、导入、网格 / 列表、选择、笔记范围、回复 / 更多、发送 / 返回；目标至少 44 CSS px。键盘焦点可见，横向无意外溢出，支持减少动画。
- `UI-05`：执行 Svelte check / build、领域现有测试、可运行 UI 检查、文档 gate 和 Linux 正式 GUI gate；记录未通过或未覆盖项。截图与交互证据分开。
- `UI-06`：真实阅读页展示正文、当前章节和紧凑工具条；目录跳转、搜索跳转、前后页、进度拖动和书签仍可用；字号 / 主题在阅读设置首页直接可达，保存 / 重开 / 恢复及 Locator 稳定通过本地原生验证。360 / 1000 / 1600 px 下正文与工具不意外重叠。

## Files And Steps

1. 以 Image Gen 生成桌面书架、三栏阅读、移动和设置参照，保存在忽略的本机证据目录；审阅并记录偏离。
2. 修改 `reader/theme.css`、`reader/atha-reader.css`、`reader/app/src/library.css`、`shell.css` 和相关现有 Svelte 组件，不触碰后端既有改动。
3. 在 `reader/app/tests/ui-redesign.test.mjs` 保留本地可重跑检查，更新 `docs/codebase/READER-MOBILE-UI.md` 和 `ACTIVE` 指针。
4. 执行检查、截图对照和独立只读 review；未授权提交、推送、安装或发布。

## Iteration Control

- Mode: `iterative`
- Maximum repair iterations: `3`
- Unchanged-delta limit: `2`
- Human stop: 需要更改数据、产品合同或真实设备写入时停止并确认；平台不可用时保留未执行。
- 本次用户要求的阅读界面续做重新计数最多 3 轮；下方原 iteration 1-3 为书架阶段历史，不作为阅读主界面完成证据。

## Checks

- Svelte check：0 errors / 0 warnings；Vite production build 通过。
- Node：书架、词典、Markdown、阅读状态、标注和对话共 24 项通过。
- `mise exec node@24.17.0 -- node reader/app/tests/ui-redesign.test.mjs http://127.0.0.1:1422 http://127.0.0.1:1423`：第一个 origin 检查书库 mock IPC；第二个检查 production build 的真实阅读运行时，覆盖 360 / 1000 / 1600 px 章节 / 进度同步、设置首页、目录、搜索结果、进度面板、输入目标与页面错误。重跑前执行 `pnpm --dir reader/app build`，创建 `reader/app/dist/tests` 并复制 `reader/app/tests/reader-design.xhtml` 到该目录，再以 `pnpm --dir reader/app exec vite preview --port 1423` 启动预览；1422 是原 Vite 服务。正式包不包含 fixture。
- `bash scripts/check-reader-linux.sh`：Linux Tauri / WebKitGTK 正式脚本通过，覆盖 360 / 600 / 1000 / 1280 / 1600 px 工作区、原书跳转、历史引用、设置作用域、输入器避让、Locator 恢复、13 种鼠标场景与 AppLog 隐私；公共书籍首次导入封面实际加载、完整缩放和加载失败后的书名回退通过。
- 阅读续做：默认 5 次预热 / 20 次测量的完整 Linux gate 通过，共 220 次手势测量；最终搜索输入和章节回退微调使用 `--gesture-warmups 1 --gesture-measurements 1` 重跑同一完整脚本。新增 `verify_reader_redesign` 覆盖 360 / 1000 / 1600 px 顶栏同步、44 px 工具目标、不遮正文、设置首页和偏好写入 / 重开 / 原记录恢复；低采样不作为新的性能统计结论。
- 文档：本次触碰的三个中文 Markdown 经 `autocorrect --fix` / `--lint`；仓库未配置额外 Markdown formatter，使用项目 `docs` gate 检查结构和长度。`docs` gate 与 `git diff --check` 通过。

## Result

阅读界面续做候选已实现：无框正文、同步章节顶栏、紧凑工具条、目录 / 搜索 / 进度面板，以及阅读设置首页字号 / 主题。真实书籍排版、分页、Locator 与持久化入口保留；下列先保留书架阶段记录，阅读续做单独列出。

生产书架、阅读记忆、应用设置、阅读工具、笔记索引和对话已采用新视觉。生成图只作为视觉参照，生成器误加的图片附件、错误书名 / 作者和缺失默认排序不进入生产；保留实际操作与数据，不逐字复制合成内容。

- iteration 1：review findings 为颜色字面量绕过中央 token、正式脚本仍断言旧主题 RGB；repair 为颜色回归中央 token，并更新真实调色板断言；validation cases 为 Node 24 项、check / build、跨视口浏览器和 Linux GUI gate；remaining_delta 为独立 review 与文档收尾；evidence level 为本地浏览器和 Linux Tauri GUI；stop reason 为进入独立 review。
- iteration 2：review findings 为浏览器偏好 sentinel 缺少生产 schema、测试启动注释不能复现，以及封面可见性证据不足；repair 为使用完整非默认偏好记录、独立断言应用主题 key、写明可运行命令，并加强原生封面检查。文档 gate 首次因 `ACTIVE` 箭头格式拒绝，已按既有解析格式修复；validation cases 为完整 Linux GUI gate、三视口浏览器重跑、文档 gate；remaining_delta 为零个本轮阻塞项；evidence level 同上；stop reason 为实现和本地验收完成，保留未提交交接与真实目标未覆盖项。
- iteration 3：review findings 为截图捕获了工具层 120ms 淡入过程；repair 仅让公共截图等待工具层完全显现，并采用结构化轮询结果，消除初版布尔返回造成的 jq 重试警告，不改产品动画。validation cases 为同一候选默认采样完整 GUI gate，以及修正轮询返回后的 `--application target/debug/atha-reader-app --gesture-warmups 1 --gesture-measurements 1` 截图回归；两次均通过。remaining_delta 为零；evidence level 同上；stop reason 为稳定态截图复核完成，结束本轮。
- 生成图和实际截图已通过 `view_image` 对照：桌面侧栏与六列书架、窄屏三列与底部导航、主题色样、原文细线与回应层级、笔记索引、多栏边界、固定输入器均已核对。实际无封面资料显示书名回退，不用生成图冒充用户书封；不同平台使用本机字体回退。
- 实际保留 320 px 笔记索引与正式工作区断点，不照搬生成图的比例；移动对话继续使用已有 12 px 边界，而非引入半屏拖动窗口。章节筛选、默认排序、完整词典设置和输入器折叠工具没有因视觉简化被删除。

### 阅读主界面续做

- iteration 1：真实预览发现顶栏初次打开为空，修复为 Navigation 的 `syncControls` 同时更新原面板和新增顶栏，覆盖初次绑定与后续导航，不增加状态。Vite 开发 CSS 模块不能充当正文读取的原始 CSS，浏览器阅读证据改用 production preview；未修改生产加载边界。
- iteration 2：新原生检查错用未许可的 360 × 900 启动尺寸，改用现有 360 / 1000 × 760、1600 × 900；修正布尔轮询引起的 jq 告警。浏览器检查改用移动面板关闭按钮、桌面既有工具切换，并等待动画与分页稳定。真实 Linux 字号 / 深色主题写入、刷新保持、原记录恢复（包括缺失记录）通过；remaining_delta 为独立审查和最终候选回归。
- iteration 3：Standards 指出搜索输入未显式保证 44 px、文档命令遗漏 reader origin。输入增加最小高度与断言，记录双 origin 命令；桌面搜索标题沿用目录的无返回按钮布局，窄屏保留返回。Spec 指出前后页、进度和书签缺少控件链路证据，原生脚本补齐三视口真实 WebDriver 鼠标点击 / 拖动和状态还原，不用诊断 API 代替控件。最终结果与审查以下方收据为准。
- 最终 validation cases 为三视口浏览器、24 项 Node、Svelte check / build、文档 gate，以及新增控件链路后的低采样 Linux 全脚本，均通过；remaining_delta 为零个本轮阻塞项，evidence level 为本地 Chromium 与 Linux Tauri GUI，stop reason 为实现 / 验证完成，等待用户复核。最后一次扩充脚本时编辑了运行中的 Bash 文件，造成读取偏移失败；固定文件后从头重跑通过，该失败保留在工站记录，不视为产品故障或有效验收。
- 参照 `reader-main-concept.png` 与实际 `reader-main-1600.png`、`reader-quick-360.png` 已同次查看对照。布局：保持无框、居中正文，工具条从全宽收为 400 px；字体：工具文字紧凑，书源字体与章节排版不强行覆盖；颜色：白色 / 石墨与绿色选中态，阅读纸张主题仍可选；层级：字号与四主题在首屏，分类两列；图标：使用既有 Lucide 和原生 range；密度：移动首屏可见全部分类，长页仍独立滚动。
- 有意偏离参照：不用生成的书名、正文、统计、假 iOS 状态栏或虚构模块数；移除生成图的设置嵌套卡片和工具文字标签。页码放在顶栏第二行而非工具条上方，避免高 DPR 正文安全区重叠。参照是 1586 × 992 的双设备合成图，实际按 360 / 1000 / 1600 px 分别验收，不伪称逐像素复刻。浏览器散文是独立测试内容；章节优先读取真实目录名，缺少名称时显示节序号，不向用户暴露 `entry` 或内部 section ID。

## Review

以 `ae7287d08c8f0d92ec4430cef9e3b1ea3230c217` 为基线，对本轮声明的工作树范围执行独立只读 Standards 与 Spec review。两位 reviewer 分别复核修复后，当前两轴均无遗留 finding，未发现 P0 / P1。主 agent 复核审查结论与实际检查输出；不把继承的后端改动误归本轮，也不把本地结果扩大为跨平台验收。

## Evidence And Residual Risks

设计参照不等于实现或验收。浏览器截图和参照位于忽略目录 `artifacts/local/ui-redesign-20260905/`；Linux 公共 fixture 最终稳定态截图位于 `artifacts/local/reader-linux-ui/atha-reader-linux-1819364-27142-1/`。完整默认采样记录对应 `atha-reader-linux-1766569-554-1`；最终微调后的全脚本回归降低手势采样量，但所有 UI 断言保留。原生脚本对私有公式输入不保留截图。

最高证据等级为当前 Linux 主机的原生 Tauri GUI 自动化。Windows、Android、自然触摸、真实 IME、屏幕阅读器和长期使用尚未执行；合成键盘视口不是自然输入法验收。既有后端与其他未提交改动保留，本轮不提交、推送、安装或发布；工作流要求提交收据且不允许 waiver，因此只能留交接，不能把未提交候选标为正式 closed。

---
description: 笔记、标注、对话与消息的用户术语契约、界面统一和验收记录。
---

# 笔记对话用户术语

## Status

implemented

## Problem

Atha 已经把笔记呈现为稳定的列表与消息线程，但界面仍混用“笔记”、“标注”、“标记”、“阅读对话”、“聊天记录”和“消息”。这些词并非同义：

- “标记”没有对应的产品对象，却出现在范围切换、计数、空态和辅助文本中；
- “阅读对话”重复说明当前所在场景，不如即时通讯界面的“对话”直接；
- “本条标记 / 本章标记 / 本书标记”同时混淆当前线程和聚合范围，在窄屏也占据不必要宽度；
- 笔记列表底部的“笔记与标注”、阅读记忆中的“跨书消息”和存储中的“消息与快照”分别从实现层命名同一批用户数据，用户需要反复翻译。

本切片只统一用户可见语言，不把内部 `Message`、`Conversation`、Annotation adapter 或数据库术语改名。

## Terminology Contract

- `笔记`：阅读器中的功能入口和用户写下的阅读反应；也是列表的主名称；
- `标注`：只选中并高亮原文、尚未写入正文的记录；它是笔记列表中的一种状态；
- `对话`：从一段原文开始的当前线程，或本章 / 本书中的线程集合；
- `消息`：对话中的一条正文或回复，只在消息级操作、时间和数量中使用；
- `原文引用` / `历史引用`：当前可跳回的原文上下文与创建时存档；
- 范围切换只显示 `当前`、`本章`、`本书`，由控件名称“对话范围”补足语义；
- 用户界面不再使用 `标记`、`阅读对话`、`聊天记录`、`标注消息` 或裸露的技术名 `Message`。

## Scope

- 更新笔记列表的搜索、辅助名称、计数和删除动作，让“笔记”作为入口、“标注”作为无正文状态保持稳定；
- 更新线程标题、范围切换、聚合摘要、空态、加载 / 错误和返回辅助名称；从笔记列表进入线程时返回按钮明确为“返回笔记”，从正文或阅读记忆直接进入时仍为“返回阅读”；
- 将阅读记忆的跨书入口改为搜索“笔记与回复”，将资料库恢复、删除与占用中的技术“消息”改为用户能识别的笔记 / 对话语言；
- 同步无框架 HTML fallback、Svelte 组件、正式 Linux 门、GUI 截图和事实所有者；
- 保持当前视觉布局、Message 数据、线程查询、输入器、词典和 CSS 模块不变。

## Non-goals

- 不改内部类型、变量、DOM id、data attribute、IPC、数据库、导出格式或日志 token；
- 不把标注和笔记合并成同一操作，也不删除纯标注记录；
- 不新增动态作者、头像、AI 角色、未读状态、对话重命名或用户术语设置；
- 不重做阅读记忆页面、消息编辑器、资料库维护流程或响应式分栏；
- 不把文档中的领域术语 `Message` / `Conversation` 替换为模糊中文。

## Architecture Impact

none

产品文案只投影既有状态，不改变 Module 所有权、Interface、数据、信任边界、依赖或运行拓扑。若未来作者 / AI 角色进入正式 Message 数据，再用新的 change 设计参与者名称和气泡身份。

## Acceptance Criteria

- `TERM-CONTRACT-01`：用户界面能稳定区分笔记功能、纯标注状态、对话线程、消息和原文引用；生产界面不再出现“标记”、“阅读对话”、“聊天记录”或“标注消息”。
- `TERM-NOTES-02`：笔记列表标题仍为“笔记”，搜索为“搜索笔记或标注”，列表辅助名称为“笔记和标注列表”，无筛选状态只显示“共 N 条”；笔记和纯标注的类型标签、编辑与删除名称准确。
- `TERM-THREAD-03`：线程标题为“对话”，范围为“当前 / 本章 / 本书”；本章 / 本书摘要使用“X 个对话 · Y 条消息”，加载、空态和错误也使用“对话”。
- `TERM-BACK-04`：从笔记列表进入线程时，返回按钮的可访问名称和 tooltip 为“返回笔记”；直接从正文或阅读记忆进入时为“返回阅读”。
- `TERM-MEMORY-05`：阅读记忆使用“跨书笔记”、“搜索笔记与回复”和“正在搜索笔记”；资料库恢复、删除和存储占用不再向用户暴露技术“消息”分类。
- `TERM-LAYOUT-06`：360、1000、1280 和 1600 px 的新文案不溢出、不遮挡；`A+B`、`B+C`、`A+B+C` 与竖屏 `B → C → B` 几何不变。
- `TERM-REGRESSION-07`：消息查询、范围、排序、回复、编辑、删除、历史、跳回、导出、Locator、输入可见性和 AppLog 隐私的既有 Linux 门通过。
- `TERM-EVIDENCE-08`：最新 Linux Tauri 截图与 DOM 文案断言对应同一候选，`design-qa.md` 没有未关闭 P0 / P1 / P2。

## Iteration Control

- Mode: `iterative`
- Maximum repair iterations: `3`
- Unchanged-delta limit: `1`
- Human stop: 只能通过新增消息参与者 / 身份数据、重新定义标注与笔记产品边界或真实辅助技术判断才能判定。
- Per-iteration receipt: change 的 `Result` 与 `Evidence And Residual Risks`；完整命令和输出由 `project-workflow` 工站保存。

## Files And Steps

1. 按术语契约替换 Svelte、HTML fallback 与 reader module 的用户可见文案，不改内部标识符。
2. 让线程返回辅助名称根据现有返回目标切换，不增加状态。
3. 扩充 Linux 门，直接断言列表、线程、范围、聚合摘要、返回和资料库分类文案，同时保留现有几何与 Locator 门。
4. 采集相同状态 GUI，检查窄屏容纳、层级和无技术词泄漏。
5. 更新事实所有者、Design QA 和 review 收据。

## Checks

- `mise exec -- pnpm --dir reader/app check`；
- `mise exec -- pnpm --dir reader/app build`；
- `mise exec -- node --test reader/web/conversations.test.mjs`；
- `bash scripts/check-reader-linux.sh --gesture-warmups 0 --gesture-measurements 1` 作为迭代门；
- `bash scripts/check-reader-linux.sh` 作为最终 Linux 真壳门；
- 360、1000、1280 和 1600 px 当前截图与 DOM 文案 / 几何；
- 本次中文 Markdown 的 AutoCorrect 与 `git diff --check`；
- `project_workflow.py station note-conversation-terminology-20260901 --activity verification --gate docs`。

## Result

### Iteration 1

- `iteration`：`1`；
- `review findings`：现有界面把“标记”、“阅读对话”、“聊天记录”和技术“消息”混入笔记主循环；初版返回名称又用可见列表项推断来源；
- `repair`：按契约统一笔记列表、当前线程、聚合范围、阅读记忆和资料库分类，并让返回名称复用现有焦点目标；
- `validation cases`：Svelte check / build、Conversation Node 测试和静态检查通过；首次精简 Linux 门准确发现阅读记忆深链错误显示“返回笔记”；
- `remaining_delta`：返回名称仍把几何可见性误当导航来源，必须区分已有深链校验路径；
- `evidence level`：静态 / 构建 + Linux Tauri / WebKitGTK 真壳；
- `stop reason`：存在当前范围内可修复的失败，进入第二轮。

### Iteration 2

- `iteration`：`2`；
- `review findings`：阅读记忆已有非空 `expectedRootId` 作为深链根消息校验事实，不需要新增来源状态或回调；
- `repair`：深链使用该既有参数明确返回阅读，普通入口仍按可见的笔记行返回列表；同步事实所有者、正式门和 Design QA；
- `validation cases`：精简门和完整 `scripts/check-reader-linux.sh` 均通过；完整门覆盖 360 / 600 / 1000 / 1280 / 1600 px、13 个手势场景、220 次测量、两类返回名称、对话范围 / 聚合、Locator、输入可见性和 AppLog 隐私。Svelte check / build、Node 测试、AutoCorrect、文档门和 `git diff --check` 通过；
- `remaining_delta`：实现与验收范围内无 blocking 差异；候选提交后的两轴 review、committed-HEAD required gate 和工作流收据属于收尾；
- `evidence level`：静态 / 构建 + Linux Tauri / WebKitGTK 真壳自动化 + 当前 GUI / 同图 Design QA；
- `stop reason`：第二轮清空首次实现差异，进入候选两轴 review。

### Iteration 3

- `iteration`：`3`；
- `review findings`：Spec review 发现横屏笔记栏已打开时从正文新建笔记仍会按选中新列表行误判返回来源；Standards 与 Spec review 同时发现 `READER-CORE` 保留“跨书消息 / 消息与快照”，GUI 证据元数据也早于候选提交；
- `repair`：正文入口复用已有根消息校验参数声明直接来源，关闭动作按已经确定的返回语义恢复正文或列表焦点，线程内切换保持原返回目标；同步 `READER-CORE`，并让 Linux 门验证阅读记忆关闭后真实返回正文焦点；
- `validation cases`：Svelte check / build、Conversation Node 测试、脚本语法、精简与完整 Linux Tauri 门通过；完整门继续覆盖两类返回名称，并新增阅读记忆关闭后的正文焦点断言，13 个场景和 220 次测量通过；
- `remaining_delta`：实现、规范和候选证据均无 blocking 差异；required gate 与工作流关闭收据属于收尾；
- `evidence level`：静态 / 构建 + Linux Tauri / WebKitGTK 真壳自动化；
- `stop reason`：第三轮修复当前 review 差异并达到修复上限，停止 Repair。

## Review

- Standards：独立复核 `2db459f...e9cbc36`，先前 `READER-CORE` 事实所有者遗漏已关闭；return-target 修复复用已有根消息校验与焦点目标，没有发现 blocking 或 non-blocking 代码标准问题。
- Spec：独立复核 `TERM-CONTRACT-01` 至 `TERM-EVIDENCE-08`；正文直接进入返回正文、笔记列表进入返回列表、阅读记忆进入返回正文三条链路成立，术语、布局、回归与事实同步没有 blocking 或 non-blocking 差异。
- Evidence：修复后的完整 Linux 门通过；候选 `e9cbc36` 已重新采集四种视口、宽屏三栏与竖屏两页截图，`uiux-evidence.json` 的 commit 与候选一致。
- Residual：Windows、PCT-AL10、自然触控、真实输入法、真实屏幕阅读器和大型长期笔记库属于已声明的平台 / 人工边界，不阻塞本切片。

## Evidence And Residual Risks

- 静态 / 构建：`pnpm --dir reader/app check` 为 0 errors / 0 warnings，Vite build 与 `node --test reader/web/conversations.test.mjs` 通过；生产 reader 与 Linux 门中未再检出旧用户术语；
- Linux Tauri 真壳：精简门和完整门均通过，候选环境为 WebKitGTK 2.52.6、WebDriver browserVersion 0.55.1，完整门执行 13 个场景和 220 次测量；新文案在全部五种视口无溢出，`A+B`、`B+C`、`A+B+C` 与 `B → C → B` 的 Locator 和几何不变，列表与阅读记忆返回焦点均通过；
- GUI / Design QA：候选 `e9cbc36` 的截图、DOM 指标、Telegram 参考并置图与 `uiux-evidence.json` 位于 Git 忽略目录 `artifacts/local/audits/atha-note-terminology-20260901/`；没有未关闭 P0 / P1 / P2；
- 未覆盖：Windows 与 PCT-AL10 当前候选、自然手指、真实输入法、真实屏幕阅读器和大量长期笔记库。Linux WebDriver 的鼠标自动化与 DOM 名称不替代这些平台 / 人工证据。

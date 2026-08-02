# 通用流程工站 CLI 研究

## 问题

ATHA 目前依赖项目工作流技能、文档约定和零散脚本控制开发节奏。它们能描述流程，却不能统一记录一次工作经过了哪些阶段、在哪里等待、哪些检查实际运行、哪些重复失败，以及哪些步骤真正拖慢了交付。

translator 的 CLI 工站思路证明：把过程变成可观察的本机记录，能够让“先做什么、检查什么、何时停止”成为工具行为，而不是仅靠会话记忆。ATHA 不需要翻译 CLI，但需要一个不携带领域语义的流程工站工具。

## 目标

建立一个可复用的本机 CLI 核心，供 ATHA 和未来项目使用。它应当：

- 显式开始、切换、完成或中止一个工作工站；
- 记录任务类别、阶段、结果、相邻工站、耗时、声明的人工/Agent 活动和非内容性文件变化；
- 把项目自己的检查、构建和 benchmark 作为可记录的步骤，而不是重写它们；
- 聚合成功率、失败、等待、重复工作、median、P95 和异常慢样本；
- 提示流程摩擦，形成候选改进，不自动修改项目、文档或代码；
- 不记录正文、提示词、书名、用户数据、绝对路径、完整命令、密钥或对话内容。

## 非目标

- 不取代用户批准、产品判断、代码审阅或项目测试；
- 不建立远程遥测、账号、数据库、仪表盘或后台常驻服务；
- 不把 ATHA 的阅读器、书籍、消息、样本或性能语义写死在通用核心中；
- 不根据单次耗时自动改变流程或宣布性能回归；
- 不把工站经过时间伪称为精确人工专注时间。

## 设计原则

### 通用核心，项目适配

核心只理解 `run`、`station`、`phase`、`activity`、`check`、`result` 和本地日志。ATHA 通过很小的项目配置声明合法任务类别、阶段、正式检查入口和哪些变更需要活动 change；未来项目可以换自己的配置，而不是复制 ATHA 工作流。

### 工站是事件，不是状态机替身

工站记录发生过什么，例如 `research → design → implementation → validation`。它可以拒绝缺失的必填字段或不合法阶段，但不替代 `docs/changes/*.md` 对范围、验收和批准的判断，也不能在没有用户批准时让代码进入实现。

### 证据与内容分离

日志保存可比较的元数据：UTC 时间、稳定任务类别、阶段、成功/失败、持续时间、相邻工站引用、文件相对状态摘要和检查名。真正的产品证据仍由测试输出、截图、benchmark 原始样本和对应 change 保存。

### 先观察，再优化

连续失败、长时间等待、重复人工活动或性能长尾只能产生候选。候选必须进入研究或 accepted change，经过代表样本复测后才改变流程。

## 最小命令面

```text
workflow station start --task reader-engine-research --phase research --activity investigation
workflow station begin --run RUN --phase validation --activity none
workflow station check --run RUN --name reader-samples --result success
workflow station end --run RUN --phase validation --result success
workflow station finish --run RUN --result success
workflow benchmark --task reader-engine-research
```

命令名和参数只是研究草案。第一个实现必须优先复用 ATHA 现有 `Measure-Workflow.ps1` 的本机 JSONL 思路，不能为命令外观重建另一套日志存储。

## ATHA 的首个适配

ATHA 首先只登记：

- 文档系统升级；
- 组合式阅读引擎 A 组预检；
- WebView2 三样本验收；
- 后续已接受 change 的 implementation、validation 和 review。

阅读器 benchmark 继续由已有正式脚本生成原始性能数据；流程 CLI 只引用该检查结果和耗时，不吸收书籍内容或替代性能定义。

## 准入问题

在实现前必须回答：

1. ATHA 的 `Measure-Workflow.ps1` 能否收敛为通用核心，还是应先提取一个与仓库无关的小工具？
2. 项目适配使用静态配置文件还是 CLI 注册，怎样避免引入复杂 DSL？
3. 什么最小文件摘要既能识别范围外变化，又不记录内容或绝对路径？
4. 哪些阈值只提示候选，哪些情况必须阻止后续阶段？
5. 怎样让未来项目复用，而不让 ATHA 为抽象层付出额外维护成本？

## 建议的验证顺序

1. 用现有 ATHA 流程日志做只读兼容性探针；
2. 在组合式阅读引擎 A 组预检中手动记录一次完整工站链；
3. 确认日志能发现等待、失败和重复步骤，但不泄露内容；
4. 在至少五个可比样本后验证 benchmark 聚合；
5. 只有这些通过，才起草通用 CLI 的 accepted change。

## 当前结论

这是可复用的工具方向，但尚不是要立即实现的通用框架。ATHA 应先以现有脚本和一个真实 change 验证最小工站模型；若模型不能降低协调成本，就不继续抽象或推广到其他项目。

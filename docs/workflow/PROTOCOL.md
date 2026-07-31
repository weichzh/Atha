# 文档驱动协作协议

## 目的

本协议使仓库中的协作可恢复、可审阅、可机械检查：

- `AGENTS.md` 定义启动顺序和门禁；
- `docs/ACTIVE.md` 保存短期状态；
- `docs/INDEX.md` 映射长期记忆；
- 规格定义必须满足什么；
- 计划定义怎样实施；
- 评审记录实施是否符合规格和计划；
- 守卫脚本检查文档同步和长度。

## 启动

每次会话按以下顺序读取：

1. `docs/ACTIVE.md`；
2. `ACTIVE` 信息不足时读取 `docs/INDEX.md`；
3. 当前任务明确需要的里程碑、规格、计划、决策、架构或 schema 文档。

禁止先读完整文档树。

## 模式

- `discussion`：讨论、比较、记录和提出规格；禁止生产代码修改。
- `specification`：起草、自审和接受规格；禁止生产代码修改。
- `planning`：编写计划、测试、回滚并请求交叉审阅；禁止生产代码修改。
- `implementation`：仅在全部代码门禁通过时实施。
- `review`：检查 diff、运行验证和记录结论；禁止计划外生产修改。

## 规格规则

规格必须说明问题、目标、非目标、用户可见行为、内部行为、验收标准、边界情况、风险和关联文档。

接受前必须完成自审，明确歧义、缺失的验收标准、范围问题和风险。状态只使用 `draft`、`self-reviewed`、`accepted` 或 `superseded`。

## 计划规则

计划必须引用规格，列出实施方案、预计改动文件、步骤、测试、回滚、风险、文档同步和交叉审阅结果。

实施前必须由独立 reviewer 或子 agent 审阅。状态只使用 `draft`、`cross-reviewed`、`accepted`、`implemented` 或 `superseded`。

## 实施门禁

以下条件缺一不可：

- active 里程碑；
- accepted 规格；
- accepted 计划；
- 已通过交叉审阅；
- `ACTIVE` 允许修改生产代码；
- 用户已批准实施范围。

实施后必须运行验证、更新 `ACTIVE`、评审记录和结构文档，并运行文档守卫。

## 收尾

结束前更新 `docs/ACTIVE.md`：当前模式和里程碑、任务、允许动作、下一会话上下文、已知状态、下一动作、触碰文件和检查结果。

## 机械检查

```powershell
python scripts/doc_guard.py
python scripts/doc_length_check.py
```

## 长度限制

- `docs/ACTIVE.md`：最多 150 行；
- `docs/INDEX.md`：最多 250 行；
- 其他 `docs/**/*.md`：默认最多 400 行。

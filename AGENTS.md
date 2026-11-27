# AGENTS.md

## 启动

每次会话依次读取：

1. `AGENTS.md`；
2. `CONTEXT.md`；
3. `docs/ACTIVE.md`；
4. `ACTIVE` 指向的相关 Context Bundle；没有合适入口时，从 [文档索引](docs/INDEX.md#context-bundles) 选一个匹配的 Bundle。

## 仓库工作流

- 新修改采用 GitHub Flow：在短期分支上工作，验证并经 PR 审查后合入 `main`。
- 开始、恢复或关闭任务前，读[仓库协作规则](docs/agents/workflow.md)。跨会话工作同时读关联 Issue 及最新交接。
- 按改动范围运行仓库检查脚本。只有确需项目专属流程时才在 `.agents/skills/` 增加 skill。

## 当前文档

- `CONTEXT.md` 定义稳定目标、目录边界和事实所有权；
- `ACTIVE.md` 指向当前工作的入口；
- `INDEX.md` 是当前文档注册表和 Context Bundle；
- 产品、架构、代码地图和契约各自只维护自己的事实；
- `docs/changes/*.md` 是新跨模块变更的唯一记录；已关闭的记录由 Git 保存；
- `docs/research/*.md` 保存当前未决问题的研究；将形成的结论更新到对应事实所有者。

## 工作路径

- `audit`：有明确对象和停止条件的只读调查；
- `fast`：局部、可回滚的维护，维持既有依赖、接口、数据、安全与信任边界；
- `change`：产品行为、依赖、架构、数据、验收或跨模块实现变化。实施前需有一份 `accepted` change 和用户对范围的批准。

在同一份 change 中记录问题、范围、验收、文件与步骤、检查、结果和 review。

## 事实与验证

- 不可信书籍默认拒绝脚本、网络、路径越界和未知资源；
- 当前阅读渲染统一使用平台 WebView：Windows 使用 WebView2，Linux 使用 WebKitGTK，Android 使用 system WebView；外部独立引擎达到浏览器兼容、选择与性能门槛后才重新研究；
- PCT-AL10 真机检查与安装按[项目流程](docs/agents/references.md#pct-al10-真机与-adb-取证)执行；
- 按实际执行环境记录验证结果，区分静态检查、本地检查、真实目标与生产等价证据。

## 收尾

更新受影响的事实所有者，按需调整 `ACTIVE` 指针，并运行相称检查。推送、合并和发布分别按用户授权执行。

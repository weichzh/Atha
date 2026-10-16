# AGENTS.md

## 启动

每次会话依次读取：

1. `AGENTS.md`；
2. `CONTEXT.md`；
3. `docs/ACTIVE.md`；
4. `ACTIVE` 指定的 Context Bundle；只有信息不足时才读 `docs/INDEX.md`。

不得默认通读 `docs/` 或旧档案。

## Issue 主导的工作流

长期工作、跨会话研究、涉及真实书库/设备验收及需长期恢复的质量问题，都以 GitHub Issue 为目标、进度、验收与下一步的事实来源；项目总览见 [项目总控](https://github.com/users/weichzh/projects/1)。具体规则见 `docs/agents/issue-tracker.md`。

默认按仓库现有 `audit` / `fast` / `change` 路径执行，不启用全局 `project-workflow` 的任务登记、claim、工站或 close 控制器。单次局部可回滚问题不必造 Issue。开始长期任务读关联 Issue 正文与最新评论，并核对当前 HEAD、工作区和实际证据；不从旧聊天推测进度。

已明确授权的任务可在独立任务分支内实施、按风险验证并创建草稿 PR；不要把“已验证开发”当成发布或真机可用。合并 main、外发用户书库、安装真实设备与正式发布单独确认。保留未提交修改，不为流程做 reset/clean。

## 当前文档

- `CONTEXT.md` 定义稳定目标、目录边界和事实所有权；
- `ACTIVE.md` 只指向当前工作，不保存详细状态或验收摘要；
- `INDEX.md` 是当前文档注册表和 Context Bundle；
- 产品、架构、代码地图和契约各自只维护自己的事实；
- `docs/changes/*.md` 是新跨模块变更的唯一记录；已关闭的记录由 Git 保存；
- `docs/research/*.md` 只保存当前未决问题的可追溯研究，结论进入事实所有者后不再作为默认上下文。

## 工作路径

- `audit`：有明确对象和停止条件的只读调查；
- `fast`：局部、可回滚、不改变依赖、接口、数据、安全或信任边界的维护；
- `change`：产品行为、依赖、架构、数据、验收或跨模块实现变化。实施前需有一份 `accepted` change 和用户对范围的批准。

新 change 在同一份文件写问题、范围、验收、文件与步骤、检查、结果和 review。不要再为新任务机械新建 milestone、spec、plan 与 review。旧四件套仅用于追溯，不能自动成为当前约束。

## 事实与验证

- 不可信书籍默认拒绝脚本、网络、路径越界和未知资源；
- 当前阅读渲染统一使用平台 WebView：Windows 使用 WebView2，Linux 使用 WebKitGTK，Android 使用 system WebView；外部独立引擎达到浏览器兼容、选择与性能门槛后才重新研究；
- PCT-AL10 真机操作以 `docs/agents/references.md` 的项目流程为入口，不依赖用户级 `android-cli` skill，也不得把 `android install` 或 `adb install` 当作华为静默安装方案；
- 静态、本地、真实目标和生产等价证据必须明确区分；
- 不得把代码阅读、截图或本地结果说成未执行的真实验收。

## 收尾

修改版本控制文件后更新事实所有者与 `ACTIVE` 指针，运行相称检查。触碰中文 Markdown 时只格式化本次文件；不推送或发布，除非用户明确要求。

# Atha 仓库协作与检查入口

开始、恢复或关闭仓库任务前读取 `AGENTS.md`、`CONTEXT.md`、`docs/ACTIVE.md` 和对应 Context Bundle。外部技术与标准的官方入口见 `docs/agents/references.md`；涉及框架行为、API、兼容性、错误或性能时，按需核对对应版本的官方文档或源码。

## GitHub Flow

1. **开短期分支**：独立任务从最新远端 `main` 开分支；依赖未合并工作时，记录所依赖的分支和提交。共享工作区正在使用或有在途改动时，用独立 worktree 开工。
2. **开发并提交**：一个分支只处理一项相关变化。修复可复现的行为时，先证明回归用例能抓到原问题，再确认修复后通过。跨会话工作关联 [Issue](issue-tracker.md)。
3. **验证后提审**：候选提交完成后，按[本地检查](#提审前在本机检查)验证干净工作树。获准外发后再推送、开 PR；需要早期反馈可先开 Draft，并标明未跑的检查。PR 写清被测提交、环境、结果和未覆盖项。
4. **审查并合入**：处理阻塞意见。候选或目标基底变化时，只复核受影响的检查与审查。确认 PR 当前 head 就是已审查的候选，获得明确授权后才合入 `main`。
5. **收尾**：合并后更新相应 Issue；仍待验收的 Issue 保持打开。确认无后续分支依赖且获准清理后，删除短期分支。

## 工作路径

按[工作协议](../workflow/PROTOCOL.md)选择 `audit`、`fast` 或 `change`。在获批范围内工作，保留其他在途改动。

## Review、迭代与证据

独立 review 只读检查实现与验收条件，并把发现分为 `blocking`、`non-blocking`、`out-of-scope`。迭代任务在同一份 change 中按 Review、Repair、Validate 记录轮次、剩余差异、停止原因与证据等级；完整命令和输出按需保存在 Git 忽略的 `artifacts/local/`。

检查从成本最低且最能区分假设的步骤开始：配置、语法和静态预检，受影响 Module 的目标检查，仓库 gate，最后才是高成本 GUI、真实目标或 benchmark。低成本检查失败时先停止扩展并修复；预期拒绝必须由项目脚本确认正确失败类型并返回零，意外接受、错误失败类型或未完成检查返回非零。

## 提审前在本机检查

在 T3 中交付任务时，代理先在候选所在的干净工作树完成下列适用检查，失败先处理，再提审。已有同一候选、基底和环境的有效结果就直接引用。检查都在本机执行，`git push` 不启动测试。

- **候选差异**：`bash scripts/check-workflow.sh --base origin/main`。以实际 PR 目标替换基底；按协议走 `fast` 时加 `--kind fast`。检查从共同祖先到候选的已提交差异及空白错误；检查工作区时不带参数。纯文档改动到这里即可，不启动产品构建。改动文档门本身时，用 `python3 scripts/tests/test_doc_guard.py -v` 复验真实 Git 历史的正反例。
- **代码改动**：跑受影响的单测、静态检查和构建；跨模块可直接用 `bash scripts/check-fast.sh --base origin/main`，其中已包含候选文档门。当前浏览器回归入口见 [阅读代码地图](../codebase/MAP.md) 与 `reader/app/tests/*.browser.mjs`；Chromium 结果按浏览器证据记录。
- **原生行为**：只改书架往返时，运行 `bash scripts/check-reader-linux.sh --shelf`；影响完整阅读链路时，运行无参 `bash scripts/check-reader-linux.sh`。Linux AppImage 候选用 `bash scripts/check-reader-candidate.sh` 另行验包；系统输入、原生桥或安装改动按对应平台和候选选择检查。

## 发版单独验收

默认在 `0.1.x` 线上递增补丁版本；只有用户明确要求时才提高次版本或主版本。

记录上版 tag（首发记录公开基底）、本次候选提交、版本及实际安装包。从这段差异列出用户会遇到的变化，在预定目标平台验证安装或覆盖升级、启动和受影响的真实操作。记录所用包、证据和未测项；模拟器与 debug build 的结果分别记录。获准后发布已验证的包；重新构建或换包时，先复核受影响的证据。PCT-AL10 的构建、校验与安装边界见 [真机流程](references.md#pct-al10-真机与-adb-取证)。

Issue 保存跨会话交接，change 保存获批范围、技术决策和验收证据，PR 保存候选差异与审查，`ACTIVE` 指向当前工作。关闭任务前更新事实所有者和 `ACTIVE` 指针，运行相称检查，记录实际结果及覆盖缺口。Rust 版本由 `rust-toolchain.toml` 固定。

# M4 本地书架与应用内导入

## Status

accepted

## Problem

当前 Tauri 产品入口必须通过命令行指定 EPUB 或已准备书根，用户无法从应用内建立书架、选择本地书籍并再次打开。阅读器能力已经可用，但缺少成为日常应用所需的最短入口闭环。

Readest 的有效经验是把文件选择、内容哈希去重、耐久书目和打开前可用性检查串成一条流程；其多格式转换、同步、分组、批量传输队列、远程来源和全局 store 不适合 Atha 当前阶段。

## Scope

- 无启动书籍参数时进入本地书架；既有 `--epub` 与验证入口保持兼容；
- 使用 Tauri 官方文件对话框选择一个或多个 EPUB，并复用现有 `import_epub`；
- 从 EPUB3 OPF 读取受限标题、作者和封面路径，内容哈希继续作为稳定书籍身份；
- 在 `%LOCALAPPDATA%/Atha/Library` 以每书一份小型 JSON 记录保存书架，导入内容继续位于既有 `ImportedBooks/<sha256>`；
- 书架支持列出、导入、打开和移除；移除只删除书架记录，不删除导入缓存与阅读状态；
- Tauri 在打开书籍时切换受控 `atha-book` 书根，封面由独立只读协议提供；Svelte 只消费受限书目数据；
- 新增竖屏优先、可随窗口扩展的书架界面，包含空状态、导入状态、失败反馈、封面卡片和返回阅读链路。

## Non-Goals

- 书架分组、筛选、排序设置、批量管理、拖放和自动扫描；
- 最近阅读、阅读时长、云同步、账户、OPDS、URL 导入和多格式工厂；
- 删除导入缓存、标注、书签或进度；
- EPUB2/NCX、受保护书籍修复、联网补全元数据或封面；
- Windows 文件关联、安装包和后台导入队列。

## Acceptance Criteria

- [ ] 不带书籍参数启动 Tauri 时显示书架；空书架可直接选择 EPUB；
- [ ] 文件对话框只选择 EPUB，可多选；取消不改变书架；单本失败不回滚同批成功项；
- [ ] 相同内容从不同路径重复导入只产生一个书架项；重启后书架仍存在；
- [ ] 卡片显示受限标题、作者和可用封面；缺少封面时显示稳定占位；
- [ ] 点击书籍复用现有阅读内核打开，返回按钮回到书架，既有进度与偏好按内容身份恢复；
- [ ] 从书架移除后内容缓存和阅读状态不被删除，再次导入可以恢复；
- [ ] 损坏书架记录、未知书籍身份、缺失缓存和非法封面请求明确失败，不越过书根；
- [ ] 既有 EPUB CLI、困难样本、安全检查和 Tauri 性能门槛保持通过；
- [ ] 前端检查、production build、Rust fmt/clippy/test、真实 Windows Tauri 启动和相称 UI 检查通过。

## Files And Steps

1. 扩展 `reader::epub`，在不改变阅读 manifest 的前提下产生受限书目元数据和封面路径。
2. 新建深 module `reader::library`；其 interface 只提供 `list`、`import`、`open`、`remove`，隐藏记录校验、目录和缓存定位。
3. 在 Tauri 壳接入官方 dialog、书架 commands、动态书根和只读封面协议，保留原 CLI 验证入口。
4. 用 Svelte 书架页组合标题栏、空状态和书籍卡片；阅读内核仍只在阅读路由加载。
5. 增加书架正式检查，更新 Roadmap、阅读架构和代码地图。

## Checks

- `cargo fmt --all -- --check`
- `cargo clippy --locked --workspace --all-targets -- -D warnings`
- `cargo test --locked --workspace`
- `pnpm --dir reader/app check`
- `pnpm --dir reader/app build`
- `pwsh -NoProfile -File scripts/check-library-shelf.ps1`
- `pwsh -NoProfile -File scripts/check-reader-samples.ps1`
- `pwsh -NoProfile -File scripts/check-tauri-reader.ps1`
- `autocorrect --fix` 与 `autocorrect --lint` 仅处理本次中文 Markdown
- `git diff --check`

## Rollback

移除书架 module、Tauri commands/协议和 Svelte 书架入口，恢复启动时必须提供书籍参数。已导入缓存和书架 JSON 可保留为无消费者的本地数据，不影响旧 CLI 阅读入口。

## Approval

2026-08-04：用户批准参考 Readest 导入功能，开始 M4 书架与界面实现。

## Result

待实现。

## Review

- Blocking：待 review。
- Non-blocking：待 review。
- Out-of-scope：待 review。

## Evidence And Residual Risks

待实现与验证。

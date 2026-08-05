---
description: 让阅读器兼容无 XHTML 扩展名章节和标准 HTML5 导航声明的 EPUB 3。
---

# 《唯物主义》EPUB 兼容性

## Status

implemented

## Problem

《唯物主义（2023）》是未加密 EPUB 3，具备单一 OPF、EPUB 3 导航和 XHTML 书脊；但部分书脊资源没有 `.xhtml` 扩展名，导航包含标准 `<!DOCTYPE html>`。当前导入器在已声明 `application/xhtml+xml` 后仍强制检查文件扩展名，并拒绝导航中的所有 DOCTYPE，导致应用报 `unsupported-epub`。修复这两项后，阅读内核又因无扩展名章节没有书源样式表而报 `missing-stylesheet`。

## Scope

- 以 OPF manifest 的 `application/xhtml+xml` 声明判断导航和书脊资源，不再额外要求 `.xhtml` 后缀；
- 允许导航文档使用精确的 HTML5 `<!DOCTYPE html>`，继续拒绝其他 DOCTYPE，container 与 OPF 的现有限制不变；
- 让受限资源服务把已导入的无扩展名书脊作为 XHTML 返回；
- 允许合法 XHTML 章节不引用书源样式表，此时只应用阅读器样式；
- 用合成 EPUB 固定上述兼容性，并用指定真实样书验证普通应用入口能够进入阅读页。

## Non-Goals

- 不扩展 EPUB 2、加密 EPUB、远程资源、脚本或未知媒体类型支持；
- 不放宽路径、归档大小、活动内容、XML 外部实体或资源越界限制；
- 不为单本书增加文件名特判、预处理副本或内容改写。

## Acceptance Criteria

- [x] 合成 EPUB 可导入无扩展名 XHTML 书脊和带 HTML5 DOCTYPE 的导航；
- [x] 无扩展名书脊由 `BookRoot` 以 `application/xhtml+xml` 返回；
- [x] container 中的 DOCTYPE、非 HTML5 导航 DOCTYPE 与既有不安全输入仍被拒绝；
- [x] 不带书源样式表的合法章节能够呈现；
- [x] 指定《唯物主义（2023）》通过普通 Tauri 入口打开并进入阅读页；
- [x] 现有后端、EPUB 和 Tauri 阅读器回归无 blocking。

## Files And Steps

1. 先增加最小合成 EPUB 回归，复现扩展名与导航声明限制；
2. 在 importer 和 `BookRoot` 的共享边界做最小修复；
3. 运行后端检查和指定真实样书的 Tauri/WebView2 验收；
4. 更新阅读内核兼容性事实和代码地图，关闭 change。

## Checks

- `cargo test -p atha-backend --test epub_import`
- `pwsh -NoProfile -File scripts/check-backend.ps1`
- `pwsh -NoProfile -File scripts/check-tauri-reader.ps1 -Epub 'fixtures/local/唯物主义 (2023).epub'`
- `pwsh -NoProfile -File scripts/Invoke-Atha.ps1 check docs -Activity validation -Scope project-workflow`
- `autocorrect --fix` 与 `autocorrect --lint` 仅处理本次中文 Markdown
- `git diff --check`

## Rollback

整体回退本 change 的提交；已有导入缓存按内容哈希隔离，不修改源 EPUB。

## Approval

2026-08-05：用户提供《唯物主义（2023）》并要求阅读器能够打开该书，批准本兼容性范围。

## Result

EPUB importer 现在以 OPF media type 判定 XHTML，并只在 navigation document 接受精确 HTML5 DOCTYPE。`BookRoot` 从受限 reader manifest 建立 section MIME 映射，未声明的无扩展名文件仍拒绝。阅读会话不再要求 section 以 `.xhtml` 结尾，也不再要求每章必须带书源样式表；同源链接仍交由 Navigation 对未知 section 安全回落。

## Review

- 首轮 Standards/Spec 双轴复核发现两个 blocking：manifest MIME 映射可能跟随越界符号链接，以及 HTML5 DOCTYPE 没有限制位置和唯一性；修复后分别由 Windows 符号链接回归和重复 DOCTYPE 负例覆盖。
- 修复后 Standards 与 Spec 复核均无 blocking；Spec 无 non-blocking。Standards 仅保留 importer、`BookRoot` 与浏览器会话分别维护 section 路径和数量边界的重复判断；三处属于不同语言和信任边界，等待规则真实漂移后再考虑共享生成，不为本次兼容性引入跨层抽象。

## Evidence And Residual Risks

- 静态与本地：合成 EPUB 回归覆盖无扩展名 XHTML、HTML5 navigation DOCTYPE、非 HTML5 DOCTYPE 拒绝，以及未声明无扩展名资源拒绝；完整后端 fmt、clippy、workspace test 和 doc 通过。
- 真实浏览器：现有四样本的明暗主题、内容交互、安全、搜索、标注和跨进程状态回归全部通过。
- 真实 Tauri/WebView2：在隔离应用数据中从普通 `--epub` 入口打开《唯物主义》，识别 11 个 section 和 10 条 TOC；无扩展名章节返回 `application/xhtml+xml`，10 条目录项逐章打开均保持 `pass`，最后到达“译者简介”。用户现有书架和笔记未写入。
- 环境说明：Rust 检查多次报告 Windows incremental 目录无法收尾，未影响构建、测试或运行结果；全局隔离 `LOCALAPPDATA` 会使 mise 误判工具未安装，因此真实产品检查只对 Atha 子进程隔离该目录。

---
description: 用真实样本矩阵修复 EPUB 中可安全降级的包装、导航与资源缺陷。
---

# EPUB 兼容性矩阵

## Status

implemented

## Problem

部分可被成熟阅读器打开的 EPUB 存在缺失导航、缺失非关键成员、非规范路径或历史工具生成的包装差异；当前导入器会因此拒绝整书。可降级的导航、封面、字体和普通图片不应成为安全正文可读时的硬门槛。

## Scope

- EPUB 3 缺少 `nav` 时复用有效 NCX；两者都没有时允许空目录；
- NCX 中重复的规范化目录目标稳定保留第一项，不因可忽略重复拒绝整书；
- 容忍安全的点路径、单次百分号编码、外部 DOCTYPE、兼容的 manifest 路径别名和缺失 spine 成员；
- 只把允许的字体混淆算法降级为系统字体，并允许缺失的封面、样式表和普通图片局部降级；
- 忽略不影响 container / package 识别的非规范 `mimetype` 包装，但不修写源 EPUB；
- 以 Windows OneDrive 中按修改时间排序的 137 本 EPUB 做匿名导入矩阵，并对可导入项逐本执行 Linux Tauri / WebKitGTK GUI 打开；
- 保留归档大小、路径逃逸、加密、解压边界和正文成员检查；
- 用匿名合成书回归，并用用户本地真实文件做 Linux 验证，不复制或记录私有内容。

## Non-Goals

- 不修补 ZIP 二进制、不推测缺失章节、不放松归档与路径安全边界；
- 不允许远程资源、绝对路径、重复 manifest ID 或未知加密；
- 不引入新解析器、依赖或通用 EPUB 修复框架。

## Acceptance Criteria

- [x] EPUB 3 可在缺少 `nav` 时使用 NCX；
- [x] 导航损坏时正文仍可导入，目录降级为空；
- [x] 缺失非关键资源时保留正文，图片使用替代文本且不发起外部请求；
- [x] 137 本真实样本矩阵给出匿名结果、耗时分位数和剩余硬拒绝理由；
- [x] 现有安全拒绝测试保持通过；
- [x] 所有成功导入项均可在 Linux GUI 打开非空正文。

## Architecture Impact

present

- Design purpose: 把包装、导航和可选资源错误从整书失败缩小为局部降级，同时保持正文、路径、网络、实体和加密边界不变。
- Modules / interfaces: 后端 EPUB adapter 仍只输出现有 ReaderManifest / BookRoot；前端 content 层只消费 manifest 已发布资源，缺失可选资源不新增接口。
- Candidate and tradeoffs: 复用 `zip 8.6`、`quick-xml`、浏览器 CSSOM 与原生替代文本，不引入修复框架或第二 parser；pseudo-ZIP64 上游缺口保留为明确拒绝。
- Evidence / review trigger: 匿名 137 本矩阵、重复 release 基准、所有成功项的 Linux Tauri / WebKitGTK 真实打开、缺失图片多章节遍历和独立安全审查。

## Files And Steps

1. 先以匿名矩阵按稳定错误码归类，不从书名、路径或内容推断兼容策略；
2. 在共享归档、EPUB package 和 ReaderManifest 边界实现最小安全容错，并用合成书固定拒绝条件；
3. 重建前端 bundle 后逐本运行 Linux GUI，另遍历缺失图片样本的全部受影响 section；
4. 重复 release 基准、全仓 gate 和独立安全复审后更新事实所有者。

## Checks

- EPUB 导入定向测试与 workspace Rust 检查；
- 真实样本矩阵与重复导入基准；
- Linux Tauri / WebKitGTK 逐本真实打开；
- AutoCorrect、文档 gate、`git diff --check` 与独立 review。

## Rollback

恢复严格导航分支即可；不迁移或删除书籍数据。

## Approval

用户明确要求：只要其他阅读器能打开，Atha 就应尽量忽略可恢复错误，优先保证阅读；功能可以局部降级。

## Result

后端 release 矩阵稳定导入 133 / 137 本。剩余四项均是有意硬拒绝：#25 含活动网络 manifest URL，#67 命中 `zip 8.6` 的 pseudo-ZIP64 上游解析缺口，#105 使用绝对归档成员与绝对图片引用，#109 含 89 个重复 manifest ID。系统 ZIP 工具可打开 #67，但 Atha 没有绕过归档重叠与边界校验。

最终三轮各检查 137 本且保持四项相同拒绝；导入 median 分别为 65.64、64.14、64.58 ms，P95 为 592.36、596.56、595.60 ms，最大值为 1235.21、1214.97、1211.43 ms。Linux Tauri / WebKitGTK 0.55.1 对 133 个成功导入项逐本打开非空正文和非空截图，结果 133 / 133；缺失普通图片样本另遍历 99 个受影响 section，全部保持 `layout-stable`，section P95 为 223.60 ms。

## Review

独立复审发现并修复 EPUB3 非图片封面晚失败、无效 SVG 回退 O(N²)、大量字体混淆项查找 O(N²)、大写 `.SVG` 绕过显式校验和自检多余请求。最终复审未发现 P1 / P2；路径逃逸、实体、远程资源、CSS 子资源及未知加密仍保持硬拒绝。

## Evidence And Residual Risks

EPUB 定向测试为 17 passed / 1 ignored；workspace Clippy、Rust 全测试、Svelte check / build、正式 Linux GUI 门、AutoCorrect、文档 gate 与 diff 检查均通过。正式 Linux GUI 门同时覆盖共享 CSS 模块、统计、宽窄视口与状态恢复，CSS 模块和阅读统计 P95 均为 2 ms。

残余风险是 `zip 8.6` 的 pseudo-ZIP64 兼容缺口和未执行系统化 EPUB fuzzing。对上游 ZIP64 sentinel 的三行兼容补丁已通过 `zip` 自身 115 项测试并越过归档重叠检查，但矩阵中的同一样本随后仍因重复 manifest ID 被安全拒绝，净兼容增益为零，因此不维护本地 fork。绝对路径、重复 manifest ID、远程资源与未知加密不是残余兼容缺口，而是保留的信任边界。

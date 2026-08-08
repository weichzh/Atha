---
description: FB2 与 FBZ 导入、受控渲染及 Linux GUI 纵切。
---

# FB2 与 FBZ 纵切

## Status

`accepted`

## Problem

Atha 已支持 EPUB、CBZ、Markdown 与 TXT，但 Readest 0.11.18 的非 PDF 能力还包含 FB2、FBZ、MOBI、AZW 与 AZW3 / KF8。路线图要求先关闭 FB2，再进入 Kindle 系列。当前 picker、LocalLibrary 和正式 Linux Tauri GUI gate 都不接受 FB2。

## Research And Decision

- Readest 0.11.18 的公开格式边界是 EPUB、MOBI / AZW / AZW3、FB2、CBZ、TXT 与 Markdown；其固定源码还接受单 FB2 ZIP 封装 `.fbz`。本切片同时交付 `.fb2` 与 `.fbz`，不把任意 `.zip` 解释为书籍；
- Readest 应用代码是 AGPL-3.0，`foliate-js` 是 MIT。只参考行为边界，不复制应用实现，也不在 WebView 中增加第二套书籍模型；
- `fb2` 0.4.4 是 MIT 的完整内存数据模型，但最后更新于 2023 年，会额外引入旧版 `quick-xml`，且不能消除 XHTML 投影、分节、链接重写和资源边界。采用仓内已有的 `quick-xml` 流式解析，另用成熟 `base64` 解码二进制；这是依赖和 Android 峰值更小的现成库路径；
- FB2 / FBZ 仍归一为 schema 1 ReaderManifest / BookRoot。禁止 DTD、处理指令、书内脚本、外链、路径逃逸和未知二进制类型；只投影 FB2 标准正文元素、内部链接及 JPEG / PNG 图片。

## Scope

- 导入 `.fb2` 和只含一个根级 `.fb2` 文件的 `.fbz`；
- 有界解析书名、作者、封面、正文 sections、目录、内部锚点和图片资源；
- 接入桌面 / Android picker、LocalLibrary、日志错误码与既有 protocol；
- 用原创动态 fixture 覆盖正例、FBZ、编码、资源、链接、拒绝矩阵和内容寻址复用；
- 扩展正式 Linux Tauri GUI gate：在仓库 `.tmp` 下通过真实 LocalLibrary 种入原创 fixture，再验证书架、目录、搜索、跨 section 导航、重启恢复、非空截图与隐私；系统 picker 后缀由 Rust 单元测试覆盖，不用脆弱的桌面原生对话框自动化冒充导入链路。

## Out Of Scope

- MOBI / AZW / AZW3 / KF8；
- DRM、外部资源、书内脚本、任意 ZIP、FB2 自定义 stylesheet 与非图片 binary；
- Android / ARM 真机性能结论；日常开发不启动模拟器，Kindle 与词典阶段再用指定 ARM 真机和私有样本验收。

## Acceptance Criteria

- [x] `.fb2` 与 `.fbz` 经同一 importer 生成稳定 manifest、书目、封面、正文、目录和跨 section 锚点；
- [x] DTD / PI、外链、未知根、深度 / section / TOC / source / binary 越界、无正文、损坏 XML / base64、歧义 FBZ 和不支持图片返回稳定错误；
- [x] ReaderManifest、BookRoot、Locator、搜索、消息和 CSS 层不分叉；EPUB、CBZ、Markdown 与 TXT 不回归；
- [x] Rust fmt / Clippy / tests、Svelte / Tauri、Linux GUI 正式 gate、AutoCorrect、required docs gate 与独立 Spec / Standards review 通过。

## Files And Steps

1. 以原创 fixture 建立 FB2 / FBZ 正例和拒绝矩阵；
2. 用已有 `quick-xml` 与 archive 边界实现单一 importer，有界生成 XHTML / 图片 / manifest；
3. 接入 LocalLibrary、picker 和固定错误码；
4. 扩展 Linux Tauri GUI gate，记录导入、打开、目录、搜索、翻页、恢复和隐私证据；
5. 更新事实所有者，完成双轴 review、提交和 task closure。

## Checks

- `cargo test --locked -p atha-backend --test fb2_import` 与 workspace fmt / Clippy / tests；
- `scripts/check-fb2-source.ps1`；
- `pnpm --dir reader/app check`、`pnpm --dir reader/app build` 与 Tauri tests；
- Linux Tauri GUI 从干净应用数据打开隔离 LocalLibrary 中的原创 fixture，覆盖书架、目录、搜索、跨 section 导航和重启恢复；
- AutoCorrect、required docs gate、Spec / Standards review。

## Rollback

删除 FB2 importer、picker 扩展和 gate 分支，恢复 LocalLibrary 的既有分派。ReaderManifest、书架记录、Locator、消息数据库和其他格式缓存 schema 均不迁移；已导入 FB2 书根只会成为未引用 cache。

## Approval

用户已明确批准按照 Atha 路线图持续完成 Readest 支持的非 PDF 格式，并要求成熟库和现成平台能力优先。本 change 是已批准路线图中日志收口后的下一最小切片。

## Result

已交付直接 FB2 与单根成员 FBZ importer、LocalLibrary / picker 分派、metadata / 封面 / 目录 / 内部链接 / 图片投影、稳定错误码和原创测试矩阵。同一 XML 的 `.fb2` 与 `.fbz` 共享内容身份；声明为 Windows-1251 的样本以及 XML 预定义 / 数字字符引用可解析，无 `id` 嵌套章节不会生成重复 TOC 目标，源 stylesheet 与未知主动内容不会进入 WebView。

日常目标端改为 Linux Tauri / WebKitGTK。真实 GUI 验收同时暴露并修复了 Linux 应用根 `tauri://localhost` 无尾斜杠、custom scheme 的 `URL.origin` 恒为 `null`，以及 WebKitGTK 不暴露 Permissions Policy JavaScript 检查 API 三项既有跨平台问题；Android 模拟器已关闭并移出日常门禁。

## Review

首次独立双轴 review 找到四个有效问题：GIF / WebP 绕过像素预算、未引用未知 binary 被静默丢弃、合法 XML 实体被误拒、无 `id` 嵌套章节生成重复 TOC href。实现已收缩到 JPEG / PNG 并在 binary 发现时拒绝未知 MIME；两遍解析只解析 XML 预定义 / 合法数字引用，TOC 目标按 href 去重。对应回归加入后，独立 Spec 与 Standards 复核均为 zero findings。

## Evidence And Residual Risks

- 本地 importer / 回归：workspace Rust fmt、Clippy 与 tests 通过；Svelte check 为 0 errors / 0 warnings，Vite production build 和 Linux Tauri debug build 通过；
- Linux 真实目标：`scripts/check-fb2-source.ps1 -VerifyLinuxGui` 通过真实 Tauri / WebKitGTK，确认 4 sections、3 TOC、1 条全书搜索结果、跳转到第 4 section、重启恢复第 4 section，截图包含 305 种颜色，AppLog 隐私扫描通过；
- 原创 fixture：源 SHA-256 为 `155225e7aa977574c5f75559f58ad121004bf714b91e10caeacd774da5550186`，内容版本为 `5cec82bcb55147801cbf6c6ba1da32b94483e7e14af8b98439048075eec68f52`；fixture 只在 `.tmp` 动态生成并在 gate 后清理；
- 当前最高证据是 Linux 真实 GUI，不是 Android ARM 真机性能、系统 picker GUI 自动化、安装包或生产证据。FBZ 的 FB2 成员仍受共享 archive 16 MiB 单成员上限；GIF / WebP binary、源 stylesheet 和未知正文元素明确不支持。

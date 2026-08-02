# R5 进度恢复与书签

## Status

implemented

## Problem

R2 已定义稳定 Locator，R3 已定义应用与本书两层偏好，但它们仍只存在于当前进程。关闭阅读器会回到开头并丢失偏好，也没有一个耐久消费者验证 Locator 在正常重开、生命周期中断和内容版本变化后的行为。

R5 只建立 WebView2 原生存储上的最小阅读状态：高频位置单独写小记录，低频偏好与书签按所有权写入各自记录；不引入数据库、同步或通用状态框架。

## Scope

- Windows host 使用持久 WebView2 profile，并向阅读页传递不暴露本机路径的稳定书籍状态键；
- 应用偏好跨书持久化，本书样式、书签和阅读位置按书籍状态键分区；
- 位置只保存 schema 1 Locator，与偏好和书签分开写入；同一任务内的多次位置更新合并，页面隐藏或退出前 flush；
- 打开书籍时先恢复有效偏好，再恢复同内容版本的位置；损坏、越界或错版本记录安全回落且不阻止阅读；
- 书签可在当前 Locator 创建、跳转和删除；错版本书签保留为不可跳转记录并明确报告；
- 验证模式默认使用易失存储，正式浏览器持久化探针显式启用并在结束后清理，避免污染样本和用户状态。

## Non-Goals

- 不引入 SQLite、文件状态服务、迁移框架、账户、云同步、冲突合并或跨设备标识；
- 不实现书架、阅读统计、历史轨迹、书签分组、备注、排序、导入导出或跨版本自动重锚；
- 不把页码作为耐久坐标，不保存书源 HTML、选区原文或用户目录；
- 不为未来来源预建格式工厂；M3 再定义真实书籍身份。

## Acceptance Criteria

- [x] 正常重开和 page lifecycle flush 后恢复最后稳定 Locator；高频位置只写独立小记录且同任务合并；
- [x] 应用偏好跨书、本书偏好按书恢复，损坏状态不会阻止安全打开；
- [x] 书签可创建、跳转、删除，重复位置不重复创建，错版本书签不误跳；
- [x] 状态键不暴露本机路径，不可信书籍仍无法访问应用存储或 IPC；
- [x] 四样本实际 host、明暗浏览器、持久化重开、Rust 检查和 benchmark 均通过；
- [x] 独立规格与标准 review 无 blocking，事实所有者和 `ACTIVE` 与最终实现一致。

## Files And Steps

1. 为 host 增加稳定状态键并启用持久 WebView2 profile；
2. 增加阅读状态模块，校验并分开持久化应用偏好、本书状态与进度；
3. 增加最小书签控件与模块，并在 Navigation 稳定点调度进度写入；
4. 扩展诊断和正式 runner，覆盖合并写、flush、损坏/错版本回落和真实重开；
5. 更新事实所有者，运行完整检查、benchmark 与独立 review。

## Checks

- 所有页面 JavaScript module 的 `node --check`；
- `cargo fmt --all --check`、`cargo clippy --workspace --all-targets --locked -- -D warnings`、`cargo test --workspace --all-targets --locked`；
- `pwsh -NoProfile -File scripts/check-reader-samples.ps1`；
- `pwsh -NoProfile -File scripts/check-reader-slice.ps1`；
- workflow `docs` gate；
- 本次中文 Markdown 的 `autocorrect --fix` 与 `autocorrect --lint`；
- `git diff --check`。

## Rollback

回滚本 change 的提交即可恢复 R4D 的纯会话状态；不迁移或删除外部数据。R5 生成的浏览器 localStorage 记录可被旧版本忽略。

## Approval

用户明确授权依据路线图连续实现至 M2 结束，并要求缺少规格时补规格。本 change 只完成 R5 进度、偏好持久化与书签。

## Result

Windows host 改用持久 WebView2 profile，并从规范入口路径生成 16 个十六进制字符的状态键。manifest 沿用声明的内容版本，旧 `entry` 兼容入口根据 XHTML 字节生成内容指纹。阅读页以三个 schema 1 记录分开保存应用偏好、本书偏好与书签、阅读进度；稳定导航在同一任务内合并进度写入，并在隐藏或离开页面时 flush。

打开时先恢复有效偏好，再恢复同内容版本的位置。书签支持当前位置创建、去重、跳转和删除；错版本书签保留但拒绝跳转。损坏记录或不可用存储只降级当前状态，不阻止阅读会话。

## Review

- Spec：首次检查发现 legacy `entry` 版本、真实 host 重开证据和提前关闭状态共 3 项 blocking；修复及 probe 命名空间隔离后复审无 blocking；
- Standards：首次检查发现偏好故障隔离、耐久 Locator 严格校验和真实 host 证据共 3 项 blocking；修复后复审无 blocking。

## Evidence And Residual Risks

- Windows 本地四样本正式 runner 通过：实际 WebView2 host、明暗浏览器、自检、持久重开、书签 UI 和损坏进度回落均通过；
- Rust `fmt`、`clippy -D warnings`、workspace tests 和 release host build 通过；页面模块 `node --check` 通过；
- R5 benchmark run `1785700440970-41468` 的 10 样本中位数为：冷启动 814.967ms、首个稳定页面 190.750ms、热打开 22.900ms、翻页 7.150ms、字号重排 31.100ms；
- 最高证据等级为 Windows 本地真实 host 与真实浏览器；正式 runner 在独立 probe 存储命名空间和状态键上启动两个 WebView2 host 进程，第二进程验证主题、字号、精确 Locator 与书签恢复后清理状态；
- 当前状态键绑定规范书根与入口，书籍移动后不迁移旧状态；真实书籍身份和跨内容版本重锚留给 M3 与 R7。
- WebView2 profile 目录仍使用平台默认行为，安装路径变化后的连续性留给正式打包；兼容 `entry` 的 FNV 派生内容指纹不是正式强身份；
- 同版本但语义越界的书签会在跳转时安全回落；额外的存储故障注入矩阵留给 R8，当前静态复审确认应用与本书记录的故障隔离。

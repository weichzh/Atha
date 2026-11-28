# 本地清理与 PCT-AL10 试装入口

## Status

implemented

## Problem

PR #29 收尾和手机试装分别需要手拼 Git 与 Android 命令。固定这两条路径，复用已有检查、签名与安装机制。

## Scope

提供可预览并逐项选择的本地引用清理，以及编译、同签名覆盖安装和启动 PCT-AL10 的本地 Run 入口。保持原有 build／verify／install 调用、数据保护与签名要求。无需运行 GitHub Actions，也不更改既有钩子、远端分支、T3 工作树、发版配置或手机数据。

## Architecture Impact

none

## Acceptance Criteria

- 清理默认只读，显示候选和受保护项；仅删除操作者选中的已合入本地短分支或失效远端跟踪引用。可取消；引用、主线、依赖或占用变化时重新确认。不删除工作树或文件。
- 手机 Run 先检查显式目标，再以本机试用签名编译；复用现有验包、同签名覆盖、前台启动与收据。不默认选设备，不卸载、不清数据、不降级。

## Files And Steps

在 Mise 注册轻量入口。清理用 Git 官方查询与非强制删除；手机 Run 扩展现有脚本和局部 Gradle 签名配置。

## Checks

临时 Git 仓库与 bare remote，验证预览、取消、筛选、保护和选择后状态变化。验证 Android 参数、拒绝分支和本机签名产物；没有部署请求时不写手机。

## Iteration Control

- Mode: `iterative`
- Maximum repair iterations: `2`
- Unchanged-delta limit: `1`
- Human stop: 未获准就不操作真实删除或安装。

## Approval

用户要求实现可选择的仓库清理与 Android Studio 式编译、安装和启动入口。

## Result

本地候选添加 `mise run repo:cleanup` 和 `mise run android:run -- --device <SERIAL>`。预览中工作树只展示；应用清理时以当前远端主线和用户选中的精确引用重新核对。Android Run 使用独立产物路径，设备写入前由现有安装器验包。

定向复验：临时 Git 仓库 5 项通过；包括跳过只剩失效跟踪引用的未合入提交、只删选中项和拒绝预览后的变更。Mise 实跑预览和设备未连接拒绝。最终 `build --local-test` 本机 arm64 APK 构建成功，v2／v3 签名及 16 KiB ZIP／ELF 对齐通过。

## Review

本地复核 Git 删除保护、设备预检与原入口兼容；独立审查及处理结果见 PR。

## Evidence And Residual Risks

本轮未对真实仓库执行删除、未安装到手机。真机安装与自然触摸仍是不同验收。

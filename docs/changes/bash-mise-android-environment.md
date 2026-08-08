# Bash / mise / Android 4G 环境修复

## Status

implemented

## Problem

Linux 开发入口没有项目级工具版本固定，当前 Shell 的 Node、pnpm 和 `JAVA_HOME` 可能偏离 Android 门槛；专用 AVD 只有 2 GiB，且离线 Cargo 缓存不完整，导致项目不能稳定从 Bash 构建和启动。

## Scope

- 使用项目根 `.mise.toml` 固定 Node 24.1.0、pnpm 11.19.0 和 Temurin JDK 21；保留 `rust-toolchain.toml` 作为 Rust 唯一版本来源。
- 保留 `env/local.ps1` 作为现有 PowerShell 检查脚本的兼容入口，并与 mise 版本对齐。
- 将本机 `Atha_API_36_16K` AVD 调整为 4 GiB，在 GNOME user manager 环境中启动并验证 API 36、x86_64 和 16 KiB 页。
- 修复 Linux 下消息资产目录链接测试的清理语义；不改变产品数据契约。
- 修复 Linux 非 Windows 诊断 unit struct 的 Clippy `default_constructed_unit_structs` 阻塞。

## Architecture Impact

none

## Acceptance Criteria

- [x] Bash 下 `mise install`、锁定依赖解析、前端 check/build 和 Rust workspace build 通过。
- [x] backend 全量测试、前端 Markdown 测试、Rust fmt 和 Clippy 通过。
- [x] Android x86_64 debug APK 构建、安装和 `com.atha.reader` 冷启动通过。
- [x] AVD 配置为 4 GiB，运行时保持 API 36、x86_64、16 KiB 页。
- [x] AutoCorrect、`git diff --check` 和 required docs gate 通过。

## Files And Steps

1. 写入项目级 mise pins，安装缺失的 pnpm 版本并同步本机兼容入口。
2. 修改本机 AVD 内存，通过 user-level systemd 启动，验证 Android runtime shape。
3. 补齐 Cargo registry 缓存，执行前端、Rust 和 Android 检查。
4. 修复测试平台差异与 Linux Clippy 阻塞，重跑失败用例、完整 backend 测试和 Android smoke。

## Checks

- `mise exec -- pnpm --dir reader/app install --frozen-lockfile`
- `mise exec -- pnpm --dir reader/app check`
- `mise exec -- pnpm --dir reader/app build`
- `mise exec -- pnpm --dir reader/app test:markdown`
- `mise exec -- cargo build --workspace --locked`
- `mise exec -- cargo test --locked -p atha-backend`
- `mise exec -- cargo fmt --all -- --check`
- Android x86_64 debug build、安装和启动 smoke

## Result

项目现在可从 Bash 使用 mise 管理的 Node 24.1.0、pnpm 11.19.0 和 Temurin JDK 21 构建。Cargo 锁定依赖已补齐，workspace、frontend 和 backend 检查均通过；API 36 x86_64 16 KiB、4 GiB AVD 上的 debug APK 已安装并启动 `com.atha.reader`。

## Review

本 change 为本机开发环境与测试清理维护，不改变产品接口或数据契约。Clippy 与 Linux symlink 清理问题均已按最小平台差异修复。

## Evidence And Residual Risks

最高证据为本机 Linux Bash、本地构建和真实 x86_64 16 KiB AVD 启动；ARM64 真机、Windows 和发布包仍不在本次范围。Gradle 的 SDK XML 版本 4 警告和弃用提示未阻塞当前 debug 构建。

## Approval

用户于 2026-08-08 明确要求当前 Linux 入口使用 Bash、工具链改由 mise 管理、可安装 JDK 21、AVD 调整为 4 GiB，并立即修复到项目至少可以正常运行。

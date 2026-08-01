# PLAN-0001：Windows 后端工程初始化

## 状态

implemented

## 对应规格

`docs/specs/SPEC-0001-windows-backend-foundation.md`

## 实施方案

建立一个 virtual Cargo workspace，唯一成员为零依赖库 `atha-backend`。根 manifest 明确设置 `resolver = "3"`；workspace package 固定 `version = "0.1.0"`、`edition = "2024"`、`rust-version = "1.97.1"` 和 `publish = false`；workspace lint 设置 `unsafe_code = "forbid"`。后端 crate 逐项继承这些字段和 lint。P0 Rust crate 继续独立，不参与根 workspace 或根锁文件解析。

`rust-toolchain.toml` 固定包含 `rustfmt` 和 `clippy`。新增一个 Windows PowerShell 检查入口，复用现有 P0 脚本的 `cargo.exe` 发现方式，显式设置两个 RsProxy rustup 环境变量，依次运行规格指定的 fmt、clippy、test 和 doc 命令。doc 使用 `RUSTDOCFLAGS=-D warnings`；每一步检查 `$LASTEXITCODE`，失败时抛出包含阶段名的错误并使脚本非零退出。M1 不引入业务接口、测试框架或运行时依赖。

同时以 RsProxy 包元数据、`rusqlite` 上游源码和 SQLite 官方文档形成 SQLite 与迁移 ADR，但只记录 M2 的依赖政策，不在 M1 编译该依赖。

## 预计改动文件

- `Cargo.toml`
- `Cargo.lock`
- `rust-toolchain.toml`
- `backend/atha-backend/Cargo.toml`
- `backend/atha-backend/src/lib.rs`
- `scripts/check-backend.ps1`
- `README.md`
- `docs/ACTIVE.md`
- `docs/INDEX.md`
- `docs/codebase/DATABASE.md`
- `docs/codebase/MAP.md`
- `docs/decisions/ADR-0002-sqlite-and-migrations.md`
- `docs/milestones/M1-windows-backend-foundation.md`
- `docs/plans/PLAN-0001-windows-backend-foundation.md`
- `docs/reviews/REVIEW-0002-windows-backend-foundation.md`

## 步骤

1. 接受规格并完成本计划的独立交叉审阅。
2. 创建根 virtual workspace，写入精确的 resolver、版本、Rust、edition、publish 和 lint 字段；只纳入 `backend/atha-backend`，显式排除 `p0/ffi/rust`。
3. 创建零依赖后端库，继承 workspace package 字段和 lint，只写 crate 说明，不增加公共业务项。
4. 为固定 toolchain 增加 `clippy`；新增 `scripts/check-backend.ps1`，设置 RsProxy 变量、四条检查命令及逐阶段失败传播。
5. 基于已验证的包元数据和官方资料接受 SQLite 与迁移 ADR。
6. 生成根 `Cargo.lock`，运行 metadata、正式后端检查入口和 P0 FFI 回归。
7. 更新 README、代码地图、里程碑、索引、`ACTIVE` 和实施评审。
8. 运行文档排版、文档守卫、长度和 Git diff 检查，形成独立提交。

## 测试与检查

- 解析 `cargo metadata --format-version 1 --no-deps` JSON：workspace 只包含 `atha-backend`；包解析值为版本 `0.1.0`、Rust `1.97.1`、edition 2024，且依赖数组为空。
- 静态检查两个 manifest：根 workspace 为 `resolver = "3"` 并设置 `unsafe_code = "forbid"`；后端 crate 对版本、Rust、edition、publish 和 lint 全部使用 workspace 继承。
- 检查 `rust-toolchain.toml` 同时包含 `rustfmt` 和 `clippy`。
- 检查根 `Cargo.lock` 只含正式后端包，不含 P0、`rusqlite`、`libsqlite3-sys` 或其他外部包；P0 继续保留独立锁文件。
- 静态检查 `scripts/check-backend.ps1`：RsProxy 两个变量、四条精确命令、doc warnings-as-errors 和每阶段非零失败传播均存在。
- `pwsh -NoProfile -File scripts/check-backend.ps1`：fmt、clippy、test、doc 全部通过。
- 在隔离的子 PowerShell 中注入无效 `RUSTFLAGS` 运行检查脚本：必须在 clippy 阶段非零退出并报告阶段名；该负向探针不修改仓库文件。
- `pwsh -NoProfile -File scripts/check-p0-ffi.ps1`：CTest 1/1 和 Rust 2/2 继续通过。
- 检查 `backend/atha-backend/Cargo.toml` 无依赖段，`src/lib.rs` 无公共业务项和 unsafe；由 metadata 与根锁文件共同证明 M1 未引入或编译 SQLite。
- 检查相对提交 `5d255e4` 的变更路径，不得出现 Windows 前端、移动平台或计划外文件。
- 对本次中文 Markdown 运行 `autocorrect --fix` 和 `autocorrect --lint`。
- `python scripts/doc_guard.py`、`python scripts/doc_length_check.py`、`git diff --check`。

## 回滚方案

实施提交前可反向应用本次聚焦 diff；提交后使用新的反向提交回滚 M1。P0 目录和其独立锁文件不修改，因此无需数据或实验代码回滚。

## 风险

- Cargo 自动发现 P0：通过 workspace 成员断言和独立锁文件检查阻断。
- 空 crate 被填入占位抽象：计划只允许 crate 说明，不允许业务项。
- 检查脚本依赖调用者 PATH：复用标准 rustup 路径回退并明确失败阶段。
- SQLite 选型偷跑实现：M1 只写 ADR，不向正式 crate 添加依赖。

## 必需文档同步

- `README.md`
- `docs/ACTIVE.md`
- `docs/INDEX.md`
- `docs/codebase/DATABASE.md`
- `docs/codebase/MAP.md`
- `docs/milestones/M1-windows-backend-foundation.md`
- `docs/reviews/REVIEW-0002-windows-backend-foundation.md`

## 交叉审阅结果

- Reviewer：`/root/m1_plan_review_2`
- 状态：approved；首次 `needs changes` 的全部阻塞项已修正并通过复审。
- 阻塞问题：无。
- 非阻塞建议：可补一个隔离 PATH 的 `cargo.exe` 回退探针；不影响计划通过。仅在新 ADR 与现有数据库文档冲突时修改该文档。
- 必须修改：已补齐逐项静态与动态断言，并从实施文件清单移除已接受规格。ADR 解决了数据库基线中的未决项，因此按 reviewer 建议同步该文档。

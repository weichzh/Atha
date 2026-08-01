# SPEC-0001：Windows 后端工程初始化

## 状态

accepted

## 问题

仓库目前只有 `p0/` 技术验证。没有根 Cargo workspace、正式后端 crate、统一检查命令或生产依赖政策，因而不能在清晰边界内继续实现后端。

直接在 P0 上追加功能会混淆实验和生产；一次性生成完整后端又会在接口、依赖和目录尚未证明时制造大量占位代码。

## 目标

建立最小、可构建、可检查的正式 Rust 后端工程基线，使 M2 可以在一个清晰 seam 内实现首个纵向用例。

## 非目标

- 不实现任何产品用例、数据库行为或领域模型；
- 不加入 SQLite、序列化、日志、异步运行时或错误处理依赖；
- 不暴露业务函数、trait、adapter、FFI、HTTP 或 Tauri command；
- 不修改 P0 代码或把 P0 schema 复制进正式后端；
- 不创建 Windows 前端、移动端或 CI；
- 不决定 M2 之外的同步、AI、格式解析和发布设计。

## 用户可见行为

没有最终用户行为变化。

开发者获得一个根级检查命令，用于验证正式后端的格式、lint、测试和文档构建。命令失败时必须返回非零退出码并指出失败阶段。

## 内部行为

### Workspace

- 根 `Cargo.toml` 定义 edition 2024 workspace，并明确使用 `resolver = "3"`。
- workspace 统一版本 `0.1.0`、Rust 版本 `1.97.1`、edition 和 lint；crate 继承这些字段。
- 正式成员只有一个后端库 crate，名称为 `atha-backend`，路径为 `backend/atha-backend`。
- `p0/ffi/rust` 明确排除在正式 workspace 外，继续保留独立锁文件和验证脚本。

### 后端 module

- `atha-backend` 是未来调用者使用的 module seam。
- M1 的 crate 只包含 crate 级说明和安全编译约束，不包含公共业务接口。
- 在出现第二个 adapter 之前不定义 trait；在 M2 用例明确之前不定义错误枚举和领域占位类型。
- 禁止 unsafe，除非以后有独立规格和审阅。

### 工具链与检查

- Rust 固定为仓库当前 `rust-toolchain.toml` 版本，并包含 `rustfmt` 与 `clippy`。
- Cargo registry 使用仓库现有 RsProxy sparse 配置；检查脚本显式设置 RsProxy rustup 环境变量。
- 新增 `scripts/check-backend.ps1`，按顺序运行 `cargo fmt --all --check`、`cargo clippy --workspace --all-targets -- -D warnings`、`cargo test --workspace --all-targets` 和 warnings-as-errors 的 `cargo doc --workspace --no-deps`。
- 脚本使用当前 PowerShell 7，并可在 `cargo.exe` 不在 `PATH` 时发现标准 rustup 安装位置。

### SQLite 与迁移政策

M1 不引入 SQLite 依赖，但必须形成 accepted ADR，明确 M2 使用的随包 SQLite 方案、固定方式、所需编译特性、迁移入口和 P0 schema 的迁移原则。ADR 必须基于包元数据和官方资料，不凭版本直觉决定。

## 验收标准

- [ ] 根 `cargo metadata --no-deps` 成功，正式 workspace 只包含 `atha-backend`；
- [ ] workspace 使用 edition 2024、`resolver = "3"` 和 Rust `1.97.1`；
- [ ] `p0/ffi/rust` 仍有独立 `Cargo.lock`，不进入根锁文件；
- [ ] `atha-backend` 没有运行时依赖、公共业务接口或占位 trait；
- [ ] crate 明确禁止 unsafe；
- [ ] `rust-toolchain.toml` 包含 `rustfmt` 和 `clippy`；
- [ ] 单一 PowerShell 检查入口覆盖 fmt、clippy、test 和 doc，并在任一步失败时非零退出；
- [ ] 检查入口在 RsProxy 环境下通过；
- [ ] 现有 `scripts/check-p0-ffi.ps1` 仍通过 CTest 1/1 和 Rust 2/2；
- [ ] SQLite 与迁移政策 ADR 状态为 `accepted`，但 M1 不下载或编译 SQLite crate；
- [ ] README、`ACTIVE`、代码地图和实施评审与实际结构一致；
- [ ] 文档守卫、长度检查、中文排版和 `git diff --check` 全部通过；
- [ ] 工作树不包含 Windows 前端或移动平台文件。

## 边界情况

- `cargo.exe` 不在 `PATH`：脚本检查 rustup 标准安装位置，仍找不到则明确失败。
- `clippy` 或 `rustfmt` 未安装：由固定 toolchain 配置安装；离线缺失时明确报告，不静默跳过。
- RsProxy 暂时不可用：检查失败并保留原始错误，不自动回退到 crates.io 或其他镜像。
- P0 crate 被 Cargo 误识别为根成员：`cargo metadata` 验收必须发现该错误。
- 文档构建产生警告：按 warnings-as-errors 处理或在计划中说明工具限制。

## 风险

- 工程初始化过度：通过“一个 crate、零依赖、零业务接口”限制。
- seam 过早固定：M1 只固定 crate 位置，不固定 M2 的方法和类型。
- SQLite 决策与实现脱节：ADR 在 M1 接受，但依赖和代码只在 M2 规格中进入。
- Windows 脚本不可移植：当前实施平台就是 Windows；未来跨主机需求再添加对应入口。

## 相关文档

- 里程碑：`docs/milestones/M1-windows-backend-foundation.md`
- 决策：`docs/decisions/ADR-0001-windows-backend-first.md`
- 架构：`docs/architecture/OVERVIEW.md`
- 代码库：`docs/codebase/MAP.md`
- 数据库：`docs/codebase/DATABASE.md`
- 路线图：`docs/roadmap/ROADMAP.md`

## 自审

- 歧义：正式 crate 名、位置、P0 隔离和检查范围均已明确；SQLite 精确 crate/版本刻意留给有证据的 ADR。
- 缺失的验收标准：未发现；实现阶段仍需记录实际 Cargo metadata 和检查输出。
- 范围问题：CI、业务行为和前端已明确排除。
- 风险判断：低风险、可完全回滚；主要风险是偷跑 M2，验收标准已禁止依赖和业务占位。
- 是否可接受：yes；用户于 2026-08-01 接受规格并批准开始 M1 实施。

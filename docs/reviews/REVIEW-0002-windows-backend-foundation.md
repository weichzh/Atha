# REVIEW-0002：Windows 后端工程初始化

## 范围

审阅 `SPEC-0001` 和 `PLAN-0001` 定义的 M1：根 Cargo workspace、零依赖后端 crate、Windows 检查入口、P0 隔离及 SQLite 与迁移政策。

## Diff 摘要

- 新增一个正式 virtual workspace 和一个零依赖库 crate；
- 固定 Rust `1.97.1`、edition 2024、resolver 3、rustfmt、clippy 和禁止 unsafe 的 workspace lint；
- 新增正式后端统一检查脚本；
- 接受随包 SQLite 与仅向前迁移 ADR；
- 同步 README、项目索引、代码地图、数据库基线、里程碑和 `ACTIVE`。

## 已执行检查

- [x] `cargo metadata --format-version 1 --no-deps`：一个 workspace 成员、一个零依赖包；
- [x] `pwsh -NoProfile -File scripts/check-backend.ps1`：fmt、clippy、test、doc 通过；
- [x] 隔离负向探针：无效 `RUSTFLAGS` 在 clippy 阶段非零退出并报告阶段；
- [x] `pwsh -NoProfile -File scripts/check-p0-ffi.ps1`：CTest 1/1、Rust 2/2 通过；
- [x] workspace 精确字段、crate 继承、根锁文件和 P0 独立锁文件静态断言通过；
- [x] 符合规格；
- [x] 遵循已独立交叉审阅的计划；
- [x] 文档已更新；
- [x] `ACTIVE` 已更新；
- [x] 代码地图和数据库基线已更新；
- [x] `scripts/doc_guard.py` 通过；
- [x] `scripts/doc_length_check.py` 通过；
- [x] 中文 Markdown 排版与暂存区 `git diff --check` 通过。

## 发现

- 初次计划审阅遗漏了部分验收断言；修订后由 `/root/m1_plan_review_2` 复审为 `approved`。
- 后端 crate 有意运行零项业务测试；M1 没有非平凡业务逻辑，构建、lint 和文档检查是对应证据。
- 当前 `cargo.exe` 不在 PATH，成功检查已实际覆盖标准 rustup 路径回退。
- 只读依赖探针把 registry 元数据和包源码放入用户 Cargo cache；正式 workspace 没有解析或编译 SQLite。ADR 的版本和 feature 证据来自该探针、上游源码和 SQLite 官方文档。

## 后续

M2 必须先建立独立规格，才能添加 `rusqlite`、迁移和首个业务用例。不得在 M1 空 crate 中预留接口。

## 结论

approved。最高证据等级为 Windows 本地；未覆盖真实产品链路或生产等价环境，因为 M1 不包含产品行为。

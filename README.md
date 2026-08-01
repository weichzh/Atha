# Atha

Atha 是一个本地优先、以消息形式保存阅读反应的个人阅读系统。

当前只推进 Windows，并遵循“后端先于前端”。正式 Rust 后端工程基线已经建立，但尚无产品用例；移动平台和 Windows 前端仍未开始。现有 `p0/` 只用于 FFI 与 SQLite 技术验证，不属于正式后端。

## 工程入口

- 根 `Cargo.toml`：正式 workspace；
- `backend/atha-backend/`：零依赖后端库；
- `scripts/check-backend.ps1`：fmt、clippy、test 和 doc 统一检查；
- `p0/`：独立实验，不进入根 workspace。

```powershell
pwsh -NoProfile -File .\scripts\check-backend.ps1
```

## 本地开发环境

每台电脑在开始开发前复制 `env/example.ps1` 为 `env/local.ps1`，并填写本机的 `cargo`、`cmake`、`ctest` 和 `sqlite3` 路径。`env/local.ps1` 已被 Git 忽略；检查脚本统一加载它，不依赖当前 Shell 的 `PATH`。

## 项目入口

- 当前状态：`docs/ACTIVE.md`
- 文档索引：`docs/INDEX.md`
- 架构总览：`docs/architecture/OVERVIEW.md`
- 路线图：`docs/roadmap/ROADMAP.md`
- 协作规则：`AGENTS.md`

生产代码必须经过规格、计划、交叉审阅和用户批准门禁。

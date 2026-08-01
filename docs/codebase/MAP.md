# 代码库地图

## 仓库状态

- 分支：`main`
- Git 初始化提交：`8baa176`
- SQLite P0 提交：`840cdea`
- M0 工作流提交：`fc104e0`
- M1 规格提交：`5d255e4`
- 当前没有远程仓库。
- 当前已有根 Cargo workspace 和正式后端 crate；没有前端工程。

## 顶层结构

| 路径 | 责任 | 状态 |
|---|---|---|
| `.cargo/config.toml` | RsProxy sparse index 与 Cargo 网络配置 | 已配置 |
| `Cargo.toml`、`Cargo.lock` | 正式 virtual workspace 与锁文件 | M1 已验证 |
| `backend/atha-backend/` | 正式零依赖后端库 | M1 工程基线 |
| `p0/ffi/` | Rust/C++ 共享 C ABI 调用与所有权对照 | 本地 P0 实验 |
| `p0/sqlite/` | SQLite、FTS5、Outbox schema 与故障检查 | 本地 P0 实验 |
| `scripts/check-backend.ps1` | 正式后端 fmt、clippy、test 和 doc | M1 已通过 |
| `scripts/check-p0-ffi.ps1` | 构建两个 FFI 实现并运行统一 runner | 已通过 |
| `scripts/check-p0-sqlite.ps1` | 重建数据库并验证事务、FTS 与 10k 冒烟 | 已通过 |
| `docs/` | 项目权威记忆、规格、计划、决策和评审 | 已建立 |

`p0/` 只保存技术验证，不是生产后端。后续正式代码不得直接在 P0 目录上堆叠。

### 正式后端基线

- workspace 只有 `atha-backend` 一个成员，并显式排除 P0 Rust crate；
- 版本 `0.1.0`、edition 2024、Rust `1.97.1` 和禁止 unsafe 的 lint 由 workspace 统一；
- 后端 crate 没有外部依赖、公共业务接口或占位 trait；
- 根锁文件只包含正式后端包，P0 继续保留独立锁文件；
- SQLite 与迁移政策已固定，但数据库依赖和实现延后到 M2。

## 已实现能力

### FFI 对照

- 共享 C 头文件；
- C++ 与 Rust 动态库；
- ABI 版本、空调用、1 MiB 字节校验、字符串跨边界分配与释放；
- 统一动态加载 runner；
- Rust 单元测试与 CTest。

### SQLite 对照

- `Work`、`Edition`、`Conversation`、`Message`、`MessageRevision`、`SourceAnchor` 与 `OutboxEvent` 骨架；
- WAL、外键、FTS5 外部内容表和同步触发器；
- 当前修订归属外键；
- 强制 Outbox 失败后的整事务回滚验证；
- 10,000 消息、修订和 Outbox 的本地冒烟。

## 最近验证基线

证据等级均为 Windows 本地：

- MSVC 19.51 与 CMake 4.4.1 构建通过；
- Rust 1.97.1 单元测试 2/2 通过；
- CTest 1/1 通过；
- 正式后端 fmt、clippy、零测试编译和 warnings-as-errors 文档构建通过；
- metadata 证明正式 workspace 只有一个零依赖包；
- 负向探针证明 clippy 失败时检查脚本非零退出并报告阶段；
- Rust/C++ 10,000 次空 FFI 调用中位数均约 1.13 ns/次；
- 系统 SQLite 3.53.4 上回滚、FTS 完整性、外键和数据库完整性检查通过；
- B-DB-001 单次本地冒烟约 150 ms，不是正式性能结论。

## 已知缺口

- P0 schema 含 SQLite CLI 指令，尚未转为正式版本化迁移；
- 正式后端尚未添加或编译已决策的随包 SQLite；
- 没有应用服务、领域 API、错误模型或跨进程接口；
- 没有导入解析、Locator 重锚定或富文本迁移；
- 没有 CI、Windows 安装包或真实产品链路；
- 性能数据未记录设备指纹，也没有重复样本统计。

## 正式代码约定

正式后端使用 `backend/`，测试靠近所属 crate；P0 实验继续保留在 `p0/`。新增 module 或依赖必须由后续已接受规格和计划驱动，不能用空骨架预留。

## 相关文档

- 架构：`docs/architecture/OVERVIEW.md`
- 数据库：`docs/codebase/DATABASE.md`
- 路线图：`docs/roadmap/ROADMAP.md`

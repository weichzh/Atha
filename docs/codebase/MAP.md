# 代码库地图

## 仓库状态

- 分支：`main`
- Git 初始化提交：`8baa176`
- SQLite P0 提交：`840cdea`
- M0 工作流提交：`fc104e0`
- M1 规格提交：`5d255e4`
- 远程仓库：`github.com/weichzh/Atha`。
- 当前已有根 Cargo workspace、正式后端 crate、Windows WebView2 阅读 host 与原生 HTML/CSS/JavaScript 阅读页；没有前端框架。

## 顶层结构

| 路径 | 责任 | 状态 |
|---|---|---|
| `.cargo/config.toml` | RsProxy sparse index 与 Cargo 网络配置 | 已配置 |
| `Cargo.toml`、`Cargo.lock` | 正式 virtual workspace 与锁文件 | M2 已验证 |
| `backend/atha-backend/` | 正式零依赖后端库、书根资源边界与阅读遥测校验 | M2 阅读切片 |
| `reader/atha-reader-host/src/` | Wry/Tao Windows WebView2 承载；入口、启动参数、受控协议和诊断按职责分离 | M2 R2 已验证 |
| `reader/atha-reader.html`、`reader/atha-reader.css` | 唯一阅读页结构、默认样式与原生阅读偏好面板 | M2 R3 已验证 |
| `reader/web/` | Locator、导航、偏好、阅读会话、内容安全与加载、分页与公式布局、诊断与 benchmark、页面组合入口 | M2 R3 已验证 |
| `reader/samples.json` | 四个本地验收样本的入口、manifest、边界与内容断言清单 | M2 R1 已验证 |
| `p0/ffi/` | Rust/C++ 共享 C ABI 调用与所有权对照 | 本地 P0 实验 |
| `p0/sqlite/` | SQLite、FTS5、Outbox schema 与故障检查 | 本地 P0 实验 |
| `scripts/check-backend.ps1` | 正式后端 fmt、clippy、test 和 doc | M1 已通过 |
| `scripts/check-p0-ffi.ps1` | 构建两个 FFI 实现并运行统一 runner | 已通过 |
| `scripts/check-p0-sqlite.ps1` | 重建数据库并验证事务、FTS 与 10k 冒烟 | 已通过 |
| `scripts/check-reader-slice.ps1` | 构建实际 host，运行安全、布局和性能验收 | M2 已通过 |
| `scripts/export_reader_sample.py` | 安全、可重复地从 EPUB 导出单章节或带 manifest 的多章节验收样本 | M2 R1 已通过 |
| `scripts/Serve-ReaderValidation.ps1` | 只读环回提供同一阅读页、manifest 和书根资源 | M2 R1 已通过 |
| `scripts/check-reader-samples.ps1` | 四样本实际 host、切章释放与明暗主题截图总验收 | M2 R1 已通过 |
| `scripts/Invoke-Atha.ps1` | 统一工程 CLI；自动记录 `check docs`、`station` 与 `report` | 本地已验证 |
| `scripts/Measure-Workflow.ps1` | schema v1/v2 本机流程日志、兼容汇总与自检 | 本地已验证 |
| `docs/agents/workflow.md` | 全局工作流的项目契约、任务类型和真实检查 gate | 已配置 |
| `docs/` | 项目权威记忆、规格、计划、决策和评审 | 已建立 |

`p0/` 只保存技术验证，不是生产后端。后续正式代码不得直接在 P0 目录上堆叠。

### 正式后端基线

- workspace 包含 `atha-backend` 与 `atha-reader-host`，并显式排除 P0 Rust crate；
- 版本 `0.1.0`、edition 2024、Rust `1.97.1` 和禁止 unsafe 的 lint 由 workspace 统一；
- 后端 crate 没有外部依赖、公共业务接口或占位 trait；
- 根锁文件包含正式后端与固定版本的 Wry/Tao 承载依赖，P0 继续保留独立锁文件；
- SQLite 与迁移政策已固定，但数据库依赖和实现留待后续数据库里程碑。

### HTML 阅读切片

- `BookRoot` 规范化书根并拒绝编码、路径、符号链接、文件类型、MIME 与大小越界；
- schema 1 manifest 声明内容版本、有序 section、资源和可选 TOC；Windows host 的 `--manifest` 与兼容 `--entry` 互斥；
- `atha` 与 `atha-book` 自定义协议只提供应用资源和当前书根资源；导航、新窗口、下载与外部请求默认拒绝；
- 原生 host 的 `main.rs` 只选择 Windows 入口；`windows.rs` 组合事件循环，`launch`、`protocol` 与 `diagnostics` module 分别拥有参数和窗口、受控资源、日志与 benchmark；
- 阅读页源码保持原生 ES module：`locator` 校验、序列化并比较内容坐标；`navigation` 组合页、section、TOC 与重排恢复；`preferences` 合并应用默认与本书样式；`session` 拥有 manifest 和内容生命周期；`content` 校验并加载单份 XHTML、CSS 与 SVG；`pagination` 负责公式与固定页面布局；`diagnostics` 负责自检、benchmark 和仅验证模式可见的只读快照；`app` 只组合打开流程；
- 八份页面源码由应用资源协议按固定顺序交付为单个 `atha-reader.mjs`，避免为源码分层增加多次自定义协议请求；浏览器验证服务器使用同一顺序；
- Locator 以内容版本、section id 和 DOM 文本 UTF-16 偏移表示 point/range；R2 range 限于单 section 并检查实际文本边界，无效输入安全回落并留下诊断，页码不作为内容坐标；
- 上一页和下一页可跨 section，manifest TOC 通过原生 `select` 跳转；字号重排按变化前 Locator 恢复到包含同一偏移的页面；
- 应用默认拥有主题、字号、字体与紧凑/标准/舒展密度；本书覆盖只拥有书源样式和安全用户 CSS，均可独立恢复且暂不持久化；
- 公式按源尺寸随字号缩放，行间公式使用独立 `1.5` 倍率并在逻辑内容列中居中；
- 固定 1264 × 1680 设备像素页使用 CSS 多栏，并以 `1 / devicePixelRatio` 隔离系统 DPI；文字、公式和原子内容均有布局后裁切检查；
- Windows 窗口与壳层控件使用系统逻辑像素，当前默认窗口按固定页面设备像素换算并限制在屏幕逻辑宽高的 80%；
- 宿主 IPC 只接收固定、限长、非内容性的性能与状态事件。

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

- 统一工程 CLI 的 station、`check docs`、schema v1/v2 混合 report、受控失败传播和非法参数拒绝通过；
- MSVC 19.51 与 CMake 4.4.1 构建通过；
- Rust 1.97.1 单元测试 2/2 通过；
- CTest 1/1 通过；
- 正式后端 fmt、clippy、零测试编译和 warnings-as-errors 文档构建通过；
- M2 Rust 资源与遥测集成测试 3/3 通过；实际 Windows WebView2 host 在 24/32/40px 下完成公式、安全与分页自检；
- 当前 4K、200% DPI 环境实测页面为 1264 × 1680 设备像素、默认客户区为 680 × 816 逻辑像素、窗口外框为屏幕高度的 78.3%；工具栏按钮为 44px 逻辑高度；
- 行间公式 6/6 在 32px 下为 `3×` 源尺寸、中心偏差 0px、宽高比误差 0；Agent Browser 首尾翻页与 40px 重排截图通过；
- 四样本实际 host 与 Agent Browser 明暗验收通过：既有三样本保持原内容断言；《数学及其历史》R1 样本依次加载三个标题、两次释放旧 DOM、关闭后重新打开首章，首章含 23 个公式和 2 张普通 PNG；
- R2 在实际浏览器验证 Locator 往返与 range 边界、32→40→24→32px 逐次位置恢复、TOC 控件切章、并发导航串行化、section 首尾导航和错版本安全回落；
- R3 在实际浏览器验证系统/亮/暗主题、书源/衬线/无衬线字体、三档绝对密度、24/32/40px 字号、Locator 恢复、书源与用户样式启停、安全 CSS 拒绝和实际偏好控件；
- 明暗正文对比度分别为 15.94 和 13.84；暗色下只反色公式，普通图始终为 `filter: none`；
- R1 最终 10 样本基准中位数：冷启动 772.623ms、首个稳定页面 166.300ms、热打开 20.700ms、翻页 6.300ms、字号重排 20.800ms；该轮只证明指标与既有样本保持有效，未执行旧代码的同时间受控对照，不能归因于本次改动；
- R2 最终 10 样本基准中位数：冷启动 666.998ms、首个稳定页面 177.850ms、热打开 20.800ms、翻页 6.300ms、字号重排 27.800ms；字号重排包含文本位置捕获与恢复，仍在正式门槛内，未执行旧代码同时间对照；
- R3 最终 10 样本基准中位数：冷启动 885.044ms、首个稳定页面 209.500ms、热打开 20.800ms、翻页 6.250ms、字号重排 27.800ms；未执行旧代码同时间对照；
- metadata 证明正式后端仍是 workspace 中唯一零依赖包；
- 负向探针证明 clippy 失败时检查脚本非零退出并报告阶段；
- Rust/C++ 10,000 次空 FFI 调用中位数均约 1.13 ns/次；
- 系统 SQLite 3.53.4 上回滚、FTS 完整性、外键和数据库完整性检查通过；
- B-DB-001 单次本地冒烟约 150 ms，不是正式性能结论。

## 已知缺口

- P0 schema 含 SQLite CLI 指令，尚未转为正式版本化迁移；
- 正式后端尚未添加或编译已决策的随包 SQLite；
- 除受限阅读遥测外，没有应用服务、领域 API 或通用跨进程接口；
- 没有导入解析、跨内容版本 Locator 重锚定或富文本迁移；
- 没有 CI、Windows 安装包或书籍导入产品链路；
- 性能数据未记录设备指纹，也没有跨日期重复运行统计。

## 正式代码约定

正式后端使用 `backend/`，测试靠近所属 crate；P0 实验继续保留在 `p0/`。新增 module 或依赖必须由后续已接受规格和计划驱动，不能用空骨架预留。

## 相关文档

- 架构：`docs/architecture/OVERVIEW.md`
- 数据库：`docs/codebase/DATABASE.md`
- 路线图：`docs/roadmap/ROADMAP.md`

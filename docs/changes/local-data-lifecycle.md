---
description: 完整本地数据备份、恢复、占用统计和书籍两级删除的实施与验收记录。
---

# 完整本地数据生命周期

## Status

accepted

用户已批准按路线图进入实现；本 change 将“完整本地数据生命周期”收敛为一个可独立验收的纵向切片。

## Problem

Atha 当前只有 MessageStore 完整备份，书架、耐久书源、离线词典和 WebView 本地阅读状态仍分散在各自数据边界。用户看不到各类数据占用，也只能“移出书架”，不能明确选择是否同时删除书籍文件和阅读状态。直接复制应用目录会遗漏 SQLite WAL、带入可重建缓存与日志，也不能安全恢复不可信制品。

## Scope

- 新增 schema 1 `.atha-data` 单文件资料库备份，包含规范化书架记录、全部耐久书源、离线词典、现有 MessageStore 完整备份和生产阅读状态；
- 明确排除 `ImportedBooks`、Picker 临时文件、日志及其他可重建缓存；
- 恢复先验证唯一 entry、路径、数量、容量、哈希、书架与书源身份、词典结构、MessageStore 和浏览器状态，再发布到当前数据根；
- 用同文件系统 staging、rollback、恢复日志和既有 SQLite Online Backup API 协调多根恢复；进程中止后在下次启动回滚未提交恢复，已提交但未确认的浏览器状态继续完成；
- 书架管理显示书籍、阅读缓存、消息、词典和阅读设置的本地占用；
- 保留“移出书架”现有语义；新增“删除本地数据”，删除书架记录、耐久书源、导入缓存和本书阅读状态，但保留 Message、SourceAnchor 与 SourceSnapshot，供后续跨书记忆在书籍缺失时继续读取；
- 备份、恢复、占用与删除只允许主窗口书架根路由调用，日志只记录固定阶段、计数、字节数、耗时和错误码。

## Non-goals

- 不做账户、云同步、跨设备合并、增量备份、定时任务、加密或真实性签名；
- 不恢复或备份可重建 `ImportedBooks`，不新增缓存清理页；
- 不物理删除消息、历史快照或消息资产，不改变 Message 墓碑语义；
- 不兼容导入旧的“仅消息”`.atha-backup` 为完整资料库；原 MessageStore 接口与制品保持可用但不再作为书架主入口。

## Acceptance Criteria

1. 从含书架、耐久书源、离线词典、消息和生产 localStorage 状态的数据根创建 `.atha-data` 后，在另一空数据根恢复得到同一书架、书源、词典、消息与阅读状态；`ImportedBooks` 为空，首次打开可从耐久源重建。
2. archive 出现未知 / 重复 / overlapping entry、路径越界、容量越界、哈希不符、损坏书架记录、错误书源身份、损坏词典、无效 MessageStore 或非法浏览器状态时，在任何正式写入前稳定拒绝。
3. 恢复发布前写入持久恢复日志和旧 MessageStore 备份；受控发布失败或进程在提交前终止时，下次启动恢复原目录与消息。后端提交后浏览器状态应用失败时可显式回滚；未确认提交在重启后继续应用并最终清理 rollback。
4. 占用统计的分类字节和等于总字节；不跟随 symlink，不输出路径或内容。备份总解压上限为 8 GiB，浏览器状态最多 16 MiB，文件数最多 100,000。
5. “移出书架”仍只删除书架记录；重新导入相同内容恢复原身份与阅读状态。“删除本地数据”删除记录、全部同身份耐久源、导入缓存和本书偏好 / 书签 / 进度 / 旧标注及本书统计项，保留全局偏好、其他书数据、消息与快照。
6. 书架 UI 提供资料库备份、恢复和存储占用；批量选择同时提供语义和确认文案不同的“移出书架”与“删除本地数据”，移动 / 桌面视口无文字遮挡或横向溢出。
7. backend、前端状态事务、Tauri origin / command seam、Svelte check / build、Linux Tauri 书架链路和 required docs gate 通过；Android 只复用已验证 SAF bridge，本 change 不把本地或 Linux 结果称为 PCT-AL10 验收。

## Architecture Impact

present

- **设计目的与退出条件**：建立唯一应用级本地数据生命周期边界；成功往返、恶意制品拒绝、受控失败 / 重启恢复和两级删除语义均可执行后停止。
- **驱动因素**：A-DATA-01 数据中止恢复、A-PRIV-01 内容隐私，以及本地优先、可迁移和缓存可重建约束。
- **Module / Interface / Seam**：新增 backend `local_data` Module 协调现有 `LocalLibrary`、`LocalDictionaries`、`MessageStore`，但不接管各自事实；Tauri Adapter 继续拥有 dialog、SAF 和书架 origin；Svelte 只捕获、事务替换 allowlist localStorage 并呈现状态。
- **数据与原子性**：`.atha-data` 是新的持久化交换接口；目录以 staging / rollback rename 发布，MessageStore 仍只经既有 `.atha-backup` 和 Online Backup API 恢复。恢复日志在第一次正式 rename 前耐久发布，`prepared` 一律回滚，`committed` 等待浏览器状态确认。
- **替代方案**：直接复制整个应用目录实现更短，但会复制 WAL、日志和缓存，且不能校验不可信输入；给每个 Module 单独暴露多个备份按钮会把一致性和恢复责任推给用户。采用单一协调包，复用现有 ZIP、SHA-256、文件 picker 和 MessageStore 备份，不新增依赖、repository、provider 或同步 schema。
- **证据计划**：Rust 往返 / 损坏 / 发布恢复测试，Node localStorage 事务测试，Tauri command / origin 测试，Svelte 静态检查与 Linux 真壳书架验收。持久接口与中止恢复决定记录在 `ADR-0010`。
- **复查触发器**：备份接近 8 GiB / 100,000 文件，需要取消或进度；数据根出现第二写入进程；要求加密、合并、旧 schema 迁移、云同步或正常消息物理删除。

### Quality Scenarios

- `LD-DATA-01`：用户在正常单实例书架页恢复有效备份；刺激为选择制品；制品为书架、书源、词典、MessageStore 与浏览器状态；响应为完整校验后发布并重载；度量为成功后所有耐久事实相等、导入缓存为零、首次打开可重建。
- `LD-DATA-02`：进程或注入 I/O 在恢复发布期间中止；环境为 staging 已完成、提交未完成；制品为当前数据根；响应为下次启动依据恢复日志回滚或完成；度量为只观察到恢复前或恢复后完整集合，数据库完整性与外键检查通过。
- `LD-SEC-01`：本地不可信备份包含越界、重复、重叠、超限或语义伪造数据；环境为恢复前；响应为拒绝且不改当前数据；度量为正式目录、消息数据库和浏览器状态零变更。
- `LD-PRIV-01`：任一生命周期操作失败；环境为产品日志开启；响应为固定阶段与错误码；度量为日志中书名、路径、正文、查询、笔记和内容哈希为零。

## Files And Steps

1. 新增 `backend/atha-backend/src/local_data.rs` 与 focused tests；为书架、词典和 MessageStore 增加最窄的校验 / 快照复用点。
2. 新增 Tauri `local_data_maintenance` commands，复用 `platform_file` 和书架 origin；把数据根与恢复状态接入启动顺序。
3. 在 `library.ts` 实现 allowlist 浏览器状态捕获、替换、回滚与按书清理；在 `LibraryView.svelte` / `library.css` 完成管理、占用和两级删除界面。
4. 增加单一 Bash 检查入口，更新架构、代码地图、数据库边界、路线图和 `ACTIVE`。
5. 运行独立 review，修复 blocking 项后提交、重跑 gate、关闭 change。

## Checks

- `bash scripts/check-local-data.sh`；
- `bash scripts/check-reader-linux.sh` 的书架场景；
- `autocorrect --fix/--lint` 仅针对本次中文 Markdown；
- `project_workflow.py station <task> --activity verification --gate docs`。

## Result

待实施。

## Review

待实施后独立评审。

## Evidence And Residual Risks

- 当前批准只建立本地 / Linux 证据；PCT-AL10 的 SAF 资料库往返与自然触摸不在本 change 的完成声明内。
- 外部 Android provider 在最终 content URI 复制失败时仍可能留下不完整外部文档；应用自身 Picker cache 会清理，但不能替 provider 删除残留。
- schema 1 不加密、不签名，也不跨版本猜测迁移；用户必须自行保护备份文件。

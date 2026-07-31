# ARCHIVE-0001：移动端架构提案 v0.1

## 状态

historical / non-authoritative

## 来源

- 原文件：`Atha_移动端产品与技术架构方案_v0.1.md`
- 日期：2026-08-01
- 首次纳入 Git：提交 `8baa176`
- 完整原文：保留在 Git 历史的上述提交中。

## 原提案范围

原提案以 iOS 和 HarmonyOS 为首发目标，推荐原生平台壳、Readium、Reader Kit、稳定 C ABI、SQLite/FTS5/Outbox、消息式笔记和真机性能门禁，并将 Windows 明确排除在首发范围之外。

## 为什么不再权威

用户已明确将当前范围改为 Windows，并要求后端先于前端。`docs/decisions/ADR-0001-windows-backend-first.md` 覆盖原提案中的平台和实施顺序。

## 仍可复用的结论

- 本地优先与用户数据可导出；
- `Work`、`Edition`、`Message`、`MessageRevision` 与 `SourceAnchor` 语义；
- 规范 Locator 与平台快速定位数据并存；
- SQLite WAL、FTS5、单写事务与 Outbox；
- 当前修订、编辑历史、删除墓碑和稳定消息 ID；
- 性能指标必须定义起点和终点；
- 生产日志禁止记录阅读内容；
- 不可信出版物需要资源上限和路径逃逸防护。

## 不得直接沿用的内容

- iOS/HarmonyOS 首发结论；
- Readium、Reader Kit、ArkUI-X、RNOH 和移动真机矩阵；
- 移动端最低版本与发布地区假设；
- 未经当前 Windows 证据复核的性能预算；
- P2/P3 的格式、同步和 AI 时间表。

## 当前权威入口

- `docs/ACTIVE.md`
- `docs/architecture/OVERVIEW.md`
- `docs/decisions/ADR-0001-windows-backend-first.md`
- `docs/roadmap/ROADMAP.md`

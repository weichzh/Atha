# M1：Windows 后端工程初始化

## 状态

active

## 日期

- 开始：2026-08-01
- 目标完成：
- 完成：

## 目标

建立正式根 Cargo workspace、最小后端 crate 和单一验证入口。M1 只建立工程基线，不实现产品行为。

## 范围

- 根 Cargo workspace；
- 一个正式后端库 crate；
- Rust 版本、格式化和 lint 规则；
- Windows PowerShell 验证入口；
- P0 实验与正式代码的隔离规则；
- 固定 SQLite 和迁移入口的书面决策；
- 对应的代码地图、评审和 `ACTIVE` 更新。

## 非目标

- 数据库 schema 或迁移实现；
- 书籍、消息、Locator、Outbox 或搜索行为；
- C ABI、Tauri、HTTP 或其他传输层；
- Windows 前端和任何移动平台工程；
- CI、安装包和发布。

## 退出条件

- [ ] 规格状态为 `accepted`；
- [ ] 计划已交叉审阅且状态为 `accepted`；
- [ ] 根 workspace 和后端 crate 可重复构建；
- [ ] 后端 crate 没有业务占位接口和未使用依赖；
- [ ] 单一检查入口覆盖 fmt、clippy、test 和 doc；
- [ ] P0 FFI 验证仍通过；
- [ ] SQLite 与迁移政策有 accepted ADR；
- [ ] 文档守卫、长度和 diff 检查通过；
- [ ] 实施评审 approved 并形成独立提交。

## 活跃文档

- 规格：`docs/specs/SPEC-0001-windows-backend-foundation.md`
- 计划：规格接受后创建。
- 决策：`docs/decisions/ADR-0001-windows-backend-first.md`
- 评审：实施后创建。

## 风险

- 空 crate 若暴露占位接口，会过早固定错误的 seam。
- 把 P0 纳入正式 workspace 会让实验依赖污染生产锁文件。
- 在 M1 引入 SQLite crate 会产生未使用依赖并偷跑 M2 范围。

## 说明

M1 采用深 module 原则：先建立一个小而稳定的 crate seam，不创建只有一个 adapter 的 trait，也不为未来功能预留空接口。

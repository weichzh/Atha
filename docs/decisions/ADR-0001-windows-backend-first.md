# ADR-0001：Windows 后端优先

## 状态

accepted

## 背景

初始 v0.1 提案以 iOS 与 HarmonyOS 为首发目标，并要求在 P0 同时验证移动阅读引擎。用户随后明确调整范围：当前只考虑 Windows，且应先完成后端，再考虑前端；Windows 窗口未来可以采用移动端式布局。

用户同时指定 Rust 工具链和 crates 下载使用 RsProxy。

## 决策

1. 当前唯一实施平台为 Windows。
2. 后端项目初始化、领域边界、数据可靠性和可测试接口先于任何前端实现。
3. 后端形成可调用纵向切片之前，不创建 Windows 前端工程。
4. iOS、HarmonyOS 与移动端适配代码全部暂缓；相关旧结论仅作为研究资料。
5. Windows 前端可以采用窄窗口和移动端信息密度，但不得改变平台无关的领域语义。
6. Rustup 使用 `https://rsproxy.cn`，Cargo 使用 RsProxy sparse index；仓库保留项目级配置，用户环境也使用同一镜像。

## 影响

- 当前不需要 Xcode、DevEco、Reader Kit 或移动真机。
- 原移动端架构提案不再是权威路线图。
- 后端接口必须保持与 UI 解耦，为以后 Windows 前端和可能恢复的移动端留出稳定边界。
- 前端视觉概念不能先于后端契约成为实现约束。
- 每个生产变更仍需经过规格、计划、交叉审阅和验证门禁。

## 备选方案

- 同时初始化 Windows 前端与后端：否决，边界尚未稳定，会制造无效骨架。
- 继续移动端 P0：否决，与当前用户范围冲突。
- 直接做单体桌面原型：否决，难以验证数据可靠性与可替换 UI 边界。

## 相关文档

- 里程碑：`docs/milestones/M0-current.md`
- 架构：`docs/architecture/OVERVIEW.md`
- 路线图：`docs/roadmap/ROADMAP.md`
- 历史研究：`docs/studies/ARCHIVE-0001-mobile-architecture-v0.1.md`

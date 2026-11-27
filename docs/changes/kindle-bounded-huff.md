---
description: 按实际正文输出和重组成本限制 Kindle HUFF/CDIC 书籍导入。
---

# Kindle 有界导入

## Status

implemented

## Problem

图片等非正文数据曾被计入 HUFF 扩展估算，导致部分正文较短的书过早拒绝。仅放宽整文件估算会给解压、结构重组与链接投影留下过宽的内存和工作预算。

## Scope

从已有本地实现独立交付普通 MOBI / 纯 KF8 的有界 HUFF/CDIC 导入、共享词典解码器、安全的图片和目录投影、HTML5 字符引用文本转换及公开合成测试。保留旧入口条件、源身份、缓存兼容、越根/未声明资源/不支持协议拒绝。公开 PR 不包含真实书源或测试清单。

## Architecture Impact

present

- 目的：对实际输出和工作量计量，在依赖读取之前核对结构和资源边界。
- 接口：Atha 在隔离暂存区保留原记录编号，完成受限正文扩展；`boko 0.5.0` raw importer 带原资源和目录语义进入现有安全投影与发布。
- 权衡：采用暂存路径和预检，不 fork 依赖，也不对旧入口或已验证缓存施加新条件。目录 guide 指向已存在的片段；不猜测未声明资源。
- 证据：合成声明/计量边界与资源、锚点、发布回归。出现新编码或要求变更输入信任边界时复核。

## Acceptance Criteria

- [x] 对读取、展开、字典缓存、KF8 重组、记录与章节投影施加可验证的限额。
- [x] 合成 MOBI / KF8 通过登记、开书、章节与资源读取，三种后缀同身份；越界和不支持内容仍以既有规则拒绝。
- [x] [PR #7](https://github.com/weichzh/Atha/pull/7) 已合入 `8b30352`；真实样本与跨端验收仍由 [Issue #2](https://github.com/weichzh/Atha/issues/2) 跟进。

## Files And Steps

从本地既有 Kindle 模块提取独立差异，补齐依赖、许可与合成用例；书架分类和阅读器界面由各自 PR 验证。

## Checks

Kindle 相关独立 Rust 单元和合成 importer 测试、共享词典解码回归、公开 corpus probe 以及文档检查。

## Iteration Control

- Mode: `iterative`
- Maximum repair iterations: `3`
- Unchanged-delta limit: `1`
- Human stop: 真实样本无法验证时记录证据缺口；输入映射不唯一时保留拒绝。

## Result

独立候选沿用有界 HUFF / KF8 投影与共享词典解码，未纳入阅读器、书架分类或新目录字段。读取、结构重组、输出和复制工作均在进入对应扩展路径前计量。

公开检查：27 项阅读器 Rust 单元、8 项 Kindle 导入、3 项 corpus probe 和 8 项 Node corpus runner 测试通过；原样本 opt-in 按默认跳过。库单测串行 36 项通过、3 项忽略。同一候选的并行库单测曾在两项快照资产测试的初始数据库打开处失败，另核对并行夹具与主线基线。

## Review

代码与安全复审覆盖 `77ce1fe`。库单测的快照初始化失败在主线和候选对照运行中未复现。新版书架基线上复验合成 MOBI/KF8、EXTH 分类、资源拒绝和共享词典解码通过。

## Evidence And Residual Risks

非公开书库收据留在本机。KF8 flow 样式、多头 combo 和不支持的外部引用维持原边界；本候选的自动化证据与真实目标验收分开。

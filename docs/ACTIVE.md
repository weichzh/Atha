# 当前工作

- `change`：本地提审检查与发版验收 → `docs/changes/local-ci.md`
- `change`：正式开发基线收尾 → `docs/changes/repository-readiness.md`

本轮书架、阅读器和验证工具的实现已合入；后续按以下入口验收：

- [Issue #1：完整阅读体验](https://github.com/weichzh/Atha/issues/1) — 用同一原创 EPUB 在目标 WebView 串起导入、找书、阅读、选区、笔记和原文回跳。当前实现见 [阅读界面代码地图](codebase/READER-MOBILE-UI.md)；[产品假设](product/OVERVIEW.md)中的 `H1-H4` 仍未决。
- [Issue #2：真实书库与阅读资产耐久性](https://github.com/weichzh/Atha/issues/2) — 用隔离资料目录验证锚点、笔记及快照在重启和备份恢复后仍可回到原文，按平台记录覆盖。

各任务的最小阅读范围见[文档索引](INDEX.md#context-bundles)。

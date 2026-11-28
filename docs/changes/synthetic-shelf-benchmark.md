# 恢复合成书架基准的 fixture 入口

## Status

implemented

## Problem

`scripts/bench-library-shelf.py` 要求由 Rust example 生成的 256 本合成书架，但当前主线没有该 example，干净 checkout 不能生成 fixture。

## Scope

恢复现有 runner 所需的原创 fixture、后端计时与本地复测入口。保持书架产品和 Linux GUI 的计时定义不变。不迁移旧版书架的 Windows / AVD 断言，不访问真实资料库。

## Architecture Impact

none

## Acceptance Criteria

- seed 只创建新目录，产出 256 本原创书，其中 208 本已准备；验证封面、收藏、分类与打开样本。
- startup / return 只接受完成的 fixture；输出与现有 runner 兼容的后端计时，不包含路径、正文或书名。
- 在本机跑 release 后端基准；不把后端耗时当作 WebView、整机冷启动或平台验收。

## Files And Steps

恢复 `backend/atha-backend/examples/shelf_benchmark.rs`，补充 [代码地图](../codebase/MAP.md)中的调用方式。关联 [Issue #2](https://github.com/weichzh/Atha/issues/2)。

## Checks

定向 release 构建、fixture 形状与隔离反例、现有 Python 后端 runner、文档门。

## Iteration Control

- Mode: `single-pass`

## Approval

按逐批交付剩余验证工具的授权恢复独立 fixture。

## Result

Rust 1.97.1 / Linux：release 构建和 `seed`、`startup`、`return` 均通过。单份 fixture 有 256 本书、208 本已准备、64 本收藏、32 本有自定义封面、92 本有封面、160 本带主题。

三轮 Python 后端 runner 产出九条计时；每轮分别包含冷文件页启动、热启动和热返回。共 1,680 个 fixture 文件页，逐轮确认冷态驻留 0 页、热态驻留 1,680 页。这里的冷态不包含目录 / inode 缓存。

```bash
cargo build --release --locked --offline -p atha-backend --example shelf_benchmark
mkdir -p artifacts/local/shelf-benchmark
target/release/examples/shelf_benchmark seed artifacts/local/shelf-benchmark/run-new
python3 scripts/bench-library-shelf.py backend target/release/examples/shelf_benchmark \
  artifacts/local/shelf-benchmark/run-new --samples 3
```

## Review

实现内复核确认 JSON 字段、目录名和 marker 与既有 runner 一致。重种已存在目录、未 seed、符号链接 marker、缺失 profile、符号链接 profile 均按预期拒绝；缺失 profile 未被创建。独立审查在集成交付时进行。

## Evidence And Residual Risks

未进行改前 / 改后对照或 Linux 原生窗口、Windows 与 Android 验收。

两个 PowerShell 书架增量面向早期书架，依赖 `.shelf-sort`、`.shelf-search-field` 或 `打开《…`，与 #13 的 Bits UI 书架不匹配。旧进度分组也不是 #13 的现有产品契约。增量仅归档；如要恢复 Windows 专项门，需按当前 UI 的实际操作重新定义断言并在 WebView2 验证。旧 AVD 门按项目文档只保留历史证据。

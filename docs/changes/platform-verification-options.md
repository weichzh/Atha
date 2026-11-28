# 承接已启用的 APK 与公式验证契约

## Status

accepted

## Problem

PCT APK 验证未检查打包后启动 Activity 的 `adjustResize`。公式性能入口不能传递 Linux 原生门已支持的两个显式图形选项。

## Scope

补最终 APK 的 launcher 清单静态断言；允许公式 wrapper 透传 `--software-compositing`、`--cpu-painting`。不更改产品清单、包签名与安装流程，不修改公式门的默认值、样本校验、性能阈值或 Linux 图形环境。

## Architecture Impact

none

## Acceptance Criteria

- 匹配 APK badging 识别的唯一启动 Activity；拒绝其缺失或使用不同的软键盘适配模式，即使其他 Activity 使用 `adjustResize`。
- 公式 wrapper 只在显式传入时转发对应图形选项，继续验证 sidecar 与样本的一致性。

## Files And Steps

`scripts/check-pct-reader.sh`、`scripts/check-reader-formula-performance.sh`。关联 [Issue #2](https://github.com/weichzh/Atha/issues/2)。

## Checks

Bash 语法；使用真实 aapt2 生成合成清单 APK 跑 `verify`；用原创样本与参数记录器检查 wrapper 转发和拒绝。

## Iteration Control

- Mode: `single-pass`

## Approval

按逐批交付剩余验证工具的授权接回缺失的平台预检能力。本切片不连接设备。

## Result

Linux：两个 Bash 脚本通过语法检查。合成 APK 的点号相对名、未限定简名与全名带 `adjustResize` 均通过新增断言，进入预期的 arm64 缺库拒绝；缺少模式、显式 `adjustPan` 或重复启动 Activity 均在新增断言拒绝。公式 wrapper 默认参数、双开关透传、未知开关拒绝、样本变化拒绝均实测符合预期。

## Review

实现内复核比对调用和拒绝边界；维持原包、SDK、签名、对齐检查与公式样本校验。独立审查在集成交付时进行。

## Evidence And Residual Risks

未执行真实签名包、真机输入法或 WebKitGTK 公式性能测量。当前 `check-reader-linux.sh` 仍包含 #13 之前的书架选择器和进度分组断言，须按当前界面修复后再作完整 GUI 验收。历史 AVD 脚本不做迁移。

旧 Kindle Bash / Python 包装的公有模式只重复现有合成测试；其性能、真实 GUI 与日志隐私逻辑依赖私密语料和旧书架 / TOC 元素。保留为档案，不随本候选交付。

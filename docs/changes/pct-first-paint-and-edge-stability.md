---
description: 修正 PCT-AL10 首次进入章节时的可见排版收缩与列表序号越界。
---

# PCT 首帧与页边稳定

## Status

implemented

## Problem

此前约 0.4 秒间隔的连续截图漏掉了短暂重排，不能证明问题已经解决。PCT-AL10 冷进程首次进入目标章节的系统录屏实际约每 54 毫秒一帧，确认阅读器会先显示尚未完成公式布局的页面，随后表格与下方正文上移。相邻页的顶层列表序号也会越过正文页左边界。

## Scope

- 在已有会话稳定状态之前隐藏不完整正文，稳定后一次显示；
- 让书内列表缩进随阅读字号增长，避免序号越过页边；
- 用同一冷进程路径的高频录屏和边界坐标在 PCT-AL10 复测。

## Non-goals

- 不等待整本书或非当前章节资源加载；
- 不改变公式大小、表格样式、分页算法或用户阅读设置；
- 不增加新的加载组件或设置项。

## Architecture Impact

阅读会话状态与模块边界不变；应用壳把既有 `content-loaded` 到 `layout-stable` 区间明确作为不可见的中间分页状态。

## Acceptance

- `FIRST-PAINT-01`：`content-loaded` 到 `layout-stable` 之间不显示可见正文；
- `FIRST-PAINT-02`：目标章节第一个可见正文帧已经包含稳定公式，后续帧中表格与下方正文不再上移；
- `PAGE-EDGE-01`：目标列表序号的左边界不小于正文页左边界；
- `REGRESSION-01`：阅读器正式门、PCT 构建及同包安装后启动检查通过。

## Files And Steps

1. 复用现有会话状态和启动遮罩，稳定前不揭示正文。
2. 在共享书籍样式中补足随字号缩放的列表缩进。
3. 扩充现有浏览器自检并在 PCT-AL10 重放原路径。

## Checks

- `node --check reader/web/app.mjs`；
- `node --check reader/web/diagnostics.mjs`；
- `bash scripts/check-reader-linux.sh`；
- `bash scripts/check-pct-reader.sh build`；
- `bash scripts/check-pct-reader.sh install --device 5ENDU19917001679`；
- PCT-AL10 冷进程首次进入目标章节的高频系统录屏；
- PCT-AL10 目标列表页的截图与 UI XML 边界检查；
- 仅对本次中文 Markdown 运行 `autocorrect --fix` 与 `autocorrect --lint`。

## Result

阅读器复用现有会话状态和启动遮罩：目标章节进入 `content-loaded` 后隐藏正文并显示遮罩，直到 `layout-stable` 才一次揭示；不增加加载组件，也不等待章节剩余资源。书内列表的最小逻辑缩进改为 `max(1.55em, 40px)`，在大字号下为序号保留随字号增长的空间。

## Review

按 Spec 与 Standards 分别复核了状态边界、失败恢复、无障碍忙碌状态、书源样式覆盖范围和目标设备证据，未发现阻塞问题。正式 Linux 门与 PCT 冷进程重放覆盖了实现路径。

## Evidence And Residual Risks

- 静态 / 本地：两个修改后的 ES module 通过 `node --check`；四份中文 Markdown 通过 `autocorrect --fix` 与 `autocorrect --lint`；完整 `bash scripts/check-reader-linux.sh` 通过，包含实际 WebKitGTK GUI、自检、响应式工作区、220 次手势测量和 AppLog 隐私检查。
- 真实目标：APK SHA-256 为 `0322a1237408a1ead9659e7e0ef2554414825f2f8a8b720d30e40dd835c0e958`，已同包更新到 PCT-AL10；`artifacts/local/audits/pct-reader-install-20260817T115012Z-2381763/` 确认签名、版本、非降级更新、进程存活和数据保留边界。
- 真实目标：冷进程首次进入原章节的系统录屏为 498 帧 / 25.962 秒，平均约每 52 毫秒一帧；该数值只说明视觉取样密度，不作为性能门槛。第 185 至 212 帧只显示既有遮罩，第 213 帧开始揭示时公式、表格和下方正文已在最终位置，此后未再出现原来的上移；证据位于 `artifacts/local/audits/pct-reader-cold-section-transition-20260817T115251Z/`。
- 真实目标：同一设备目标列表页的正文页左边界为 30 像素，各可见列表序号左边界为 54 像素，不再出现旧版本 18 像素越过页边的情况；证据位于 `artifacts/local/audits/pct-reader-edge-final-20260817T115530Z/`。
- 剩余边界：系统录屏与 ADB 手势是真机视觉证据，不等于自然手指观感；当前手机停留在目标列表页，最终体验由用户直接确认。

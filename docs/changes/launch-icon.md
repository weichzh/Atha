---
description: 将「启门」接入 Atha 的跨平台应用图标与网站图标。
---

# Atha「启门」应用图标

## Status

implemented

## Problem

平台启动图标仍使用旧书签，网站页签与应用内品牌标记也各不相同。用户批准用提供的「启门」统一这些入口，并制作各平台图标。

## Scope

- 保留 SVG 原稿；生成 Windows 多尺寸 ICO、Linux 多尺寸 PNG、macOS ICNS、iOS / iPadOS AppIcon 素材、Android 自适应和主题图标，以及标准网站图标。
- 接入现有 Windows、Linux、Android、Web 工程，更新应用导航内的品牌标记。提供 Apple 素材，不创建新宿主、改包标识、部署或发布。

## Architecture Impact

none

## Acceptance Criteria

- [x] 桌面和移动端入口使用同一图案；16 px 网站图标仍可识别。
- [x] Android 前景独立于底色，使用系统裁形，圆形模式不裁掉主体；常规、圆形和主题图标齐全。
- [x] 保留矢量源与重生成入口，检查图标格式、引用及实际本地打包结果。
- [x] [PR #6](https://github.com/weichzh/Atha/pull/6) 已合入 `86f3ee6`。

## Files And Steps

1. 引入 SVG 源稿，建立 Tauri 图标生成入口。
2. 接入桌面打包、Android 资源和网站、应用内品牌位。
3. 检查实际图标及可构建平台的制品。

## Checks

- `python3 scripts/generate-app-icons.py`：从 SVG 生成全部素材；需要项目已有的 mise / pnpm / Tauri CLI，以及构建主机的 `rsvg-convert`。应用运行与普通打包不需要 `rsvg-convert`。
- Web：Svelte 类型检查、Vite 构建、核对入口声明与应用内品牌 SVG。
- Android：编译 ARM64 APK，核对最终 Manifest、常规和圆形自适应 XML 及 APK 内两种前景的像素。
- Linux：构建、解包 AppImage，核对 `.desktop`、`.DirIcon` 与 32、128、256、512 px 图标。

## Iteration Control

- Mode: `single-pass`

## Result

Windows 的应用与 NSIS 安装器共用多尺寸 ICO。Linux、Android、Web 已接入；macOS 已配置 ICNS，iOS / iPadOS 提供可拷入 Xcode 的 1024 px 不透明 AppIcon 集。16 px favicon 简化书脊镂空；主图保留原始构图与颜色。

Linux AppImage、Android ARM64 APK 与 Web 构建通过。Android 前景和单色层经圆形、圆角方形和 squircle 裁形模拟未裁掉主体。未在 Windows、macOS 或 iOS / iPadOS 上运行原生构建；后者现无对应 Xcode 宿主。未安装或发布应用。

## Review

独立审查未发现引用断链。根据审查补齐 NSIS 安装器的 ICO；生成脚本为可派生部分使用显式 SVG 标识，在临时目录生成完整资源后才复制到目标位置。

交付候选 `a697ae4` 通过代码与安全复审。与开书动效合并前复验 Svelte、Vite、桌面品牌标记和三个 Web 图标引用。

## Evidence And Residual Risks

拆分候选重新生成素材，桌面 PNG / ICO 与 Android 输出同原候选一致，macOS ICNS 各尺寸的解码像素一致；核对 Android 三层资源、桌面安装器引用、Web 构建与图标尺寸。Svelte 和 Vite 通过。原生 AppImage / APK 证据来自原图标工作区。

图标对照图和核验回执保留在本机。版本控制中的素材可直接复用，不需从 APK 或 AppImage 提取。

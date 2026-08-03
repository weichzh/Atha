---
description: 将当前阅读器迁移到 Tauri 2 与 Svelte 5 应用壳，同时保留浏览器阅读内核和性能契约。
---

# Tauri 与 Svelte 阅读应用壳

## Status

implemented

## Problem

当前直接使用 Wry/Tao 与原生 DOM 的阅读器已经完成核心功能，但桌面能力、构建发布和 UI 控件都需要项目自行维护。后续继续复刻微信读书并扩展书架、设置和笔记界面，需要成熟的桌面与前端生态，同时不能牺牲已验证的阅读性能。

## Scope

- 新增标准的 Tauri 2、Vite、Svelte 5 和 TypeScript 应用，继续使用 Windows WebView2；
- 首轮保持单 WebView，复用现有书籍资源边界、CLI EPUB 入口、固定书页和 reader kernel；
- Svelte 只拥有应用壳，按职责拆分顶部栏、底部工具栏和面板，不让书籍 DOM 进入组件状态；
- 保留现有 Wry/Tao host 作为迁移期性能基线，Tauri 通过正式检查后再单独决定删除；
- 将 Node 与 pnpm 的本机工具路径纳入既有 `env/local.ps1` 管理方式；
- 更新正式 gate：继续记录进程树内存，但不再因内存数值拒绝交付；
- 增加 Tauri/Svelte 的构建、Rust 检查、真实 EPUB 启动和性能验证入口。
- 参考 Readest 把页边距从阅读密度中拆出，按固定设备像素分别设置四边，保留既有偏好持久化与 Locator 恢复路径。

## Non-Goals

- 不重写分页、Locator、搜索、标注、书签、偏好和内容安全模块；
- 不新增书架、听书、同步或桌面横屏功能；
- 不引入 SvelteKit、路由库、全局状态库、Tailwind 或完整设计系统；
- 不在迁移同时重新定义微信读书界面的产品结构。

## Acceptance Criteria

- [x] Tauri 应用可通过 `--epub` 打开指定样本，书籍脚本、外部网络、越界路径、新窗口、下载和权限请求继续被拒绝；
- [x] Svelte 壳层保持当前沉浸态、中央唤出、顶部返回/书签/更多和底部五入口，书页几何不因控制层变化；
- [x] reader kernel 继续由现有 JavaScript 模块直接控制 closed Shadow DOM，Svelte 不保存 XHTML 或页内 DOM；
- [x] 应用使用单 WebView，书页保持 780 × 1680 设备像素，系统控件继续遵循系统缩放；
- [x] 前端 production build、Rust fmt/clippy/test、Tauri debug build 和真实 EPUB 启动检查通过；
- [x] 冷启动、首稳、热开、翻页和字号重排继续满足 2000/750/120/50/150ms 的 P95 门槛；
- [x] 内存只记录，不设失败门槛；
- [x] 行距不再隐式改变页边距，四边距可独立调整并在系统缩放下保持固定设备像素；
- [x] 迁移代码与当前 Wry/Tao 基线分开，可通过删除 `reader/app/` 和 workspace 成员回滚。

## Files And Steps

1. 建立 Svelte/Vite 应用壳和最小组件边界，构建时直接复用现有 reader kernel；
2. 建立 Tauri host，迁移 CLI、书籍协议、安全限制、持久 profile 与遥测；
3. 接入本机 Node/pnpm 环境和正式检查入口；
4. 使用指定 EPUB 完成功能、性能与移动竖屏视觉验证；
5. 更新架构、代码地图、路线图和界面代码地图。

## Checks

- `pwsh -NoProfile -File scripts/check-tauri-reader.ps1`；
- `pwsh -NoProfile -File scripts/check-reader-gate.ps1`；
- `python scripts/doc_guard.py`；
- `python scripts/doc_length_check.py`；
- `git diff --check`；
- 390 × 840 CSS 像素、DPR 2 的真实 WebView2 沉浸态、控制层和面板截图。

## Rollback

删除 `reader/app/`，从根 Cargo workspace 移除 Tauri crate，并恢复环境、gate 和事实文档；迁移不改变书籍缓存、Locator 或用户状态 schema。

## Approval

2026-08-03：用户批准开始 Tauri 与前端框架迁移，并明确性能与成熟 UI 生态优先，内存不作为否决条件。

## Result

新增标准 Tauri 2、Vite、Svelte 5 产品入口，并把现有移动阅读壳拆为书页、顶部栏、底部栏、面板和内容 dialog；Vite 直接复用既有十六份 reader module，Tauri 复用旧 host 的 CLI、窗口尺寸和诊断逻辑。书籍资源、Locator、偏好、搜索、标注和分页内核没有重写。

页边距参考 Readest 的独立 insets 模型，从旧“阅读密度”中拆为上下左右四项固定设备像素偏好；默认值只由偏好模型持有，面板由模型同步，行距变化不再改变页面几何。

## Review

- Blocking：审查发现并修复 Tauri 内部 IPC 命名冲突、异步导航验证竞态、字号重排后的可见书签语义、受保护图片导致的大书搜索探针失焦、前端重建触发和浏览器权限拒绝证据缺口；最终 review 未发现剩余阻塞项；
- Non-blocking：旧 Wry/Tao host 继续保留为基线；内存只记录；Tauri 只验证 debug build，尚无安装包或 CI；
- Out-of-scope：书架、听书、同步、桌面横屏、完整 UI 组件库和旧 host 删除。

## Evidence And Residual Risks

- 静态与本地证据：Svelte check、production build、Cargo fmt、clippy、workspace tests 和 Tauri debug build 通过；Rust 单测覆盖页面来源与关键 `Permissions-Policy` 能力；
- 真实目标证据：指定《数学及其历史》EPUB 的 Tauri import probe、四困难样本、173 section 大书搜索、强杀恢复、状态持久化、明暗主题和 WebView2 进程树检查通过；真实文档策略确认相机、显示捕获、定位和麦克风不可用；
- 性能证据：Tauri/Svelte 运行 `1785763863358-21204` 的冷启动、首稳、热开、翻页、重排 P95 分别为 605.253、138.700、21.400、6.900、41.700ms，均低于固定门槛；旧 host 完整回归运行 `1785763100052-13648` 同样通过；
- 视觉证据：780 × 1680 设备坐标下核对沉浸态和控制层不压缩书页，默认四边距为上/右/下/左 88/32/88/32，非对称设置与恢复也通过自动断言；
- 证据等级最高为真实 Windows WebView2 本机链路；未执行安装包、CI、生产环境或跨设备性能比较。残余风险是 Tauri/Wry 双 host 在迁移期需要同步安全与内核入口变更，删除旧 host 时应另开 change。

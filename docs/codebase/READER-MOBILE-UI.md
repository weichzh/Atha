---
description: 现代极简书房无框线前端的代码位置、结构、尺寸和手工调整入口。
---

# 阅读界面代码地图

本文只说明当前产品前端界面代码放在哪里、结构怎样工作，以及手工调整时应改哪些组件与样式变量。阅读数据、EPUB、Locator、搜索、标注和持久化语义仍以 `architecture/READER-CORE.md` 为准；无框架静态兜底模板 `reader/atha-reader.html` 仍沿用 `reader/web/app.mjs` 独立组合方式。

## 文件位置

| 文件                                                                                                        | 负责内容                                                                                                |
| ----------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------- |
| `reader/app/src/main.ts`、`App.svelte`、`lib/data/back-stack.ts`                                            | 阅读入口、首屏外观预置、Android/Web 同文档开书、键盘与原生返回分发                                      |
| `reader/app/src/lib/components/BookOpeningCover.svelte`、`lib/data/open-transition.ts`                      | 取可见封面、沿书脊翻开、等比放大真实正文；不改变排版与书内位置                                          |
| `reader/app/src/lib/components/NavRail.svelte`                                                              | 悬浮玻璃左侧导航轨（>= 900 px）与底部胶囊导航栏（< 900 px）                                             |
| `reader/app/src/lib/components/LibraryView.svelte`、`BookCover.svelte`                                      | 无框线极简书架、继续阅读条带、快捷分类胶囊、网格/列表视图、崩溃恢复、拖放导入与桌面悬浮单书菜单         |
| `reader/app/src/lib/components/MemoryView.svelte`、`SnapshotPreview.svelte`                                 | 跨书阅读记忆中心：全文检索、最近阅读投影、安全跳回原书与历史快照预览                                    |
| `reader/app/src/lib/components/AppSettingsView.svelte`、`ConfirmDialog.svelte`                              | 应用外观、全书阅读默认重置、离线词典管理与完整 `.atha-data` 备份/恢复/存储分类统计                      |
| `reader/app/src/lib/components/ReaderView.svelte`                                                           | 极简沉浸分页阅读器、悬浮上下玻璃工具岛、`directory` 目录/书签/搜索工作区与快捷键响应                    |
| `reader/app/src/lib/components/NotesWorkspace.svelte`、`MessageComposer.svelte`、`MessageActionMenu.svelte` | 竖屏两页/横屏 `A+B` / `A+B+C` 笔记对话工作区、按需 Tiptap/Markdown 输入器与 `@floating-ui/dom` 消息菜单 |
| `reader/app/src/lib/components/ReaderSettingsDialog.svelte`、`CssModulesPanel.svelte`                       | 全书默认、本书覆盖与 32 模块 CSS 管理/CodeMirror 实时编辑弹窗                                           |
| `reader/app/src/lib/components/SelectionPill.svelte`、`DictionaryCard.svelte`、`StructuredLightbox.svelte`  | 正文选区悬浮动作胶囊、离线查词浮卡与图片/公式/表格/代码安全灯箱查看器                                   |
| `reader/app/src/lib/engine/reader-engine.svelte.ts`、`preferences.ts`、`kernel-globals.ts`                  | `ReaderEngine` 状态机与 `reader/web/*.mjs` 内核最小 DOM 桥接、阅读偏好同步与遥测上报                    |
| `reader/app/src/lib/data/{runtime.ts,library-adapter.ts,message-adapter.ts,demo-seed.ts}`                   | Tauri 原生 IPC 与浏览器预览演示数据的双模式适配器                                                       |
| `reader/app/src/styles/{tokens.css,app.css,book-theme.css,book-reader.css}`                                 | 暖纸墨色调语义 token、无框线应用壳样式与书内安全排版样式                                                |
| `reader/web/*.mjs`、`reader/web/style-module-package.mjs`                                                   | 书页内容校验、Locator、分页、交互、会话、导航、书签、搜索、统计、消息适配与样式包 codec                 |
| `reader/app/src-tauri/src/{lib.rs,desktop_open.rs,library_commands.rs,telemetry.rs,protocol.rs}`            | Tauri 组合根、单实例文件关联热打开、书架 IPC、遥测白名单与受控书本/封面协议                             |

## DOM 结构

当前产品壳在 `main.ts` 挂载 `App.svelte`：

```text
tauri://localhost/（或未带阅读参数的浏览器预览）
├─ NavRail（桌面左侧悬浮玻璃轨 / 窄屏底部胶囊）
└─ LibraryView（main.library-shell[aria-busy]）| MemoryView | AppSettingsView
   └─ .shelf-catalog [data-shelf-open]（C4 同文档返回焦点与位置锚点）

tauri://localhost/index.html?manifest=...&state=...（或打开书籍后切换为 reader 视图）
└─ ReaderView（main.reader-shell[aria-busy]）
   ├─ .reader-workspace-layout
   │  ├─ section.region-a（正文画布）
   │  │  ├─ .reader-frame > .reader > .page > .reader-content-host
   │  │  ├─ .reader-controls（#reader-back、.top-island、.bottom-island）
   │  │  └─ .kernel-bridge（隐藏最小契约节点供 reader/web/*.mjs 读取）
   │  ├─ NotesWorkspace（800–1439 px 为 A+B/B+C，>= 1440 px 支持 A+B+C 与专注对话）
   │  └─ SelectionPill / DictionaryCard / ReaderSettingsDialog / StructuredLightbox / #error-back
```

`.reader-controls` 仅位于 `.region-a` 内（`position: absolute; inset: 0`）并与正文同层受工作区分栏约束，不参与 `.reader` 内部设备像素分页。所有书籍 DOM 解析、安全剥离、CSS Shadow/Scoped 注入和翻页动画仍由 `reader/web/*.mjs` 执行；`ReaderEngine` 通过状态订阅将章节标题、页码百分比、书签、选区和高亮同步给 Svelte 视图。

`main.ts` 预置主题与阅读亮度。Android 和浏览器预览从书架同文档进入阅读：首屏恢复后才翻开封面，页与字在原有布局外统一缩放。无可用封面或减少动态时，保持来路至首屏就绪。返回在正文还挂载时保存进度，再恢复书架滚动和触发焦点。桌面原生打开仍跨文档，具有精确书架来源凭据时走 `history.back()`，否则回到应用根。原生文件打开也沿用这一返回契约。

## 尺寸与缩放

- `.reader` 与内部 `--page-scale` 逻辑保持不变：`pagination.mjs` 将视口 CSS px 乘 `devicePixelRatio` 写入内部画布并按 `1 / devicePixelRatio` 反向缩放；
- 字号 16–40 视觉 CSS px，左右边距在 24 / 32 / 48 视觉 CSS px 间选择并限制单栏正文约 760 CSS px；上下安全区保持 144 设备像素；
- 书架采用响应式网格/清单与 `contain` 优化；封面缺失时以稳定哈希色调 + 首字印章生成封面。

## 手工调整入口

- **颜色与动效 Token**：`reader/app/src/styles/tokens.css` 与 `book-theme.css`；`library.test.ts` 检查 `app.css` 等功能样式保持零硬编码颜色字面量；
- **应用壳与阅读外围 UI**：`reader/app/src/styles/app.css` 及对应组件 scoped `<style>`；
- **书内正文排版**：`reader/app/src/styles/book-reader.css`（及静态模板回退 `reader/atha-reader.css`）；
- **交互原语**：统一使用 `bits-ui@2.22.0`（Dialog / AlertDialog / Popover / Select / Slider / Switch）与 `@floating-ui/dom@1.8.0`、按需加载 Tiptap 与 CodeMirror 6，不引入 Tailwind CSS。

## 交互连接与当前状态

- 书架冷启动：初始挂载执行崩溃恢复、`resumeBookDataDeletions()`、`listLibraryBooks()`，并消费一次性 `takeStartupImport()`；非持久化 `pageshow` 不再取消初始加载，BFCache 持久化 `pageshow` 仅执行轻量刷新与 `restoreSessionAndFocus()`；
- 阅读内核：保留 `PageDown`/方向键/触摸/滚轮/选区查词/标注高亮/消息修订/历史快照跳回；`ReaderEngine` 在打开完成后先基于真实 64 位 `contentVersion` 初始化阅读统计并在标记 `data-reader-ready` 前 `flushProgress()`，再向 Tauri 发送 `ready|<pages>|0|0|0` 或白名单 `error|<code>|<stage>`；
- `reader/atha-reader.html` 无框架兜底模板及 `reader/web/app.mjs` 仍完整保留供独立打开校验，不依赖 Svelte 壳。

## 验证与截图证据

- **2026-11-26 新版无框线前端正式切换（`docs/changes/borderless-reader-frontend.md`）**：
  - 本地快速门：`bash scripts/check-fast.sh` 通过（145/145 Node 测试、`svelte-check` 0 errors/0 warnings、Cargo workspace 测试、`vite build`、CSP 无内联脚本检查）；
  - Chromium Playwright：`node reader/app/tests/reader-app.browser.mjs` 通过，11 张全端截图存 `artifacts/local/audits/borderless-reader-20261126/`；
  - 隔离 Xvfb Linux Tauri release 真壳：256 本合成书架 `bench-library-shelf.py native --samples 2` 四轮均 `books: 256`、`history_same_document: true`（`startup_usable_observed_ms` 684.35–713.34 ms、`return_usable_observed_ms` 170–185 ms），`check-tauri-hot-open.py` 全项通过（`tauri_hot_open=passed`），收据存 `artifacts/local/audits/borderless-reader-20261126/{native-replay.log,hot-open/}`。
  - 待补：前景 GNOME Wayland 会话下的 `scripts/check-reader-linux.sh` 需更新旧选择器以对齐新 DOM；Windows WebView2、Android PCT-AL10 真机、自然手指触摸与真实输入法尚待实机验收。
- 旧版各轮专项截图与真机历史收据保留于 `artifacts/local/audits/` 与 Git 历史。

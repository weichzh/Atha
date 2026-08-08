---
description: 移动竖屏阅读界面的代码位置、结构、尺寸和手工调整入口。
---

# 移动阅读界面代码地图

本文只说明界面代码放在哪里、当前结构怎样工作，以及手工调整时应改哪些选择器。阅读数据、EPUB、Locator、搜索、标注和持久化语义仍以 `architecture/READER-CORE.md` 为准。

## 文件位置

| 文件 | 负责内容 |
| --- | --- |
| `reader/app/src/App.svelte` | 产品阅读页根结构；组合书页、控制层和内容 dialog |
| `reader/app/src/components/ReaderCanvas.svelte` | 自适应书页、章节和进度 DOM |
| `reader/app/src/components/ReaderChrome.svelte` | 顶部栏、底部栏、选区动作条、兼容笔记 dialog 与对话浮层的组合 |
| `reader/app/src/components/ConversationOverlay.svelte` | Atha 默认对话界面、修订/关系 dialog 与历史快照 dialog 的 DOM |
| `reader/app/src/components/MessageComposer.svelte` | 自增高/全屏消息输入、两层工具栏和可视/Markdown 输入模式 |
| `reader/app/src/message-editor.ts`、`message-markdown.ts` | 受限 Tiptap 扩展、链接规则及按需加载的 Markdown 双向转换 |
| `reader/app/src/components/chrome/` | 顶部返回/书签/更多和底部五图标 |
| `reader/app/src/components/panels/` | 目录、搜索、笔记、进度和偏好面板 |
| `reader/app/src/components/CssEditor.svelte` | CSS 模块页可见时按需加载的 CodeMirror 6 渐进增强；隐藏 textarea 保持唯一状态入口 |
| `reader/app/src/shell.css` | 顶部和底部覆盖层、面板、图标及壳层明暗视觉 |
| `reader/atha-reader.css` | 自适应书页、固定内部边距、系统缩放和书籍内容样式 |
| `reader/web/app.mjs` | 组合模块；开关工具层；目录投影；设置下钻；返回按钮 |
| `reader/web/interaction.mjs` | 正文左、中、右点击区和键盘、滚轮、触摸输入；中间点击开关工具层 |
| `reader/web/bookmarks.mjs` | 右上角书签切换、目录中的书签列表和书签跳转 |
| `reader/web/message-store.mjs` | 正式根 Message 到标注/笔记投影的适配，以及旧 localStorage 记录迁移 |
| `reader/web/conversations.mjs` | 对话浮层、本条/本章/本书记录、时间/书序投影、回复、引用、编辑、删除、修订、关系、历史快照、跳回和本书消息导出 |
| `reader/web/navigation.mjs` | 章节标题、目录选择、全书近似进度和进度拖动 |
| `reader/web/pagination.mjs` | 视口设备像素换算、分页、尺寸变化、进度和公式尺寸；几何 cut 只供诊断与 verify-sample / benchmark 门使用，不作为普通阅读的全局失败条件 |
| `reader/assets/bookmark-24-regular.svg` | 右上角书签图标，来自 Microsoft Fluent System Icons；固定来源与 MIT 文本见根 `THIRD_PARTY_NOTICES.md` |
| `reader/app/src-tauri/src/lib.rs` | Tauri 窗口、受控书籍协议、导航限制和遥测 command |
| `reader/atha-reader-host/src/windows/launch.rs` | 两个 host 共用的 Windows 窗口尺寸、CLI 与阅读 URL |
| `reader/app/vite.config.ts` | production 壳构建和既有 reader module 拼接顺序 |
| `scripts/check-reader-samples.ps1` | 四本书、明暗主题、真实 WebView2 与阅读状态回归 |

## DOM 结构

Svelte 组件渲染后保持既有 DOM id 与 class，主要层次如下：

```text
.reader-shell
├─ .reader-frame
│  └─ .reader
│     ├─ #page / #book-host
│     ├─ #chapter-label
│     └─ #position
├─ .reader-controls
│  ├─ .top-toolbar
│  │  ├─ #reader-back
│  │  └─ .top-toolbar-actions
│  │     ├─ #add-bookmark
│  │     └─ .preferences
│  └─ .toolbar
│     ├─ .directory
│     ├─ .search
│     ├─ .notes
│     ├─ .progress
│     └─ .listen-placeholder
├─ #selection-actions
├─ #annotation-note-dialog
├─ #message-conversation
├─ #message-history-dialog
└─ #message-snapshot-dialog
```

顶部和底部工具不在 `.reader` 内。根元素出现 `data-reader-tools` 时，`.reader-controls` 才可见；工具层覆盖书页，不改变 `.reader`、`#page`、章节标题或进度的几何尺寸。四个面板使用同名原生 `<details name="reader-panel">`，因此只能打开一个。目录和笔记使用相同的全屏几何与返回入口。目录保留隐藏的 `#toc` 作为 Navigation 与书签的单一数据源，`app.mjs` 只把其中的 option 投影为 `#directory-list` 按钮；没有第二份目录状态。

## 尺寸与缩放

- `.reader` 填满当前 WebView；`pagination.mjs` 把内部宽高设置为视口 CSS 像素乘 `devicePixelRatio`，再用 `1 / devicePixelRatio` 设置 `--page-scale`。
- 普通图片在单页可用宽高内等比缩放；表格与代码由 reader 注入的 `.atha-structured-overflow` 容器限制在单页并允许双向滚动，避免书源样式把内容静默裁掉。
- 例如 4K 屏幕采用 200% 系统缩放且视口为 390 × 840 CSS 像素时，内部书页为 780 × 1680 设备像素；同一窗口放大后会按新的设备像素宽高重新分页。
- `.top-toolbar` 与 `.toolbar` 固定为 48 CSS px；`.tool-panel` 和普通表单控件同样不跟随 `--page-scale`，而是遵循系统 CSS 像素和系统缩放。
- 上下正文安全区最小为 144 设备像素；在 DPR 2 下正文为 y=72–768 CSS px，工具栏为 y=0–48 与 y=792–840，因此控制层只进入页眉页脚。
- 尺寸变化等待 120ms 后进入 Navigation 串行队列，使用变化前 Locator 重排和恢复；正式回归覆盖 `390 840 2`、`780 1680 1` 与 `960 720 1`。

## 手工调整入口

应用壳视觉在 `reader/app/src/shell.css` 中调整；书页内容只在 `reader/atha-reader.css` 中调整：

| 想调整的部分 | 选择器或变量 |
| --- | --- |
| 书页边距 | `reader/atha-reader.css` 的 `--page-top-margin`、`--page-right-margin`、`--page-bottom-margin`、`--page-left-margin`；上下固定，左右由本书 `#page-margin` 在 24 / 32 / 48 设备像素间选择 |
| 顶部栏 | `.top-toolbar`、`.top-toolbar-actions` |
| 底部栏 | `.toolbar`、`.icon-button`、`.fluent-icon` |
| 所有弹出面板 | `.tool-panel` |
| 目录和书签 | `.directory-panel`、`.directory-list`、`.directory-item`；隐藏数据源为 `#toc` 与 `option[data-bookmark-id]` |
| 搜索 | `.search-panel`、`.search-actions` |
| 选区动作与笔记 | `.selection-actions`、`#annotation-note-dialog`、`.notes-panel`、`.annotation-filters`、`.annotation-list`、`.annotation-item` |
| 阅读对话 | `.message-conversation`、`.message-view-controls`、`.message-segmented`、`.message-source-context`、`.message-feed-source`、`.message-card`、`.message-reference-preview`、`.message-composer`、`.message-detail-dialog` |
| 消息输入 | `.message-editor`、`.message-editor-toolbar-primary`、`.message-editor-toolbar-secondary`、`.message-editor-mode-switch`、`.message-editor-markdown` |
| 对话主题 | `.message-conversation[data-message-theme="atha"]` 内的 `--message-*` 语义令牌；当前只存在 Atha 默认主题 |
| 进度 | `.progress-panel`、`.progress-scrubber`、`.progress-book`、`.progress-position` |
| 更多菜单 | `.preferences-panel`、`.settings-list`、`.settings-view`、`.module-settings`、`.css-editor-*` |
| 主题 | `reader/atha-reader.css` 顶部语义令牌及 `data-theme="light|paper|dark"` 覆盖 |

图标按钮的可点击尺寸由 `.icon-button` 控制。底部顺序由 `BottomToolbar.svelte` 决定；`.toolbar` 固定为五等分。不要把工具栏移进 `.reader`，否则系统缩放会改变控件尺寸，或工具层会参与书页布局。

## 交互连接

- `app.mjs` 的 `toggleReaderTools()` 先撤销待处理的选区动作，再切换 `data-reader-tools`；隐藏时同时关闭已打开面板。全屏目录和笔记页的返回按钮使用同一关闭入口；根级 `contextmenu` 监听统一禁止 WebView 默认右键菜单。
- `interaction.mjs` 只按横向比例区分左 35%、中间 30% 和右 35%；中间区调用 `toggleReaderTools()`。
- `#add-bookmark` 是唯一书签切换入口。`bookmarks.mjs` 在当前位置添加或取消书签，并把已有书签作为 `#toc` 中对应章节后的 `option[data-bookmark-id]`；投影目录中的章节或书签完成跳转后自动关闭目录。
- `#brightness` 在拖动时预览根元素的 `--reader-brightness`，松开后写入应用偏好；亮度滤镜只作用于 `.reader`，不改变系统控件亮度。
- `#density` 调整行距；`#page-margin` 按书选择 24 / 32 / 48 设备像素左右边距，`#paragraph-indent` 与 `#paragraph-spacing` 生成受控可视 CSS。上下 144 设备像素安全区固定不变，旧应用记录中的四个自由边距字段仍会被忽略。
- CSS 模块页直接复用每书偏好：最多 32 个模块，支持搜索、分组、排序、批量启停和 schema 1 JSON 导入导出；新模块单个 32 KiB、启用组合 64 KiB，超限旧 CSS 只作为停用恢复副本保留。CodeMirror 在页面首次可见时按需加载，100 ms 显示 lint，180 ms 后通过同一 textarea 触发预览；输入草稿绑定原模块，任一验证、重排或持久化失败均恢复上次有效状态、渲染与 Locator。
- `#progress-range` 使用 0–1 连续值映射全书 section 和本节页，避免整数刻度在多章节书籍中丢失当前页，也不预布局其他 section；章节、百分比和本节页数都由 Navigation 的既有稳定状态更新。
- 原生正文选区在 `pointerup` 或键盘选择完成后的下一帧投影 `#selection-actions`；复制只触发浏览器 copy，标注和笔记在 Tauri 产品中写入同一根 Message。点击 CSS Highlight 覆盖的已有标注会恢复其选区；“重选”后再次拖选并保存会追加 SourceAnchor/SourceSnapshot，笔记动作追加修订，删除写入墓碑。全屏 `#annotations` 支持章节和全文筛选；点击项目打开对话浮层，独立编辑和删除按钮不触发跳转。
- `#message-conversation` 默认从底部占约半屏，拖动顶部把手可连续调高，轻点把手或标题栏全屏按钮可进入全屏；标题栏不提供收起、导出或共享。顶部可切换本条、本章和本书：本条显示当前 Conversation 的原文短预览、回复与更多；本章和本书是只读聚合记录，可按创建时间或根 Message Locator 的书内位置排列，点击“打开”后进入对应本条对话再写入。被回复消息和额外引用都以大引号摘要显示在回复正文上方，正文下方只常驻时间、回复和更多。每个摘要只读取直接目标自身的正文，不递归展开或复制目标已有的引用。编辑、删除、修订、关系、历史快照与跳回等低频动作进入更多菜单；引用摘要可跳到当前对话目标并短暂高亮。笔记页仍可导出本书消息。
- `.message-editor` 随内容增高，到达紧凑上限后出现全屏按钮。全屏编辑顶部使用两层工具栏：第一层切换可视/Markdown 输入并保留撤销、重做与返回紧凑输入，第二层显示标题、粗体、斜体、列表、引用和安全链接。Markdown 转换按需加载，切回可视模式或发送前必须通过同一正文 schema；不支持的格式保留原文并显示错误，不静默丢失。
- `#tap-to-paginate` 和 `#swipe-to-paginate` 只控制对应指针输入；键盘和滚轮继续保持原行为。
- `#reader-back` 优先使用浏览器历史；没有历史时请求关闭当前阅读窗口。

## 当前有意暂缓

- 听书只有禁用图标，没有播放逻辑；
- 标注颜色、样式、notebook 与同步没有阅读界面入口；
- 桌面横屏和大屏布局尚未设计；
- 当前使用 Lucide Svelte 图标、原生表单控件和按需加载的 Tiptap 消息编辑器；尚未引入通用 UI 组件库或动效框架。
- 当前只实现 Atha 默认聊天主题；微信、Telegram、QQ 风格模拟、主题选择和用户自定义界面等待消息主循环验收后单独设计。
- 对话字号和密度已使用较紧凑默认值；类似 Telegram 的界面字号、密度与主题设置等待当前布局稳定后再增加，不预留持久化字段。

## 截图证据

本机最新截图位于 `artifacts/local/screenshots/`：

- `reader-shell-01-reading.png`；
- `reader-shell-02-tools.png`；
- `reader-shell-03-directory.png`；
- `reader-shell-04-progress.png`；
- `reader-shell-05-settings-menu.png`。
- `message-scope-mark-430x820.png`；
- `message-scope-chapter-430x820.png`。

微信读书源图与实现的同图对照位于 `artifacts/local/audits/reader-shell-usability/`。根目录 `design-qa.md` 记录尺寸归一、交互证据和修复历史；最终没有 P0、P1 或 P2 问题。

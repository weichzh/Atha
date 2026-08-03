---
description: 移动竖屏阅读界面的代码位置、结构、尺寸和手工调整入口。
---

# 移动阅读界面代码地图

本文只说明界面代码放在哪里、当前结构怎样工作，以及手工调整时应改哪些选择器。阅读数据、EPUB、Locator、搜索、标注和持久化语义仍以 `architecture/READER-CORE.md` 为准。

## 文件位置

| 文件 | 负责内容 |
| --- | --- |
| `reader/app/src/App.svelte` | 产品阅读页根结构；组合书页、控制层和内容 dialog |
| `reader/app/src/components/ReaderCanvas.svelte` | 固定书页、章节和进度 DOM |
| `reader/app/src/components/ReaderChrome.svelte` | 顶部栏、底部栏与五个工具入口的组合 |
| `reader/app/src/components/chrome/` | 顶部返回/书签/更多和底部五图标 |
| `reader/app/src/components/panels/` | 目录、搜索、笔记、进度和偏好面板 |
| `reader/app/src/shell.css` | 顶部和底部覆盖层、面板、图标及壳层明暗视觉 |
| `reader/atha-reader.css` | 固定书页、系统缩放和书籍内容样式 |
| `reader/web/app.mjs` | 组合模块；开关工具层；亮度；返回按钮 |
| `reader/web/interaction.mjs` | 正文左、中、右点击区和键盘、滚轮、触摸输入；中间点击开关工具层 |
| `reader/web/bookmarks.mjs` | 右上角书签切换、目录中的书签列表和书签跳转 |
| `reader/web/navigation.mjs` | 章节标题、目录选择和进度拖动 |
| `reader/web/pagination.mjs` | 780 × 1680 设备像素书页、分页、进度和公式尺寸 |
| `reader/assets/bookmark-24-regular.svg` | 右上角书签图标，来自 Microsoft Fluent System Icons |
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
└─ .reader-controls
   ├─ .top-toolbar
   │  ├─ #reader-back
   │  └─ .top-toolbar-actions
   │     ├─ #add-bookmark
   │     └─ .preferences
   └─ .toolbar
      ├─ .directory
      ├─ .search
      ├─ .notes
      ├─ .progress
      └─ .listen-placeholder
```

顶部和底部工具不在 `.reader` 内。根元素出现 `data-reader-tools` 时，`.reader-controls` 才可见；工具层覆盖书页，不改变 `.reader`、`#page`、章节标题或进度的几何尺寸。四个面板使用同名原生 `<details name="reader-panel">`，因此只能打开一个。

## 尺寸与缩放

- `.reader` 永远是 780 × 1680 设备像素；`pagination.mjs` 用 `1 / devicePixelRatio` 设置 `--page-scale`。
- 4K 屏幕采用 200% 系统缩放时，书页显示为 390 × 840 CSS 像素，但内容仍对应 780 × 1680 设备像素。
- `.top-toolbar`、`.toolbar`、`.tool-panel` 和普通表单控件不跟随 `--page-scale`，而是遵循系统 CSS 像素和系统缩放。
- 验证 200% 缩放时，浏览器视口应设为 `390 840 2`，设置后重新加载页面，避免页面沿用旧的 `devicePixelRatio`。

## 手工调整入口

应用壳视觉在 `reader/app/src/shell.css` 中调整；书页内容只在 `reader/atha-reader.css` 中调整：

| 想调整的部分 | 选择器或变量 |
| --- | --- |
| 书页边距 | `reader/atha-reader.css` 的 `--page-top-margin`、`--page-right-margin`、`--page-bottom-margin`、`--page-left-margin`；默认值和校验在 `reader/web/preferences.mjs` |
| 顶部栏 | `.top-toolbar`、`.top-toolbar-actions` |
| 底部栏 | `.toolbar`、`.icon-button`、`.fluent-icon` |
| 所有弹出面板 | `.tool-panel` |
| 目录和书签 | `.directory-panel`、`.toc-control`、`#toc`、`option[data-bookmark-id]` |
| 搜索 | `.search-panel`、`.search-actions` |
| 笔记 | `.annotation-editor`、`.annotation-actions` |
| 进度 | `.progress-panel`、`.progress-actions` |
| 更多菜单 | `.preferences-panel`、`.advanced-preferences` |
| 明暗主题 | `:root[data-theme="light"]` 与 `:root[data-theme="dark"]` 下的规则 |

图标按钮的可点击尺寸由 `.icon-button` 控制。底部顺序由 `BottomToolbar.svelte` 决定；`.toolbar` 固定为五等分。不要把工具栏移进 `.reader`，否则系统缩放会改变控件尺寸，或工具层会参与书页布局。

## 交互连接

- `app.mjs` 的 `toggleReaderTools()` 只切换 `data-reader-tools`；隐藏时同时关闭已打开面板。
- `interaction.mjs` 只按横向比例区分左 35%、中间 30% 和右 35%；中间区调用 `toggleReaderTools()`。
- `#add-bookmark` 是唯一书签切换入口。`bookmarks.mjs` 在当前位置添加或取消书签，并把已有书签作为 `#toc` 中对应章节后的 `option[data-bookmark-id]`；选择书签项直接跳转。
- `#brightness` 在拖动时预览根元素的 `--reader-brightness`，松开后写入应用偏好；亮度滤镜只作用于 `.reader`，不改变系统控件亮度。
- `#density` 只调整行距；`#margin-top`、`#margin-right`、`#margin-bottom` 和 `#margin-left` 分别调整固定设备像素页边距，四项都经 Navigation 串行重排并恢复 Locator。
- `#reader-back` 优先使用浏览器历史；没有历史时请求关闭当前阅读窗口。

## 当前有意暂缓

- 听书只有禁用图标，没有播放逻辑；
- 桌面横屏和大屏布局尚未设计；
- 当前使用 Lucide Svelte 图标和原生表单控件；尚未引入额外 UI 组件库或动效框架。

## 截图证据

本机截图位于 `artifacts/local/screenshots/`：

- `math-history-mobile-immersive.png`；
- `math-history-mobile-controls.png`；
- `math-history-mobile-directory.png`；
- `math-history-mobile-search.png`；
- `math-history-mobile-settings.png`。

同视口对照图为 `design-qa-controls-comparison.png` 和 `design-qa-directory-comparison.png`。本轮检查没有 P0、P1 或 P2 问题；暂缓项只有纸张纹理和原生表单的视觉精修。目录对照确认书签位于对应章节之后，而不是目录底部的独立区域。

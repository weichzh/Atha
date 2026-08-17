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
| `reader/app/src/components/ContentDialog.svelte` | 图片、公式和表格的全屏查看层，以及双指 / 滚轮、按钮、键盘缩放和复位控制 |
| `reader/app/src/components/ConversationOverlay.svelte` | Atha 默认对话界面、修订/关系 dialog 与历史快照 dialog 的 DOM |
| `reader/app/src/components/MessageComposer.svelte` | 自增高/全屏消息输入、两层工具栏和可视/Markdown 输入模式 |
| `reader/app/src/message-editor.ts`、`message-markdown.ts` | 受限 Tiptap 扩展、链接规则及按需加载的 Markdown 双向转换 |
| `reader/app/src/components/chrome/` | 顶部返回 / 书签和底部目录 / 搜索 / 笔记 / 进度 / 设置五图标 |
| `reader/app/src/components/panels/` | 目录、搜索、笔记、进度和偏好面板 |
| `reader/app/src/components/panels/PreferencesPanel.svelte` | Readest 风格分层设置、字号滑块、排版卡片、两种阅读方式与 CSS 模块入口 |
| `reader/app/src/components/CssEditor.svelte` | CSS 模块页可见时按需加载的 CodeMirror 6 渐进增强；隐藏 textarea 保持唯一状态入口 |
| `reader/app/src/components/panels/DictionaryPanel.svelte`、`reader/app/src/dictionary.ts` | 桌面词典浮层、移动端 75% 高底部抽屉、来源 / 字号设置、选区精确查词与安全富文本词条 |
| `reader/web/style-module-package.mjs` | schema 1 CSS 模块包的无网络 codec；未来数据源只能经此复用字段、大小、重复 ID 与 CSS 安全校验 |
| `reader/theme.css`、`reader/app/src/shell.css` | 应用 / 阅读共享颜色 token，以及顶部和底部覆盖层、面板与图标布局 |
| `reader/atha-reader.css` | 自适应书页、固定内部边距、系统缩放和书籍内容样式 |
| `reader/web/app.mjs` | 组合模块；开关工具层；目录投影；设置下钻；返回按钮 |
| `reader/web/interaction.mjs` | 左右模式的键盘、滚轮、页区和横向触摸；上下模式的原生滚动、边界跨章和中间点击工具层 |
| `reader/web/content-actions.mjs`、`reader/web/structured-actions.mjs` | 图片 / 公式与安全表格投影的激活、查看、焦点返回和诊断检查 |
| `reader/web/bookmarks.mjs` | 右上角书签切换、目录中的书签列表和书签跳转 |
| `reader/web/message-store.mjs` | 正式根 Message 到标注/笔记投影的适配，以及旧 localStorage 记录迁移 |
| `reader/web/conversations.mjs` | 对话浮层、本条/本章/本书记录、时间/书序投影、回复、引用、编辑、删除、修订、关系、历史快照、跳回和本书消息导出 |
| `reader/web/navigation.mjs` | 章节标题、目录选择、全书近似进度和进度拖动 |
| `reader/web/reader-state.mjs` | 应用 / 本书偏好、书签、进度与有界本地阅读统计；统计只消费页面生命周期和既有导航稳定状态 |
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
├─ .reader-startup
├─ .reader-frame
│  └─ .reader
│     ├─ #page / #book-host
│     ├─ #chapter-label
│     └─ #position
├─ .reader-controls
│  ├─ .top-toolbar
│  │  ├─ #reader-back
│  │  └─ .top-toolbar-actions
│  │     ├─ .dictionary
│  │     ├─ #add-bookmark
│  │     └─ .preferences
│  └─ .toolbar
│     ├─ .directory
│     ├─ .search
│     ├─ .notes
│     ├─ .progress
├─ #selection-actions
├─ #annotation-note-dialog
├─ #message-conversation
├─ #message-history-dialog
└─ #message-snapshot-dialog
```

顶部和底部工具不在 `.reader` 内。根元素出现 `data-reader-tools` 时，`.reader-controls` 才可见；工具层覆盖书页，不改变 `.reader`、`#page`、章节标题或进度的几何尺寸。四个面板使用同名原生 `<details name="reader-panel">`，因此只能打开一个。目录和笔记使用相同的全屏几何与返回入口。目录保留隐藏的 `#toc` 作为 Navigation 与书签的单一数据源，`app.mjs` 只把其中的 option 投影为 `#directory-list` 按钮；没有第二份目录状态。

`.reader-startup` 从阅读路由首次挂载起用不透明书页底色覆盖书内内容；`app.mjs` 恢复上次 Locator 并绑定交互后设置根级 `data-reader-ready`，加载层才淡出。失败路径同样撤下加载层并显示错误，三点动画与淡出在 `prefers-reduced-motion` 下停用。

## 尺寸与缩放

- `.reader` 填满当前 WebView；`pagination.mjs` 把内部宽高设置为视口 CSS 像素乘 `devicePixelRatio`，再用 `1 / devicePixelRatio` 设置 `--page-scale`。
- 字号滑块保存 16–40 逻辑 CSS px，默认 19；正文实际字号为 `逻辑字号 × devicePixelRatio` 个内部设备像素。PCT-AL10 的 DPR 3 因此对应 48–120 设备像素，默认 57。
- 普通图片在单页可用宽高内等比缩放；v5 本地图片以原生 HTML 宽高和书源 CSS 之前的有界 `contain-intrinsic-size` 规则稳定解码前盒，书源与用户 CSS 继续覆盖，不等待资源完成后再揭示正文。内嵌表格忽略书源最小列宽，以固定布局、紧凑字号和省略号生成整页宽预览；超出单页高度的部分直接裁掉。代码块仍在 `.atha-structured-overflow` 内原生滚动。
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
| 选区动作与笔记 | `.selection-actions` 四图标动作栏、`#annotation-note-dialog`、`.notes-panel`、`.annotation-filters`、`.annotation-list`、`.annotation-item` |
| 词典 | `.dictionary-backdrop`、`.dictionary-panel`、`.dictionary-content`、`.dictionary-source`、`.dictionary-result` |
| 阅读对话 | `.message-conversation`、`.message-view-controls`、`.message-segmented`、`.message-source-context`、`.message-feed-source`、`.message-card`、`.message-reference-preview`、`.message-composer`、`.message-detail-dialog` |
| 消息输入 | `.message-editor`、`.message-editor-toolbar-primary`、`.message-editor-toolbar-secondary`、`.message-editor-mode-switch`、`.message-editor-markdown` |
| 对话主题 | `.message-conversation[data-message-theme="atha"]` 内的 `--message-*` 语义令牌；当前只存在 Atha 默认主题 |
| 进度与统计 | `.progress-panel`、`.reading-statistics`、`.progress-scrubber`、`.progress-book`、`.progress-position` |
| 阅读设置 | `.preferences-backdrop`、`.preferences-panel`、`.settings-list`、`.settings-view`、`.reading-mode-cards`、`.module-settings`、`.css-editor-*` |
| 主题 | `reader/theme.css` 中唯一颜色 token 及 `data-theme="light|paper|dark"` 覆盖；功能 CSS 只消费语义变量 |

图标按钮的可点击尺寸由 `.icon-button` 控制。底部顺序由 `BottomToolbar.svelte` 决定；`.toolbar` 固定为五等分，阅读设置只在底栏出现。不要把工具栏移进 `.reader`，否则系统缩放会改变控件尺寸，或工具层会参与书页布局。

## 交互连接

- `app.mjs` 的 `toggleReaderTools()` 先撤销待处理的选区动作，再切换 `data-reader-tools`；隐藏时同时关闭已打开面板。全屏目录和笔记页的返回按钮使用同一关闭入口；根级 `contextmenu` 监听统一禁止 WebView 默认右键菜单。
- `#reading-mode` 只接受 `paged` 和 `scroll`。左右模式按左 35%、中间 30% 和右 35% 处理页区点击，单指横向拖动实时移动正文并在 300ms 内收束；上下模式把纵向手势交给 `.reader` 原生滚动，到顶部 / 底部后再以前后意图跨 section。
- 表格中心单击打开安全投影后的全屏查看层；左右区域点按和表格上的横向拖动继续翻页，不再由内嵌表格截获。图片、公式与表格查看层参考 Readest 的暗色全屏结构，右侧固定关闭、放大、缩小和复位按钮，顶部显示 50%–400% 缩放值；双指按中心点连续缩放，桌面滚轮按指针位置连续缩放，单指和放大后继续原生滚动。普通图片还可点击图片外黑色区域直接关闭并把焦点还给阅读页；图片、缩放控件、表格和其他预览不会触发该捷径。代码块仍以双击或键盘打开普通预览。
- 华为 WebView 114 对 adb 和部分真实触摸会给出空 `pointerType`，`interaction.mjs` 将其归一为 touch；原生 `pan-y` 取消 pointer 时使用仍会到达的 `touchend` 处理章节边界。正文与 reader 的 touch 监听复用 inside / outside 去重，闭合 Shadow DOM 内的链接、表格、代码、dialog 和选择不会被外层事件误判。
- `#add-bookmark` 是唯一书签切换入口。`bookmarks.mjs` 在当前位置添加或取消书签，并把已有书签作为 `#toc` 中对应章节后的 `option[data-bookmark-id]`；投影目录中的章节或书签完成跳转后自动关闭目录。
- `#brightness` 在拖动时预览根元素的 `--reader-brightness`，松开后写入应用偏好；亮度滤镜只作用于 `.reader`，不改变系统控件亮度。
- `#font-size` 是原生 range，input burst 每帧只预览一次，change 后才通过原 Locator 提交重排；`#density` 使用 1.55 / 1.8 / 2.05 无单位行距。`#page-margin` 按书选择 24 / 32 / 48 设备像素左右边距，`#paragraph-indent` 以顶格 / 2em 卡片切换，`#paragraph-spacing` 生成受控可视 CSS。上下 144 设备像素安全区固定不变，旧应用记录中的四个自由边距字段仍会被忽略。
- CSS 模块页直接复用每书偏好：最多 32 个模块，支持搜索、分组、排序、批量启停和 schema 1 JSON 导入导出；独立 codec 统一解析、序列化、字段、大小、重复 ID 与 CSSOM 校验，不包含网络或 provider registry。新模块单个 32 KiB、启用组合 64 KiB，超限旧 CSS 只作为停用恢复副本保留。CodeMirror 在页面首次可见时按需加载，100 ms 显示 lint，180 ms 后通过同一 textarea 触发预览；输入草稿绑定原模块，任一验证、重排或持久化失败均恢复上次有效状态、渲染与 Locator。
- `#progress-range` 使用 0–1 连续值映射全书 section 和本节页，避免整数刻度在多章节书籍中丢失当前页，也不预布局其他 section；章节、百分比和本节页数都由 Navigation 的既有稳定状态更新。
- 进度面板在进度摘要和拖动条之间投影今日、近 7 天、本书与连续阅读。桌面为四列，600 px 及以下为 2 × 2；指标使用分隔线而非嵌套卡片。统计在工具层打开时暂停，关闭后由同一阅读状态接口恢复。
- 原生正文选区在 `pointerup`、键盘选择或稳定后的 `selectionchange` 投影紧凑四图标 `#selection-actions`；`content.selectionRange()` 以真实 `Range.collapsed` 与正文归属为准，避开华为 WebView 114 对闭合 Shadow DOM 的 `Selection.isCollapsed` 误报。复制只触发浏览器 copy，标注和笔记在 Tauri 产品中写入同一根 Message。点击 CSS Highlight 覆盖的已有标注会恢复其选区；“重选”后再次拖选并保存会追加 SourceAnchor/SourceSnapshot，笔记动作追加修订，删除写入墓碑。全屏 `#annotations` 支持章节和全文筛选；点击项目打开对话浮层，独立编辑和删除按钮不触发跳转。
- 选区“查词”复用同一待处理 Range，只向上下文词典面板发送受限查询；阅读顶栏不再常驻词典按钮。桌面保持锚定浮层，640 px 及以下按 RD-24 / RD-25 使用 75% 高底部抽屉、遮罩、固定词头与独立滚动区，点击遮罩或关闭按钮回到原阅读位置，并尊重 reduced-motion。当前词典、导入、移除和独立于正文的 85%–175% 六档释义字号在应用设置中管理；schema 1 本地设置只保存当前词典 ID 与允许字号，损坏、已移除来源或存储不可用时回退。可见词头下以应用内建样式呈现后端白名单允许的音标、词性、段落、义项、例句、列表、引用、表格、ruby 与上下标；来源 class、ID、内联样式、地址、脚本、资源和 CSS 均不进入 DOM，不加载网络或富 MDD 内容。
- `#message-conversation` 默认从底部占约半屏，拖动顶部把手可连续调高，轻点把手或标题栏全屏按钮可进入全屏；`app.mjs` 用 `VisualViewport` 的高度和顶部偏移约束普通、全屏与展开输入器，软键盘出现时保持编辑器和发送操作位于可视区，并把当前输入滚到最近位置。标题栏不提供收起、导出或共享。顶部可切换本条、本章和本书：本条显示当前 Conversation 的原文短预览、回复与更多；本章和本书是只读聚合记录，可按创建时间或根 Message Locator 的书内位置排列，点击“打开”后进入对应本条对话再写入。被回复消息和额外引用都以大引号摘要显示在回复正文上方，正文下方只常驻时间、回复和更多。每个摘要只读取直接目标自身的正文，不递归展开或复制目标已有的引用。编辑、删除、修订、关系、历史快照与跳回等低频动作进入更多菜单；引用摘要可跳到当前对话目标并短暂高亮。笔记全屏页以可见返回按钮回到正文，导出本书消息只放在右上角更多菜单。
- `.message-editor` 随内容增高，到达紧凑上限后出现全屏按钮。全屏编辑顶部使用两层工具栏：第一层切换可视/Markdown 输入并保留撤销、重做与返回紧凑输入，第二层显示标题、粗体、斜体、列表、引用和安全链接。Markdown 转换按需加载，切回可视模式或发送前必须通过同一正文 schema；不支持的格式保留原文并显示错误，不静默丢失。
- 设置入口位于阅读底栏，使用原生 `<details>`、backdrop、Escape、焦点返回和 CSS transition；600 px 以下为自适应高度底部抽屉，子页按内容收缩并以 72dvh 为上限，`prefers-reduced-motion` 下关闭进入、返回和翻页收束动画。
- `#reader-back` 优先使用浏览器历史；没有历史时请求关闭当前阅读窗口。

## 当前有意暂缓

- TTS 暂缓，当前不提供界面入口、播放逻辑或持久化字段；
- 标注颜色、样式、notebook 与同步没有阅读界面入口；
- 桌面横屏和大屏布局尚未设计；
- 当前使用 Lucide Svelte 图标、原生表单控件和按需加载的 Tiptap 消息编辑器；尚未引入通用 UI 组件库或动效框架。
- 当前只实现 Atha 默认聊天主题；微信、Telegram、QQ 风格模拟、主题选择和用户自定义界面等待消息主循环验收后单独设计。
- 对话字号和密度已使用较紧凑默认值；类似 Telegram 的界面字号、密度与主题设置等待当前布局稳定后再增加，不预留持久化字段。
- 阅读统计暂不提供趋势图、目标、导出、账户或同步，也不为这些方向预留页面和 schema。

## 截图证据

本机最新截图位于 `artifacts/local/screenshots/`：

- `reader-shell-01-reading.png`；
- `reader-shell-02-tools.png`；
- `reader-shell-03-directory.png`；
- `reader-shell-04-progress.png`；
- `reader-shell-05-settings-menu.png`。
- `message-scope-mark-430x820.png`；
- `message-scope-chapter-430x820.png`。
- `atha-reading-statistics-linux.png`；
- `atha-reading-statistics-linux-mobile.png`。

微信读书源图与实现的同图对照位于 `artifacts/local/audits/reader-shell-usability/`。根目录 `design-qa.md` 记录尺寸归一、交互证据和修复历史；最终没有 P0、P1 或 P2 问题。

Readest 原图、逐图观察和本次 Linux 统计实现副本位于忽略目录 `fixtures/local/readest/`；统计设计复核使用 WR-05 与 RD-03，不以文字报告替代原图。

本次表格缩略图、表格全屏查看、图片全屏查看与 200% 放大原图位于忽略目录 `artifacts/local/audits/content-viewer-headless/`，并附 `SHA256SUMS`；这是 Chromium production build 的本地视觉证据，不替代 PCT-AL10 实机触摸。

PCT-AL10 上 Atha 最终原生选区与词典抽屉原图、说明和 SHA-256 位于 `artifacts/local/audits/offline-dictionary-pct/`；动作栏与抽屉设计复核使用 RD-22、RD-24、RD-25 与 RD-27。阅读设置菜单、字号、布局、阅读方式和纵向滚动原图位于 `artifacts/local/audits/reader-controls-pct/`；设置层级参考本地 RD-* 原图，字号、缩进与滚动行为以同机真实交互复核。

词典富文本与设置的当前真机证据位于忽略目录 `artifacts/local/audits/dictionary-pct-revalidation-20260813/`。PCT-AL10 覆盖安装同签名 arm64 release 本地测试候选后，书架和词典仍在；ADB 自动化确认底栏为目录、搜索、笔记、进度四项，选区查词呈现安全语义结构，设置页无明显遮挡并列出 85%–175% 六档字号，150% 在进程重启后保持，最终恢复 100%。该目录含私有内容，只能用于本地复核；ADB input 不等同于自然手指触摸。

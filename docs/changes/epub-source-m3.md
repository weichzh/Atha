# M3 EPUB 真实书籍来源

## Status

implemented

## Problem

M2 阅读器只能打开预先准备的书根和 schema 1 manifest，用户不能把真实 EPUB 直接交给产品。M3 需要用一种真实格式闭合“书籍文件 → 受控内容输入 → WebView2 阅读会话”，同时保持现有渲染、安全和状态语义不变。

指定样本《数学及其历史 (2026)》是 EPUB 3.0：单一 OPF、173 个 spine section、2701 个 manifest item 和 EPUB3 navigation document。它已经以 fixture 形式通过 M2，因此本 change 只负责可靠地产生等价 R1 输入，不重写阅读内核。

## Scope

- Windows host 新增 `--epub <path>`，与既有 `--book-root` 加 `--manifest`/`--entry` 两种输入互斥；
- 后端增加一个 UTF-8 XML EPUB3 导入 module：读取 `META-INF/container.xml`、单一 OPF package、manifest、spine 和 EPUB3 TOC，把 XHTML spine 与当前阅读器支持的 CSS、SVG、PNG、JPEG、GIF、WebP 写成 schema 1 书根；
- 允许 XHTML 内联 SVG 的 `image href` 经过既有本地资源声明与同源校验后加载，用于呈现指定样本的 spine 封面；其他 SVG 外部引用仍拒绝；
- 使用源文件 SHA-256 作为 `contentVersion` 和本机导入缓存目录名；host 把缓存放在 `%LOCALAPPDATA%/Atha/ImportedBooks`，相同字节跨路径复用同一状态键；
- 信任边界拒绝非普通文件、非 EPUB mimetype、多 package、加密内容、DOCTYPE、外部 URL、路径穿越或 Windows 路径歧义、重复路径、未知 spine 类型、超过 512MiB 的源文件/解压总量、超过 10000 个成员、超过 16MiB 的单成员；
- 用指定 EPUB 验证 173 个 section、197 个 TOC link、2527 个受支持资源，并由真实 WebView2 host 完成前三个真实 spine section 的加载、释放、重开与网络安全探针；内容特定能力仍由 M2 困难样本 gate 验证。

## Non-Goals

- 不增加书架、文件选择器、拖放、最近阅读、封面/元数据 UI、文件关联或安装包；
- 不增加 EPUB2/NCX fallback、多 rendition、SVG spine、远程资源、加密/混淆、损坏 EPUB 修复或格式兼容工厂；
- 不修改阅读界面、R1 manifest schema 或 WebView2 渲染路径，不扩展封面所需 `image href` 之外的内容能力；
- 不把 fixture exporter 变成产品导入器，也不依赖 Python 运行产品。

## Acceptance Criteria

- [x] `atha-reader-host --epub <path>` 可直接打开指定 EPUB，并与既有 CLI 参数严格互斥；
- [x] 导入结果的内容版本等于源 SHA-256，spine 顺序、TOC 和受支持资源数量精确匹配样本；
- [x] 同一内容从不同路径打开会复用内容身份，源文件改变则生成新身份；
- [x] ZIP、XML、路径、大小、外部资源和加密边界有最小可运行的失败检查，失败不留下可被误认成完整书籍的缓存；
- [x] 真实 EPUB import probe、既有 Rust、reader sample、M2 reader gate 与文档检查继续通过；
- [x] 独立 review 对规格、标准、安全边界和过度设计无 blocking。

## Files And Steps

1. 固定 EPUB3 输入、资源与缓存边界；
2. 在后端实现单一深 module，将 ZIP/OPF/nav 细节收在 `import_epub` interface 后；
3. 在 Windows host 解析 `--epub`，导入后复用现有 `BookRoot`、manifest URL、状态与诊断路径；
4. 增加最小合成 EPUB 自检和指定样本的真实 WebView2 检查；
5. 更新阅读架构、代码地图、路线图和本文件结果，完成独立 review。

## Checks

- `pwsh -NoProfile -File scripts/check-epub-source.ps1`；
- `pwsh -NoProfile -File scripts/check-reader-gate.ps1`；
- workflow `docs` gate；
- 本次中文 Markdown 的 `autocorrect --fix` 与 `autocorrect --lint`；
- `git diff --check`。

## Rollback

回滚本 change 的提交即可恢复旧 CLI。导入产物只位于本机 `%LOCALAPPDATA%/Atha/ImportedBooks` 缓存；旧版本不会读取它，也没有用户数据迁移。

## Approval

用户在确认“先用薄的 M3 真实输入，再根据真实使用升级界面”后明确要求开始。本 change 不扩大到 UI 或其他产品能力。

## Result

后端新增单一 `import_epub` interface，把 ZIP 检查、namespace-aware EPUB3 container/OPF/navigation 解析和 schema 1 manifest 生成收在三个同职责文件中。Windows host 新增 `--epub`，导入到按源 SHA-256 命名的本机缓存后复用现有 `BookRoot`、WebView2 协议、状态键和阅读能力；CLI 没有新增格式工厂或第二条渲染路径。

真实样本导出 173 个 section、2527 个受支持资源和 197 个 TOC item。为其内联 SVG 封面增加的 `image href` 仍经过既有 manifest 声明、书根和同源校验；外部引用继续失败。合成 EPUB 自检覆盖稳定身份、路径逃逸、DOCTYPE、外部引用、加密标记、超大源文件和失败缓存清理。

## Review

- Spec：初审发现真实 gate 复用旧缓存、ZIP/mimetype 负向检查缺失，以及 XML 合法显式结束标签与闭合边界不足；改成隔离冷导入加第二次复用，并补全合成检查后终审无 blocking、无 scope creep；
- Standards/Security：初审发现 container 可忽略额外 rootfile、XML 未锚定 namespace/层级、事实状态提前闭合；解析器改为 OCF/OPF/XHTML 状态机，事实退回进行中直至复核，终审无 blocking；
- Ponytail/Codebase design：终审无 blocking；`import_epub` 保持唯一公开深 interface，没有 adapter、trait、factory 或第二渲染路径，内聚的 package interpretation 不按行数机械拆分。

## Evidence And Residual Risks

- `scripts/check-epub-source.ps1` 通过；固定源 SHA-256 为 `0af5dff0c0d1eb369a096b18d05eb77a4cd9c03808748db8274d5e77bbfe7368`，隔离缓存上的真实 Windows WebView2 host 冷导入精确验证 173/2527/197，第二次打开复用且未重写 manifest；
- `scripts/check-reader-gate.ps1` 通过；大书搜索为 288 条、覆盖 104 个 section，三轮进程树峰值为 638.3、628.8、636.5MiB，崩溃恢复通过；benchmark `1785720849357-40976` 的冷启动、首稳、热开、翻页与重排 P95 为 898.631、151.900、23.800、7.800 和 32.000ms，均低于既有门槛；
- 最高证据等级为当前 Windows 设备上的真实 WebView2 本地运行。未覆盖 EPUB2/NCX fallback、多 rendition、非 UTF-8 XML、加密/混淆、安装包、文件选择 UI、跨设备性能或损坏缓存自动修复；同进程并发导入也不是当前串行 CLI 的契约。尺寸、成员数与常见 Windows 路径有实现和部分负向检查，但尚未逐个生成 16MiB、512MiB、10000 成员与所有设备名边界样本。

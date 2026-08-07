---
description: 调查 Readest 桌面与 Android 的格式能力、实现依赖，并据此规划 Atha 的 Android 优先多格式路线。
---

# Readest Android 与格式能力研究

## 结论先行

1. **事实：Readest 的 Android 版已经正式分发，不只是“源码可编译”。** 官方 README 同时给出 Google Play、官网与 GitHub Releases 下载入口；桌面正式覆盖 macOS、Windows、Linux。当前稳定版是 `v0.11.20`，发布日期为 2026-07-19。
2. **事实：Readest 公开声明的阅读格式是 EPUB、PDF、MOBI、KF8（AZW3）、FB2、CBZ、TXT、MD。** 源码还把 `.azw` 作为正式文件关联和导入格式。本项目明确排除 PDF，因此 Atha 的目标集合应写成：**EPUB（含 EPUB 2/NCX 兼容）、无 DRM 的 MOBI/AZW/KF8/AZW3、FB2、CBZ、TXT、MD**。
3. **事实：Readest 没有为每种格式都使用原生库。** EPUB、MOBI/KF8、FB2、CBZ 的完整阅读模型主要来自 MIT 许可的 `readest/foliate-js`；ZIP 随机访问使用 `zip.js`，KF8 字体解压使用 `fflate`。TXT 与 Markdown 是 Readest 自己实现的适配器。Rust 侧的 EPUB/MOBI 代码只是导入元数据、封面或预取快路径，不替代完整阅读解析。
4. **推断：Atha 不应迁移到 Readest 的 Next.js/React/foliate-view 整套 UI。** Atha 已有 Svelte 5、Tauri 2、受控书根、单 WebView 阅读内核、Locator 和更严格的不可信内容边界。最低风险方案是保留这些事实，只复用格式解析器、标准和成熟基础库，把各格式规范化为现有 `ReaderManifest`/书根。
5. **建议：第一个可交付纵切只做“现有 EPUB 在 Android 真机完整跑通”。** 它必须覆盖选书、导入、打开、分页/滚动、恢复位置、触摸选择、后台/前台恢复和诊断日志。随后按 `EPUB 2/NCX → CBZ → MD → TXT → FB2 → MOBI/KF8` 扩展；MOBI/KF8 最后进入，并设置 Android 真机性能门禁。
6. **PDF 明确不进入任务。** 不引入 PDF.js，不为 PDF 预留抽象，不把 Readest 的 PDF 适配器算入 Atha 验收。

## 范围、版本与证据等级

本报告只使用官方仓库、官方文档、标准与依赖元数据。检索日期为 **2026-08-07**。

| 对象 | 锚点 | 用途 |
| --- | --- | --- |
| Readest 稳定版 | [`v0.11.20`](https://github.com/readest/readest/releases/tag/v0.11.20)，tag commit `1df1505fc5033fc949463c9908f2d53bd0fbdfa6`，2026-07-19 | 已发布产品状态 |
| Readest 当前源码 | [`2b719600c27b4c9c91bef7b2bb148b3251338ea7`](https://github.com/readest/readest/commit/2b719600c27b4c9c91bef7b2bb148b3251338ea7)，作者时间 2026-08-07 23:00:51 +08:00 | 下述实现细节；可能晚于稳定版 |
| Readest 固定的 foliate-js | [`f65836f77e8b66b84baacd54bfc92096578e7a84`](https://github.com/readest/foliate-js/commit/f65836f77e8b66b84baacd54bfc92096578e7a84)，2026-08-07 | 格式解析与渲染依赖 |
| Atha 当前基线 | `424b7b4e731dc5e4d4e1cda032af6aa6be1d698c`，2026-08-07 22:52:22 +08:00 | 本地架构比较 |

“事实”表示可以由上述源码、文档或标准直接验证；“推断/建议”表示结合 Atha 现状做出的工程判断。源码中的性能注释只作为 Readest 团队的实现依据，不当作 Atha 已复现的 benchmark。

## Readest 的桌面与 Android 支持状态

### 产品与运行时

**事实：** Readest 官方 README 将应用描述为 Next.js 16 + Tauri 2 的跨平台阅读器，覆盖 macOS、Windows、Linux、Android、iOS 与 Web，并列出移动商店和平台下载入口（[README](https://github.com/readest/readest/blob/2b719600c27b4c9c91bef7b2bb148b3251338ea7/README.md)）。这意味着：

- macOS、Windows、Linux 是已分发桌面目标；
- Android 有 Google Play 版，也有官网/GitHub APK；
- 同一 Web 前端并不等于同一浏览器内核。Tauri 官方说明 Windows 使用 WebView2，macOS/iOS 使用 WKWebView，Linux 使用 WebKitGTK，Android 使用系统 Android WebView（[Tauri WebView 版本](https://v2.tauri.app/reference/webview-versions/)、[Tauri 仓库](https://github.com/tauri-apps/tauri)）。

**限制：** Tauri 不随 Android 应用打包浏览器引擎。Android 的 CSS、DOM、选择行为和性能取决于设备当前选中的 WebView provider 及其 Chromium 版本；桌面 WebView2 的通过结果不能替代 Android 真机结果。

### Android 构建栈

当前 `main` 的可验证构成如下：

| 层 | Readest 当前实现 | 证据 |
| --- | --- | --- |
| Web UI | Next.js `16.2.11`、React `19.2.8`、静态导出到 Tauri，pnpm；Tauri CLI `2.11.4` | [`package.json`](https://github.com/readest/readest/blob/2b719600c27b4c9c91bef7b2bb148b3251338ea7/apps/readest-app/package.json)、[`next.config.mjs`](https://github.com/readest/readest/blob/2b719600c27b4c9c91bef7b2bb148b3251338ea7/apps/readest-app/next.config.mjs) |
| 原生核心 | Tauri 2 + Rust；Android 专属能力通过 Kotlin Tauri 插件桥接 | [`src-tauri/src/lib.rs`](https://github.com/readest/readest/blob/2b719600c27b4c9c91bef7b2bb148b3251338ea7/apps/readest-app/src-tauri/src/lib.rs)、[Tauri mobile plugin 文档](https://v2.tauri.app/develop/plugins/develop-mobile/) |
| Android 平台 | `minSdk 26`、`compileSdk/targetSdk 36`、JVM target 1.8；release 开启 R8/minify | [`tauri.conf.json`](https://github.com/readest/readest/blob/2b719600c27b4c9c91bef7b2bb148b3251338ea7/apps/readest-app/src-tauri/tauri.conf.json)、[`build.gradle.kts`](https://github.com/readest/readest/blob/2b719600c27b4c9c91bef7b2bb148b3251338ea7/apps/readest-app/src-tauri/gen/android/app/build.gradle.kts) |
| ABI/制品 | Rust 构建 `arm64-v8a`、`armeabi-v7a`、`x86`、`x86_64`；发布 universal APK 与单独 arm64 APK | [`release.yml`](https://github.com/readest/readest/blob/2b719600c27b4c9c91bef7b2bb148b3251338ea7/.github/workflows/release.yml) |
| 开发环境 | Android Studio、SDK/Platform Tools、NDK、JDK/Rust Android targets；Readest CI 固定 JDK 17、NDK `28.2.13676358` | [`CONTRIBUTING.md`](https://github.com/readest/readest/blob/2b719600c27b4c9c91bef7b2bb148b3251338ea7/CONTRIBUTING.md)、[`android-e2e.yml`](https://github.com/readest/readest/blob/2b719600c27b4c9c91bef7b2bb148b3251338ea7/.github/workflows/android-e2e.yml)、[Tauri prerequisites](https://v2.tauri.app/start/prerequisites/) |
| 自动化 | API 34 x86_64 emulator 上构建并运行 CDP E2E；nightly、手动或带 `e2e-android` 标签的 PR 才运行，不是所有 PR 的阻塞门禁 | [`android-e2e.yml`](https://github.com/readest/readest/blob/2b719600c27b4c9c91bef7b2bb148b3251338ea7/.github/workflows/android-e2e.yml) |

### Android 已暴露的限制

1. **文件入口不是桌面路径。** Tauri 官方 dialog 在 Android 返回 content URI；新版 API 的默认 `fileAccessMode: "copy"` 会把选择结果复制到 app sandbox，并要求调用者在不再需要时清理（[dialog 文档](https://v2.tauri.app/reference/javascript/dialog/)）。Readest 额外实现 Android Storage Access Framework、persistable URI grant、冷启动事件重放与 `ACTION_OPEN_DOCUMENT`，因为系统 picker 期间可能发生 Activity/进程回收（[`NativeBridgePlugin.kt`](https://github.com/readest/readest/blob/2b719600c27b4c9c91bef7b2bb148b3251338ea7/apps/readest-app/src-tauri/plugins/tauri-plugin-native-bridge/android/src/main/java/NativeBridgePlugin.kt)、[`useAndroidFilePicker.ts`](https://github.com/readest/readest/blob/2b719600c27b4c9c91bef7b2bb148b3251338ea7/apps/readest-app/src/hooks/useAndroidFilePicker.ts)）。Atha 当前把 dialog 结果直接 `into_path()`，在 Android content URI 上不能沿用。
2. **Android 不支持官方 dialog 的文件夹选择。** 这对首个“单文件导入”纵切无影响；不要为了首版复刻 Readest 的文件夹扫描桥。官方插件平台表也明确标注 Android/iOS 不支持 folder picker（[Tauri dialog](https://v2.tauri.app/zh-cn/plugin/dialog/)）。
3. **Android WebView 的拦截式 Range 请求有实际坑。** Readest 为本地大文件实现 `rangefile:` 自定义协议，把 `start/end` 放到 URL 而不是 `Range` header，并把单次响应限制为 8 MiB；源码说明这是为规避 Android WebView 对拦截响应二次应用 offset 的问题（[`range_file.rs`](https://github.com/readest/readest/blob/2b719600c27b4c9c91bef7b2bb148b3251338ea7/apps/readest-app/src-tauri/src/range_file.rs)）。Atha 的 EPUB 已解包为受控资源，首个 EPUB 纵切不必照搬；以后若让 JS 随机访问原始 MOBI/ZIP，必须重新评估。
4. **旧/定制 WebView 是真实设备风险。** Readest 固定了 MIT 的 `tauri-plugin-webview-upgrade`，系统 WebView 低于 Chromium 121 时尝试切换到用户侧载的 Google WebView，低于 92 时提示不受支持（[`tauri.conf.json`](https://github.com/readest/readest/blob/2b719600c27b4c9c91bef7b2bb148b3251338ea7/apps/readest-app/src-tauri/tauri.conf.json)、[插件固定提交](https://github.com/readest/tauri-plugin-webview-upgrade/tree/c7c04abee8a12e32823febec44779c075e076e25)）。插件只支持单体 APK、ABI 必须匹配、安装后需冷启动，且不解决 split provider/多进程差异。**建议首版不采用**；先记录 provider/version 并在目标真机上得到失败证据。
5. **内存不能靠 manifest 掩盖。** Readest Android manifest 设置了 `android:largeHeap="true"`（[`AndroidManifest.xml`](https://github.com/readest/readest/blob/2b719600c27b4c9c91bef7b2bb148b3251338ea7/apps/readest-app/src-tauri/gen/android/app/src/main/AndroidManifest.xml)）。这是源码事实，不是 Atha 应复制的优化方案；Atha 应先量化峰值 RSS/Java heap/WebView renderer 内存。
6. **触摸选择行为需要独立验收。** Readest 有 Android 专属的 `selectionchange`、原生 `touchmove`、长按结束后再弹操作面板等分支（[`useTextSelector.ts`](https://github.com/readest/readest/blob/2b719600c27b4c9c91bef7b2bb148b3251338ea7/apps/readest-app/src/app/reader/hooks/useTextSelector.ts)）。因此“页面能显示”不能代表阅读交互完成。

## 格式矩阵（PDF 明确排除）

### 对外支持与实际路由

Readest README 的公开列表与实际导入白名单、文件关联基本一致；源码额外明确了 `.azw`。普通产品入口应按下表理解，而不是把所有内部 loader 分支都承诺给用户（[`README`](https://github.com/readest/readest/blob/2b719600c27b4c9c91bef7b2bb148b3251338ea7/README.md)、[`constants.ts`](https://github.com/readest/readest/blob/2b719600c27b4c9c91bef7b2bb148b3251338ea7/apps/readest-app/src/services/constants.ts)、[`document.ts`](https://github.com/readest/readest/blob/2b719600c27b4c9c91bef7b2bb148b3251338ea7/apps/readest-app/src/libs/document.ts)、[`tauri.conf.json`](https://github.com/readest/readest/blob/2b719600c27b4c9c91bef7b2bb148b3251338ea7/apps/readest-app/src-tauri/tauri.conf.json)）。

| Atha 目标格式 | Readest 完整阅读路径 | 成熟库/引擎 | 已知边界 | 对 Atha 的最小复用判断 |
| --- | --- | --- | --- | --- |
| EPUB | `zip.js` loader → foliate-js `EPUB.init()` → foliate Book 接口/分页器；Rust 只预取 container/OPF/nav/NCX、条目大小和封面 | `foliate-js`（MIT，固定 commit）、`@zip.js/zip.js 2.8.x`；Rust `zip 2`、`quick-xml 0.36` | foliate 同时解析 EPUB 3 nav 与 EPUB 2 NCX fallback；脚本内容不受支持且必须 CSP 隔离；foliate API 自称不稳定 | 保留 Atha `zip 8.6` + `quick-xml 0.41` + 现有阅读内核，补 NCX/EPUB2，不替换成 foliate view |
| MOBI / AZW（KF7） | foliate-js `MOBI`/`MOBI6`；Rust `mobi 0.8` 仅 partial hash、封面快路 | `foliate-js/mobi.js`、`fflate`；辅助 `mobi 0.8`（MIT） | 经典 MOBI 会一次解压全部文本再按 `mbp:pagebreak` 分段；内存高风险；DRM 未被 Readest 官方承诺 | 最后实施；先以固定 foliate parser 做规范化导入 PoC，Android benchmark 不过再评估 `libmobi` |
| KF8 / AZW3 | foliate-js `KF8`，combo MOBI 优先进入 KF8；zlib 字体由 `fflate` 解压 | 同上 | foliate 官方说明 HUFF/CDIC 解压仍可能慢；图片/资源按需加载 | 与 MOBI 同一纵切、同一性能门禁，不把扩展名当能力证明 |
| FB2 | foliate-js `makeFB2()` 用 DOMParser 解 XML，将 FB2 元素、内嵌图片、TOC/metadata 转成合成 XHTML sections | `foliate-js/fb2.js`（MIT）、浏览器 DOM/XML | 没有 Readest 原生解析器；整个 XML 进入内存；需保留 Atha 的清洗与资源限制 | 固定并包裹 MIT parser，逐 section 写入现有受控书根；不复制 Readest AGPL 应用层代码 |
| CBZ | `zip.js` 随机访问 → foliate-js `makeComicBook()` → 每图一固定布局 section | `zip.js`、浏览器图片解码、`ComicInfo.xml` 标准 | 支持 jpg/jpeg/png/gif/bmp/webp/svg/jxl/avif；当前代码按文件名字符串排序且扩展名匹配大小写敏感；首图为封面 | 不需要整套 foliate；复用 Atha 已有 `zip`，按 ComicInfo 标准生成固定布局 manifest，成本最低 |
| TXT | Readest 自建 `TxtToEpubConverter`，检测编码、猜章节并生成 EPUB 2 | 没有独立成熟 TXT 引擎；ZIP 仍用 `zip.js` | 8 MiB 分大小文件路径；编码检测和中文章节规则是 Readest 自建启发式，不是标准保证 | 使用 `chardetng` + `encoding_rs` 做成熟编码层，先按段落/固定大小生成安全 XHTML；章节猜测后置并用 fixture 驱动 |
| MD | Readest 自建 `makeMarkdownBook`：`marked 15` + `marked-footnote` + DOM 清洗，按 H1 分 section | `marked`、`marked-footnote`、DOMPurify/Readest sanitizer | `.md` 是正式关联；`.markdown` loader 能识别但不是正常关联；原始 HTML 必须清洗 | Atha Rust 侧优先 `pulldown-cmark 0.13.x` 输出安全 XHTML，复用现有 manifest/内容验证 |
| PDF | foliate-js 的实验性 PDF.js adapter + 固定布局 renderer | PDF.js | foliate README 明确称 proof-of-concept/highly experimental | **排除：不实现、不依赖、不预留** |

`foliate-js` 自己明确写明：它已用于 Foliate 的多个稳定版本，但库本身没有稳定 API，推荐作为 git submodule 固定版本；归档文件建议由支持 File/HTTP Range 随机访问的 `zip.js` 提供 loader（[foliate-js README](https://github.com/readest/foliate-js/blob/f65836f77e8b66b84baacd54bfc92096578e7a84/README.md)）。因此它是“经过产品使用的解析实现”，不是可以随意升级的稳定 SDK。若采用，应只有一个 Atha adapter 直接依赖它，并固定 commit。

### 不应算作正式验收的隐藏/部分路由

- `.fbz`、`.fb.zip`、`.fb2.zip`：`DocumentLoader` 能解包其中的 FB2，Android manifest 也有 `.fb2.zip` 的手写匹配，但普通导入白名单与正式关联没有完整覆盖；先不承诺。
- `.markdown`：loader 和测试能识别，正式关联只列 `.md`。
- `.prc`：Readest 的 Tauri MOBI 快路正则识别它，但普通白名单、`BookFormat` 和文件关联没有列出；先不承诺。
- `.zip`：白名单允许选择，但 loader 对非 CBZ/FBZ ZIP 默认按 EPUB 打开；ZIP 是容器，不是独立阅读格式。
- KFX、AZW4、CBR/RAR、DOCX：不在 Readest 的正常阅读格式面内。`mammoth` 等依赖服务于其他转换功能，不能据此推断为可读格式。

### DRM 边界

**事实：** Readest README 没有声明 DRM 支持；foliate-js 源码能读取 MOBI encryption header，但官方说明未承诺解密能力。Atha 当前也明确拒绝加密 EPUB。

**建议：** 验收条件写成“DRM-free EPUB/MOBI/AZW/AZW3”。不要把 KFX、Kindle DRM 或 Adobe DRM 混进格式支持任务；遇到加密输入应稳定拒绝并记录原因。

## Atha 与 Readest 架构比较

| 维度 | Readest 当前源码 | Atha 当前基线 | 含义 |
| --- | --- | --- | --- |
| 产品壳 | Next.js/React | Vite/Svelte 5 | Tauri 与格式能力不要求换前端；保留 Atha |
| 阅读抽象 | foliate `Book` 接口 + foliate view/paginator，原始书文件运行时解析 | 受控 `BookRoot` + `ReaderManifest` + Atha reader kernel | 复用 parser，不替换渲染、Locator、消息语义 |
| EPUB | JS 完整解析 + Rust 预取/封面快路；支持 nav/NCX | Rust `zip` + `quick-xml`，只接受受约束 EPUB 3/nav | 首个格式增量应先补 EPUB2/NCX |
| 平台运行时 | 多桌面 WebView + Android system WebView | 目前只验证 Windows WebView2 | Android 需要新的真实目标证据，不可沿用 Windows 结论 |
| 文件入口 | 桌面路径；Android SAF/content URI/原生桥 | dialog 结果直接 `into_path()` | Android 首切必须先解决 sandbox copy/content URI |
| 内容安全 | foliate 明确要求 CSP；Readest 另有 sanitizer/transformer | 导入、协议、manifest、closed Shadow DOM 多层拒绝脚本/网络/越界资源 | 不让新格式绕过现有书根与清洗边界 |
| 定位与消息 | CFI/foliate section 位置 | Atha 自有 Locator、内容版本与消息重锚 | 格式输出必须确定性，避免重导后位置漂移 |
| Android 平台代码 | Kotlin 插件、SAF、旧 WebView、e-ink、触摸选择分支 | `atha-reader-host`/`tao`/WebView2 窗口逻辑仍被产品 app 无条件依赖 | 必须先做平台隔离，不能只运行 `tauri android init` |

Atha 当前 Android 编译的直接阻塞包括：产品 app 无条件依赖只在 `cfg(windows)` 导出 API 的 `atha-reader-host`；composition root 无条件使用 `tao::dpi`、桌面窗口尺寸、主显示器与 WebView2 profile 目录。应将这些限定在 desktop/Windows adapter，而不是移动 backend 或阅读内核。

## 推荐的最小交付纵切与后续顺序

### 纵切 0：先补可观察性，不扩大产品范围

在 Android 构建前完成当前代码的日志 seam 检索，并确保以下事件具有稳定字段：平台/版本、Android API、WebView provider/version、ABI、导入阶段与耗时、书籍格式/大小（不记录私人路径/正文）、解析失败码、首个稳定页、WebView renderer 崩溃/进程恢复。日志库与具体实施由独立日志研究决定；这里仅把它列为 Android 验收前置。

### 纵切 1：Android 上跑通现有 EPUB

目标是证明 Atha 的现有产品架构能在 Android 成立，不同时引入新格式。

1. 将 `atha-reader-host`、`tao`、主显示器、窗口尺寸和 WebView2 data directory 限定到 Windows/desktop；保持 backend、Svelte 与 reader kernel 共享。
2. 用 Tauri 2 官方 Android 初始化与 Rust 四 target 配置生成工程；首个开发/性能目标只构建 arm64，CI 再补 x86_64 emulator。
3. 文件选择优先使用官方 `@tauri-apps/plugin-dialog` document picker 的 sandbox `copy` 模式；导入完成后清理临时副本。不要在首版实现文件夹 picker、自定义 SAF 桥或“原地阅读”。
4. 把 sandbox 文件送入现有 `LocalLibrary::import`，继续生成受控 book root；阅读仍经现有 `atha-book`/manifest/closed Shadow DOM，不让 content URI 或原始文件直接进入正文 WebView。
5. 真机验收至少覆盖：冷启动选书、后台 picker 后恢复、导入、书架、打开、首个稳定页、翻页/滚动、旋转/尺寸变化、位置恢复、长按选择与面板、后台/前台、异常书拒绝、零外联安全探针。
6. 记录 Android WebView provider/version；只在真实目标出现空白页或关键 Web API 缺失后研究 webview-upgrade。

这是最小纵切，因为它既验证平台 seam，也复用全部现有 EPUB、安全和消息代码；若它未通过，增加格式只会叠加未知量。

### 纵切 2：EPUB 2 / NCX fallback

在现有 Rust EPUB importer 中识别 OPF 2 与 `<spine toc="…">`，解析 NCX `navMap/pageList`，仍输出同一个 `ReaderManifest`。参考 W3C EPUB 3.3 对 legacy NCX 的兼容章节，以及 foliate-js 已验证的 nav→NCX fallback（[EPUB 3.3](https://www.w3.org/TR/epub-33/)、[`epub.js`](https://github.com/readest/foliate-js/blob/f65836f77e8b66b84baacd54bfc92096578e7a84/epub.js)）。不引入“通用格式工厂”。

### 纵切 3：CBZ

复用现有 Rust `zip`，只接受白名单图片与 `ComicInfo.xml`；每图生成一个固定布局 section，按规范化自然排序，明确处理大小写扩展名。元数据以 Anansi 的 ComicInfo 文档为事实来源（[ComicInfo](https://anansi-project.github.io/docs/comicinfo/intro)）。CBZ 不需要 foliate paginator，也不需要运行任意 HTML。

### 纵切 4：Markdown，然后 TXT

- Markdown：采用 [`pulldown-cmark 0.13.x`](https://docs.rs/crate/pulldown-cmark/latest)（MIT、CommonMark pull parser），输出经现有内容校验的 XHTML；显式决定 raw HTML 是拒绝、转义还是白名单清洗。先实现 H1/heading TOC，不复制 Readest 的 AGPL Markdown 适配代码。
- TXT：采用 [`chardetng`](https://github.com/hsivonen/chardetng) + `encoding_rs`，BOM/UTF-8 优先，检测结果可由用户覆盖。`chardetng` 官方也说明短 CJK 文本会误判，因此 fixture 必须包含 UTF-8、UTF-16、GBK/GB18030、Big5、Shift_JIS。首版按段落或有界字节数切 section；中文网文章节猜测后置。

两者都只增加“文本→安全 XHTML→现有 manifest”的浅 adapter。

### 纵切 5：FB2

固定 `readest/foliate-js` 的 MIT `fb2.js`，封装为单一 import adapter；读取 metadata/TOC/section 后逐 section 写入受控书根，再经过 Atha 的资源、HTML 与 CSS 校验。先不支持 FBZ。若在无 DOM 的导入环境接入成本过高，再用已经存在的 `quick-xml` 实现等价转换，但必须先用同一 corpus 对照 foliate 输出，避免凭印象重写格式语义。

### 纵切 6：MOBI / AZW / KF8 / AZW3

1. 先固定 foliate-js `mobi.js` + `fflate`，只通过一个 adapter 输出 Atha 现有 manifest；MOBI、AZW、AZW3 共享实现，格式标签由文件扩展与解析结果共同决定。
2. Android corpus 必须包含：经典 PalmDOC/MOBI、HUFF/CDIC MOBI、KF8、combo MOBI/KF8、压缩字体、大图片、长书、损坏书和加密书拒绝。
3. 若 foliate 在真机上超过门禁，再评估 [`libmobi`](https://github.com/bfabiszewski/libmobi)：它支持 MOBI/PRC/KF8/AZW/AZW3/AZW4、Android 构建与内容重建，但引入 C/FFI、交叉编译和 LGPL-3.0 合规成本。不要在 benchmark 前先引入。
4. Readest 使用的 Rust [`mobi 0.8`](https://docs.rs/mobi) 结构持有完整 `Vec<u8>`，当前 Readest 也只用它做 hash/封面；它不能被视为已经证明的 KF8 完整阅读替代品。

### 不采用的路线

- 不换成 Next.js/React；Tauri 官方支持任意可编译为 HTML/CSS/JS 的前端。
- 不替换 Atha reader kernel、Locator、消息模型和内容安全边界为 foliate view。
- 不复制 Readest 应用层源码。Atha 已采用 `AGPL-3.0-or-later`，但这不会自动批准 Readest 的源码或消除版权 / 修改说明义务；优先复用 MIT 的 foliate-js、标准和独立许可依赖，并在合并第三方源码前核对精确 `-only` / `-or-later` 与实际分发方式（[Readest LICENSE](https://github.com/readest/readest/blob/2b719600c27b4c9c91bef7b2bb148b3251338ea7/LICENSE)、[foliate-js LICENSE](https://github.com/readest/foliate-js/blob/f65836f77e8b66b84baacd54bfc92096578e7a84/LICENSE)）。
- 不预建格式插件系统、generic factory 或远程格式服务。每个 importer 直接实现“输入 → 现有书根/manifest”，等至少两个实现出现真正重复后再提取。
- 不引入 PDF.js、Readest webview-upgrade、`largeHeap` 或 native MOBI 快路，除非对应实测问题出现。

## Android 性能门禁

Readest 源码说明了两个应重点防守的瓶颈：Android WebView 本地大文件随机读取，以及 MOBI/KF8 解压/解析。Atha 每个格式纵切应在固定 corpus 上记录至少：

| 指标 | 冷路径 | 热路径/交互 |
| --- | --- | --- |
| 导入 | 总耗时、解析/解压/写盘分段耗时、失败码 | 重导命中缓存耗时 |
| 打开 | 点击到首个稳定页、首屏资源数、WebView provider/version | 已导入书再次打开 |
| 阅读 | 首次布局、章节切换 | 翻页/滚动 frame time 与 P50/P95 |
| 内存 | host RSS、WebView renderer RSS、Java heap、峰值与稳定值 | 连续读 20 个 section 后是否回落 |
| 稳定性 | 冷启动、picker 回收、后台/前台、低内存恢复 | 位置/选择/消息是否保持 |

执行原则：

- emulator 只做编译、安装和基本 E2E；性能结论来自至少一台 arm64 真机，最好再有一台中档或旧 WebView 设备；
- 每项至少预热后运行 10 次并报告 median/P95；语料、设备、Android API、WebView 版本、build profile 和温度状态固定；
- 先测基线，再决定 native fast path。Readest 源码注释中的“EPUB init 约 1.5s → 0.3s”只是其 iOS 个案，不是 Atha 目标值；
- MOBI/KF8 的硬门禁必须包含峰值内存，不能只看墙钟时间；`largeHeap` 不算通过。

具体阈值应由首个 Android EPUB 纵切在真实设备上建立基线后锁定，当前研究不伪造毫秒/RSS 数字。

## 未决风险与实施前问题

1. **设备矩阵：** 首批支持哪些 Android API、厂商 WebView、e-ink 设备？在没有目标设备前只能采用 Readest 的 `minSdk 26` 作为参考，不能当 Atha 已决定的门槛。
2. **Android SDK/NDK 现场：** 当前主机是否已安装可用 JDK、SDK、NDK、adb 和 arm64 真机尚未在本研究中执行探针；这是纵切 1 的第一项现场检查。
3. **dialog 版本行为：** Atha 锁定 `tauri-plugin-dialog 2.7.2`，但还没有在 Android 实机验证 Rust blocking picker 与新版 JS `copy` 模式的差异；应以官方 JS API 的 sandbox copy 做首个实验。
4. **规范化导入接口：** FB2/MOBI 的 foliate Book → Atha `ReaderManifest` adapter 需要小型 PoC，确认资源 URL、CSS 引用、字体、内部链接与确定性 content version 能完整落盘；这是采用 foliate parser 的 go/no-go 条件。
5. **许可：** Atha 已采用 `AGPL-3.0-or-later`；vendoring foliate-js 仍需保留 MIT 许可，Android 引入 LGPL `libmobi` 仍需解决源码与重新链接材料。研究和独立重写不等于获准复制 Readest 代码。
6. **DRM/真实语料：** 需要明确所有验收文件均为可合法测试的 DRM-free 文件，并补齐 EPUB2、FB2、CBZ、TXT、MD、MOBI/KF8 固定 corpus。
7. **Readest `main` 与稳定版差异：** `main` 仍标记为 `0.11.20`，但包含发布后提交；本报告的 Android Range、TXT/MD 与 native fast path 细节不能自动声称已在 `v0.11.20` 商店包中验证。
8. **内容安全：** foliate-js 官方警告同源 blob 与 scripted EPUB 风险，要求 CSP（[Security](https://github.com/readest/foliate-js/blob/f65836f77e8b66b84baacd54bfc92096578e7a84/README.md#security)）。任何新格式 adapter 都必须在进入 Atha 书根前清洗/验证，不能以“成熟库已解析”为信任依据。

## 主要一手来源

- [Readest README、功能与下载](https://github.com/readest/readest/blob/2b719600c27b4c9c91bef7b2bb148b3251338ea7/README.md)
- [Readest DocumentLoader 与格式 MIME/扩展](https://github.com/readest/readest/blob/2b719600c27b4c9c91bef7b2bb148b3251338ea7/apps/readest-app/src/libs/document.ts)
- [Readest 正常导入白名单](https://github.com/readest/readest/blob/2b719600c27b4c9c91bef7b2bb148b3251338ea7/apps/readest-app/src/services/constants.ts)
- [Readest Android 配置与文件关联](https://github.com/readest/readest/blob/2b719600c27b4c9c91bef7b2bb148b3251338ea7/apps/readest-app/src-tauri/tauri.conf.json)
- [Readest Rust EPUB 快路](https://github.com/readest/readest/blob/2b719600c27b4c9c91bef7b2bb148b3251338ea7/apps/readest-app/src-tauri/src/epub_parser.rs)
- [Readest Rust MOBI 快路](https://github.com/readest/readest/blob/2b719600c27b4c9c91bef7b2bb148b3251338ea7/apps/readest-app/src-tauri/src/mobi_parser.rs)
- [foliate-js 格式、稳定性、安全与性能说明](https://github.com/readest/foliate-js/blob/f65836f77e8b66b84baacd54bfc92096578e7a84/README.md)
- [foliate-js EPUB](https://github.com/readest/foliate-js/blob/f65836f77e8b66b84baacd54bfc92096578e7a84/epub.js)、[MOBI/KF8](https://github.com/readest/foliate-js/blob/f65836f77e8b66b84baacd54bfc92096578e7a84/mobi.js)、[FB2](https://github.com/readest/foliate-js/blob/f65836f77e8b66b84baacd54bfc92096578e7a84/fb2.js)、[CBZ](https://github.com/readest/foliate-js/blob/f65836f77e8b66b84baacd54bfc92096578e7a84/comic-book.js)
- [Tauri Android prerequisites](https://v2.tauri.app/start/prerequisites/)、[WebView versions](https://v2.tauri.app/reference/webview-versions/)、[dialog API](https://v2.tauri.app/reference/javascript/dialog/)、[mobile file associations](https://v2.tauri.app/learn/mobile-file-associations/)
- [W3C EPUB 3.3](https://www.w3.org/TR/epub-33/)、[Anansi ComicInfo](https://anansi-project.github.io/docs/comicinfo/intro)

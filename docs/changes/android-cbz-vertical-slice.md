# Android CBZ 图片序列纵切

## Status

implemented

## Problem

Atha 已在 Windows 与 Android 共用同一 EPUB ReaderManifest、BookRoot、Locator 和 reader runtime，但书架仍只接受 EPUB。Readest 已成熟支持 CBZ；Atha 当前缺少 CBZ 的确定性图片顺序、基础元数据、封面、像素内存边界、固定图片页，以及 Android 系统 picker、翻页、恢复和内存证据。直接迁移 foliate-js fixed-layout renderer 会复制现有阅读模型，也会弱化 Atha 已建立的 ZIP、资源和主动内容边界。

## Scope

- 在 `backend::reader` 下新增具体的 CBZ importer，把 ZIP 中受支持的 JPEG / PNG 页面归一为现有 schema 1 ReaderManifest；每张图片生成一个由 Atha 控制的 XHTML section，图片作为声明资源，不新增固定版式 schema；
- 第二个真实 ZIP 调用者出现后，将 EPUB 中与格式无关的 archive 打开、索引、重复 / 重叠 / 加密 / symlink / 路径 / 大小、读取、复制和 SHA-256 逻辑下沉为 crate-private 共享 module；EPUB mimetype 与引用解析仍留在 EPUB module；
- 共享 module 继续使用 `zip 8.6`；由于它没有 pre-allocation `max_entries` API，打开前以标准 terminal EOCD hint 拒绝超过 10000 项、trailing garbage 与歧义 terminal EOCD，打开后再校验实际条目数；fallback / ZIP64 最坏预分配保留为受 512 MiB 源文件上限约束的残余；
- 使用确定性的路径分段 ASCII 数字自然排序，忽略隐藏段、`__MACOSX` 和非图片成员；重复或 ASCII 大小写折叠后歧义的成员仍拒绝整书；
- 复用 `quick-xml 0.41` 有界读取可选 `ComicInfo.xml`，只消费当前书架实际使用的 `Title`、`Writer` 与唯一有效 `FrontCover`；无效、冲突或超限元数据回退，不影响有效图片；
- 新增唯一运行时依赖 `imagesize 0.15`，关闭默认 feature，只启用 `jpeg` / `png`，校验扩展名、魔数、非零尺寸、单边与像素预算；完整压缩流仍由现有 WebView `HTMLImageElement.decode()` 验证；
- 给生成页添加固定图片页 reader CSS；尾部损坏但通过头部探针的图片显示明确占位并允许继续跨 section 导航，不为图片页发明文字选区、OCR 或区域标注；
- 把 LocalLibrary 与 Tauri picker 从 EPUB-only 改为 EPUB / CBZ：桌面已知扩展严格分派，Android opaque content URI 的中性 cache 副本按严格 EPUB marker / container 识别，否则进入严格 CBZ；不以“EPUB 失败后再试 CBZ”兜底；
- 用 Rust 测试代码与 dev-only `png 0.18.1` 动态生成原创、确定性 CBZ 与恶意变体，不自写 PNG encoder，不提交漫画或大二进制；同一 fixture writer 生成 Windows / Android gate 制品并绑定 SHA-256；
- Windows 用现有 book-root / manifest 与 import verification 验证真实 WebView2 section、图片解码、跨页和坏页恢复；Android 正式入口覆盖系统 picker、首屏、全书翻页、坏页继续、强停恢复、日志隐私、16 KiB 对齐，以及可归因时的 app / WebView renderer PSS 记录。

## Non-Goals

- 不支持 CBR / RAR / 7z / PDF，不迁移 Readest UI、zip.js 或 foliate fixed-layout renderer；
- 第一片不承诺 GIF、WebP、BMP、SVG、JXL、AVIF；真实语料证明 JPEG / PNG 不足时再扩展 `imagesize` feature 与相同资源边界；
- 不实现 ComicBookInfo ZIP comment、Series、Manga / RTL、DoublePage、自动 spread、裁切或连续滚动；
- 不建立多格式 factory、codec registry、archive trait、metadata plugin 或第二套 Locator / Message schema；
- 不把 x86_64 16 KiB 模拟器瞬时 PSS 称为 ARM 真机峰值、长期内存门槛或发布性能证据。

## Architecture Impact

present

- Design purpose: 在具体 CBZ importer 内把不可信 ZIP 图片序列归一为现有深 ReaderManifest / BookRoot module，使 reader、Locator、书架和消息接口保持不变。
- Drivers / quality scenarios: `A-CBZ-01`（高业务重要性 / 中技术风险，负责人：reader importer）；Android 用户经系统 picker 选择 CBZ 后，应按确定性自然序看到一图一页、封面与基础元数据，并在翻页、进程强停和重开后恢复同一 section。`A-CBZ-SEC-01`（P0 内容安全，负责人：archive / CBZ importer）；恶意 ZIP、路径、歧义成员、伪装图片、像素炸弹或超量页面在写入受控书根前稳定拒绝。`A-CBZ-PERF-01`（高技术风险，负责人：reader / Android Adapter）；当前页解码不得随全书页数线性常驻，正式 gate 记录首屏、末页和恢复时内存与等待时间，renderer 崩溃、ANR 或持续线性增长即停止扩展。
- Modules / Interfaces / Seams / Adapters: `reader::archive` 是 EPUB / CBZ 两个具体 importer 的内部共享 seam；`reader::cbz::import_cbz` 是新的具体 interface。ReaderManifest、BookRoot、LocalLibrary public methods、Tauri commands、TypeScript DTO、Locator、reader session 和 MessageStore interfaces 不变。系统 picker 仍是唯一平台 Adapter。
- Candidate and tradeoffs: 采用现有 `zip 8.6` / `quick-xml`、零运行时传递依赖的 `imagesize` 与 dev-only `png`，拒绝自写 JPEG / PNG parser / encoder 和完整 Rust decoder；头部探针不能证明尾部完整，因此保留浏览器最终 decode 与可导航坏页。共享 archive module 只暴露两个 importer 已需要的具体函数，不接受 limits config 或格式注册；`zip 8.6` 没有打开前条目上限 API，terminal EOCD hint + post-open 10000 项是当前最小防线，不宣称消除 fallback / ZIP64 预分配残余。
- Evidence / ADR / review trigger: 一手研究见 `docs/research/cbz-format-assessment.md`；`imagesize` 精确许可、依赖、上限和回滚进入 ADR-0009。真实语料中非 JPEG / PNG、RTL 或双页达到必要比例，或 Android 内存无法形成稳定区间时，停止当前模型并以新 accepted change 重评。

## Acceptance Criteria

- [x] 动态原创 CBZ 的自然排序、隐藏 / 非图片筛选、ComicInfo title / writer / FrontCover、封面、sections / resources / TOC 与内容哈希缓存均正确；EPUB2 / EPUB3 行为不回归；
- [x] 空包、无受支持图片、损坏 header、扩展 / 魔数不符、零 / 超尺寸 / 超像素、超量页面，以及既有 ZIP 重叠、加密、symlink、重复、路径和解压预算攻击均返回稳定错误；
- [x] 生成 XHTML 一图一 section，关闭书源样式后仍保持一页；完整 decode 失败显示明确坏页并能继续到下一 section，搜索无伪文本且消息不产生伪 SourceAnchor；
- [x] Windows 正式 WebView2 入口通过至少前三个 section、自然翻页、坏页恢复、无外联与固定资源 MIME；
- [x] Android 16 KiB 正式入口完成干净数据系统 picker、导入、封面书架、首屏、逐页到末页、坏页继续、强停重启与末页 Locator 恢复，Picker cache 为空且日志不含路径、URI、标题、作者、成员名或图片内容；
- [x] Android 证据记录 fixture / APK / gate hash、WebView / 设备事实、导入 / 打开 / 首稳 / 翻页等待和 app PSS；可唯一归因 renderer 时同时记录 renderer PSS，无法可靠归因时显式记录而不伪造数值；无 fatal、renderer gone、OOM / LMK、ANR；
- [x] Rust fmt / Clippy / tests、Svelte / Tauri、Windows / Android gates、AutoCorrect、required docs gate 与独立 Spec / Standards review 通过，依赖与事实所有者同步。

## Files And Steps

1. 先以动态 fixture 为共享 archive、CBZ 正例 / 拒绝矩阵、自然排序、元数据和图像预算建立 red；
2. 下沉最小 archive seam，接入 `imagesize`，实现具体 CBZ importer 与 LocalLibrary 内容分派；
3. 接入 picker / UI、固定图片页 CSS 与可导航坏页，保持既有 manifest / reader interfaces；
4. 建立 Windows CBZ gate，再扩 Android gate 到真实 picker、翻页、恢复和内存证据；
5. 更新 ADR 与事实所有者，执行性能 / 安全复审、双轴 review、required gate、提交与 task closure。

## Checks

- `cargo test --locked -p atha-backend --test cbz_import`、workspace fmt / Clippy `-D warnings` 与既有 EPUB 测试；
- `scripts/check-cbz-source.ps1`；
- `scripts/check-tauri-reader.ps1` 与 Svelte check / build；
- `scripts/check-android-reader.ps1 -BookPath <generated.cbz> -CleanAppData -VerifyCbzFixture`；
- AutoCorrect、required docs gate、Spec / Standards review。

## Rollback

删除 CBZ importer、图片页规则与 picker 投影，恢复 LocalLibrary 的 EPUB-only 分派；ReaderManifest、书架记录、Locator、消息数据库和既有 EPUB 缓存 schema 均不迁移。`imagesize` 没有持久格式，回滚时删除直接依赖与锁记录即可。已导入的 CBZ 书根会成为未引用 cache，不改写用户消息事实。

## Approval

用户已明确批准按照路线图持续完成 Android 与 Readest 支持的非 PDF 格式，要求优先 Android、研究成熟库、少造轮子、补足日志并对性能敏感处 benchmark。本 change 是已批准路线图中 EPUB2 / NCX 后的下一最小切片。

## Result

动态原创 CBZ 已通过后端安全矩阵、Windows WebView2 与 Android 16 KiB 模拟器纵切。四页 gate 按自然序打开，第三页尾部损坏显示可访问占位且第四页继续，强停后恢复末页；现有 EPUB Tauri gate 与固定性能门槛同时通过。ReaderManifest、BookRoot、Locator、消息 schema 和 reader runtime 未分叉。

## Review

- Blocking: 独立后端复审发现自然排序非全序、CBZ 页面实际解压总量未按读取字节累计，以及 raw EOCD magic 会误拒合法正文；均已用根因修复与回归测试关闭。
- Blocking: 独立 Spec 复审发现 Windows gate 未真正覆盖跨节 / 坏页 / MIME，Android gate 未观察书架封面且隐私样本没有内容 token；已分别补入真实 WebView2 导航与资源断言，以及 ComicInfo / PNG 隐私探针、连续封面语义断言和结构化 `privacy` / `health` 证据。
- Blocking: 最终 Spec 与 Standards 复审均为 PASS，无新增安全、许可、日志、证据或过度工程问题。
- Non-blocking: `zip 8.6` 公开 API 无法在中央目录分配前设置条目上限；保留标准 terminal EOCD hint、post-open 10000 项检查和 512 MiB 源上限，并明确 fallback / ZIP64 残余，不引入无法闭合该风险的第二个 ZIP parser。
- Non-blocking: 写入成员与 CBZ 页面按实际读取量累计；container / OPF / navigation 等少量 metadata 读取仍只受 16 MiB 单成员上限约束，不把声明解压总量描述成所有读取的全局实际累计。
- Out-of-scope: ARM 真机峰值 / 长期内存、非 JPEG / PNG 页面、RTL / spread 与图片区域标注。

## Evidence And Residual Risks

- 本地证据：动态 CBZ importer / 恶意变体测试、workspace fmt / Clippy / Rust tests、Svelte check / build、Tauri tests 与 reader 坏页自检已通过；
- Windows 真实目标证据：`scripts/check-cbz-source.ps1` 在 23.1 秒内通过 SHA-256 为 `5957e1a0daed2ed0a3a8b1439585cb7651d5478fe5cd51cde0401c7878eb30ed` 的动态 fixture、导入形状与真实 WebView2 跨节 / 坏页 / `image/png` / 无外联 probe，gate SHA-256 为 `28fa5165ee4e7597210ccdc3f2a725d4ba021598e503add29c7e6aceb1a443c1`；`scripts/check-tauri-reader.ps1` 在 44.1 秒内通过既有 EPUB Tauri 回归，benchmark run `1786145446235-34040` 的冷启动 / 首稳 / 热开 / 翻页 / 重排 P95 分别为 925.190 / 189 / 21.1 / 7 / 41.5 毫秒，均低于固定门槛；
- Android 真实目标证据：API 35 x86_64、16 KiB 页面的 `Atha_API_35_16K` 在 58 秒内通过干净数据系统 picker、书架封面、四页导航、坏页占位、强停恢复、APK 对齐、健康和隐私检查。fixture SHA-256 为 `5957e1a0daed2ed0a3a8b1439585cb7651d5478fe5cd51cde0401c7878eb30ed`，APK SHA-256 为 `7c3c6f18c78e058e681396c6444a2509126edb38606f74767be0f25d6a8fe485`，gate SHA-256 为 `7342949d325eb75a5e4424242dde292a73c0c20f1d5b012f486bb999feb06d08`；app PSS 在书架 / 首屏 / 末页 / 恢复时为 133560 / 132683 / 131932 / 133428 KiB，未呈现随页数线性增长。WebView renderer 无法唯一归因，因此证据显式记录空值；
- `imagesize` 只验证类型头与尺寸，ZIP CRC 与 WebView decode 共同覆盖其余损坏，但不能把一次成功解码推广为所有平台 codec 保证；
- `zip 8.6` 的 terminal EOCD hint 拒绝 trailing garbage、歧义 terminal EOCD 和标准结尾中的超量条目，post-open 再校验 10000 项；它没有 pre-allocation `max_entries` API，fallback / ZIP64 最坏预分配仍是受 512 MiB 源文件上限约束的已知残余；
- x86_64 16 KiB AVD 可约束构建、系统 picker、WebView 功能和同环境 PSS，不替代 ARM 真机的 GPU / renderer 峰值、热稳定或发布包证据。

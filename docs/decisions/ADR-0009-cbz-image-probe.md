# ADR-0009：CBZ 图片头探针与受控书根

## 状态

accepted

## 日期

2026-08-08

## 背景

CBZ 是不可信 ZIP 图片序列。既有 EPUB importer 已有重叠、加密、符号链接、路径、成员数与解压量边界，但没有在浏览器解码前验证图片类型和像素预算。直接迁移 foliate-js fixed-layout renderer 会复制 Atha 的 ReaderManifest、BookRoot、Locator 与内容隔离；自行解析 JPEG / PNG 头又会增加不必要的媒体格式代码。

## 驱动因素与场景

- `A-CBZ-SEC-01`（P0 内容安全，负责人：reader importer）：扩展名伪装、损坏头、零尺寸、超大单边或像素炸弹必须在写入受控书根前拒绝；
- `A-CBZ-01`（高业务重要性 / 中技术风险，负责人：reader importer）：JPEG / PNG 页面按确定性自然序导入，一图一 section，并复用现有封面、目录、Locator 与恢复链路；
- `A-CBZ-PERF-01`（高技术风险，负责人：reader / Android Adapter）：导入一次只持有一个至多 16 MiB 的成员，reader 只解码当前 section；Android gate 记录 PSS，不把模拟器瞬时值当作 ARM 真机门槛。

## 决策

1. 把 EPUB 中格式无关的 ZIP 打开、索引、重复 / 重叠 / 加密 / symlink / 路径 / 大小、读取、复制和 SHA-256 逻辑下沉为 crate-private `reader::archive`；EPUB mimetype 与引用解析留在具体 EPUB module，不建立 archive trait、格式 factory 或 limits 配置层。
   `zip 8.6` 的 read `Config` 没有 pre-allocation `max_entries` API，而 `ZipArchive` 会按中央目录记录数预分配。Atha 在同一已打开文件句柄上先读标准 terminal EOCD hint，拒绝超过 10000 项、trailing garbage 与歧义 terminal EOCD，再构造 `ZipArchive` 并后置校验 `archive.len() <= 10000`。该 hint 不代替 ZIP parser，fallback / ZIP64 在 post-open 检查前的最坏预分配仍是残余风险，只受 512 MiB 源文件上限约束。
2. CBZ 首片只接收 JPEG / PNG。使用 `imagesize 0.15`，关闭默认 feature，只启用 `jpeg` / `png`；扩展名必须与探测类型一致，宽高必须非零且不超过 8192，乘法不得溢出，总像素不得超过 20000000。
3. `imagesize` 只读取类型头与尺寸，不声称验证完整压缩流。ZIP 读取继续校验 CRC；浏览器 `HTMLImageElement.decode()` 负责最终 codec 验证，尾部损坏显示受控坏页并允许继续导航。
4. CBZ importer 把每张图片归一为 Atha 生成的 XHTML section 与声明资源，沿用 schema 1 ReaderManifest。固定图片页结构样式属于 reader CSS，即使关闭书源样式也保持一图一页。
5. 可选 `ComicInfo.xml` 只消费书架已有字段 `Title`、`Writer` 与唯一有效 `FrontCover`；无效、冲突或超限元数据回退，不让有效图片书失败。RTL、spread、ComicBookInfo 与更多图片格式等真实语料证明必要时再评估。

## 候选与权衡

- 自写 JPEG / PNG header parser：拒绝。代码短期看似更少，但会自行维护格式分支和截断边界。
- `image` 完整 decoder：暂不采用。它会在导入期解码并分配像素，依赖与 CPU / 内存面明显大于当前“头部预算 + 浏览器最终解码”职责。
- Readest / foliate-js CBZ runtime：不采用。Atha 已有更深的 ReaderManifest / BookRoot / Locator module，只借鉴格式行为，不迁移第二套 reader。
- `imagesize`：采用。它只解决现有代码和标准库没有覆盖的类型 / 尺寸探针，运行时零传递依赖，删除成本低。测试 fixture 使用成熟 `png` encoder，不自写 PNG / CRC / Adler。

## 依赖评估

- 精确版本与许可：`imagesize 0.15.0`，MIT；`Cargo.lock` 是实际版本事实；
- feature 与体积：`default-features = false`，仅 `jpeg`、`png`，无运行时传递依赖、后台线程、网络、数据库或本地配置；
- 测试依赖：`png 0.18.1`（`MIT OR Apache-2.0`）只在 dev-dependencies 动态生成原创确定性 fixture，不进入产品运行时或 APK，与项目 AGPL-3.0-or-later 兼容；
- 数据与隐私：只读取当前 ZIP 成员字节，不保存图片内容、原路径或尺寸到日志 / 证据；
- 支持与升级：升级时复跑伪装头、截断头、尺寸预算与 Android PSS gate；feature 或解析行为扩大前重新检查许可和输入边界。

## 后果

- 正面：EPUB 与 CBZ 共享同一套 ZIP 信任边界，图片像素预算在浏览器分配前生效；
- 正面：reader、书架 DTO、Locator 和消息 schema 不变，没有多格式注册层；
- 负面：通过头探针但尾部损坏的图片只能在 WebView decode 时发现；
- 负面：`zip 8.6` 的 terminal EOCD hint 只预拦截标准结尾，fallback / ZIP64 可能在 post-open 条目上限生效前触发中央目录预分配；
- 负面：首片不支持 GIF、WebP、RTL、双页和图片区域标注。

## 风险与缓解

- 图片头声明合法但压缩流损坏：当前 section 显示固定可访问占位，不产生终止性的 `reader_failure`，导航仍可进入下一 section；
- 头部像素预算与 Android 实际 GPU / renderer 内存不一致：正式模拟器入口记录 app PSS，可唯一归因时记录 renderer PSS；ARM 真机数据出现后再校准，不先放宽上限；
- ComicInfo 恶意 XML：限制成员大小、XML 深度和字段长度，不加载 DOCTYPE / 实体 / 外部资源；解析失败只回退元数据；
- ZIP 条目预分配：标准 terminal EOCD 与打开后双层 10000 项检查，并拒绝 trailing garbage / 歧义 terminal EOCD；不将这个 hint 声称为 fallback / ZIP64 上的完整 pre-allocation 保证，上游提供 `max_entries` 时直接替换；
- 新格式需求诱发工厂：只有第三个 importer 出现且重复的分派代码已经可测量时，才评估更深 seam。

## 回滚与复查

删除 CBZ importer、reader 固定页规则、picker 投影、`imagesize` 运行时依赖和 `png` dev-dependency 即可；没有数据库或 Locator 迁移。真实 corpus 中 JPEG / PNG 覆盖不足、RTL / spread 达到产品必要比例、头探针放过可重复利用的 decoder 漏洞，或 Android 内存无法形成稳定区间时，停止扩展并以新 accepted change 复查完整 decoder 或成熟 fixed-layout engine。

## 实施与检查位置

- 共享 archive 与 CBZ importer：`backend/atha-backend/src/reader/`；
- 动态安全矩阵：`backend/atha-backend/tests/cbz_import.rs`；
- reader 固定页与最终解码：`reader/atha-reader.css`、`reader/web/content.mjs`；
- 平台检查：`scripts/check-cbz-source.ps1`、`scripts/check-android-reader.ps1`。

## 相关资料

- `imagesize 0.15.0` package metadata:<https://docs.rs/crate/imagesize/0.15.0>
- WHATWG `HTMLImageElement.decode()`：<https://html.spec.whatwg.org/multipage/embedded-content.html#dom-img-decode-dev>
- PKWARE APPNOTE:<https://pkware.cachefly.net/webdocs/casestudies/APPNOTE.TXT>
- ComicInfo schema:<https://github.com/anansi-project/comicinfo>

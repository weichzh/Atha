# 微信读书式离线书架界面

## Status

implemented

## Problem

Atha 已有可用的本地书架、三列封面、导入、单本移出、消息备份与恢复，但缺少日常使用所需的本地搜索、阅读状态视图和明确的批量选择流程；常驻删除按钮与分散的维护按钮也挤占移动端空间。用户提供的微信读书书架截图验证了搜索、紧凑封面网格、显式选择模式和底部批量动作的成熟交互。本 change 只借鉴这些离线交互，并参考 Readest v0.11.20 的成熟本地书架经验；Atha 继续使用现有 Svelte 壳、LocalLibrary、受控封面协议和阅读内核。

## Scope

- 在 `LibraryView.svelte` 内重排书架页：顶部提供本地搜索、视图切换、显式“导入”与“选择”入口；紧凑管理菜单保留现有消息备份和消息恢复能力；
- 搜索只在已加载的 `LibraryBook[]` 中匹配标题与作者，忽略首尾空白和大小写，空查询恢复当前视图的完整结果；不建立索引、历史或后端查询；
- 提供 `默认`、`未开始`、`在读`、`书名`、`作者` 五个视图，或用等价的状态筛选加排序控件表达相同能力；默认视图保持后端现有导入时间顺序，书名与作者视图只做本地稳定排序；
- 阅读状态只从书架页同源 `localStorage` 的现有 `atha.reader.progress.${book.id.slice(0, 16)}.v1` 读取。记录必须严格满足现有 schema 1、精确字段、大小、完整 64 位 `contentVersion`、Locator 结构及版本一致性，并与当前 `book.id` 相等；有效记录表示“在读”，缺失或无效记录表示“未开始”，读取过程不改写或清理存储；
- `localStorage` 不可访问或读取抛错时，不把全部书籍伪装成未开始；禁用 `未开始` / `在读` 筛选并给出简短说明，`默认` / `书名` / `作者` 与搜索仍可使用；不根据 Locator 猜百分比、读完或即将读完；
- “选择”进入独立模式，书籍点击只切换选中状态，不打开阅读；顶部显示已选数量，支持对当前搜索 / 视图结果全选、取消全选和退出选择，退出后清空选择；
- 底部批量栏只提供“移出书架”。确认一次后串行复用现有 `removeBook(id)`；已成功项立即移出，失败或未处理项保持可见和选中，并准确报告部分成功，不新增批量 command 或 DTO；
- 移除常驻封面删除按钮；选择一本后走同一批量移出路径。非选择模式点击封面或书目信息仍使用现有 `openBook`；
- 移动端保持紧凑三列封面，封面图片使用原生 lazy loading 与异步解码，失败继续显示现有本地占位；Android 暗色书架使用可见的浅色系统状态栏图标；平板和桌面只做响应式增列与合理最大宽度，不建立虚拟列表；
- 保留现有加载、空书架、导入失败、备份 / 恢复确认和状态反馈；交互控件维持可见焦点、语义标签、键盘操作、至少 44 CSS px 触控目标和 reduced-motion 行为；
- 产品设计 QA 使用用户提供的 `fixtures/local/weread/bookshelf-grid.jpg`、`bookshelf-selection.jpg`、`bookshelf-progress.jpg` 作参考，并把尺寸归一、截图和问题关闭记录写入仓库根 `design-qa.md`。

## Non-Goals

- 不做私密阅读、书城、在线内容、账户、云同步、分享、社交、有声书、推荐或更新；
- 不做分组、分类、置顶、书单、拖放、最近阅读或虚拟化；
- 不增加依赖，不迁移 Readest / 微信读书代码或完整视觉品牌；
- 不修改 LocalLibrary、导入缓存、消息数据库、Tauri command、DTO、backend 或阅读状态 schema；
- 不显示阅读百分比、“读完”或任何无法由当前严格进度记录证明的状态。

## Architecture Impact

present

- Design purpose: 书架固定为深色，但阅读器支持 system / light / paper / dark；Android edge-to-edge 系统栏需要知道当前页面背景明暗，不能再由 Activity 或系统模式全局猜测。
- Callers / syntax / control flow: `LibraryView.svelte` 挂载时可选调用全局 `AthaSystemBars.setDarkBackground(true)`；`preferences.mjs` 每次应用阅读主题，以及 `theme=system` 时 media query 变化后，调用同一同步、单向、无返回值方法。唯一参数是 boolean：`true` 表示背景深、使用浅色系统图标，`false` 表示背景浅、使用深色系统图标。
- Adapter / compatibility: Android `MainActivity.onWebViewCreate` 只在当前 WebView 注册该 bridge，并在 UI thread 用 `WindowCompat` 更新 status / navigation icon appearance。Windows、普通浏览器及缺少 bridge 的旧壳通过 optional call 保持 no-op；LocalLibrary、受控协议、ReaderManifest、Locator、MessageStore、阅读偏好 schema 与数据边界不变。
- Trust / failure / diagnostics: bridge 不接收字符串、路径、URI、书籍内容或用户数据，不返回能力，也不记录调用；书籍脚本和 iframe 仍由现有 sanitizer 拒绝。调用失败只保留平台原有系统栏样式，不阻塞书架、阅读或持久化。R8 用精确 keep rule 保留唯一反射方法。
- Evidence / rollback: debug APK 的五种实际主题帧、动态 system 切换与 minified release APK 覆盖 bridge；回滚只需删除 bridge、keep rule 与两处 optional call，即恢复纯 `enableEdgeToEdge()` 行为，不迁移任何数据。

## Acceptance Criteria

- [x] 标题 / 作者本地搜索可组合五个视图，空查询可复位；默认顺序不变，书名 / 作者排序稳定且未知作者有明确回退；
- [x] 缺失、超长、未知字段、错误 schema、错误 `contentVersion` 或非法 Locator 的进度记录均不进入“在读”；有效同书记录只区分“未开始 / 在读”，界面不出现百分比或读完状态；
- [x] 同源存储不可访问时，进度筛选可见但禁用并说明原因，其余视图、搜索、打开、导入和维护能力仍正常；
- [x] 显式选择模式支持单选、多选、当前结果全选 / 取消全选、退出清空和一次确认批量移出；选择时不会误开书，部分失败反馈与剩余选择一致；
- [x] 显式导入入口与管理菜单中的消息备份 / 恢复均保留；现有恢复替换确认、导入错误和空书架路径不回归；
- [x] 封面原生延迟加载，失败占位、标题、作者和选择状态均可访问；键盘、焦点、44 CSS px 触控目标及 reduced-motion 检查通过；
- [x] 360 × 800 与 412 × 915 保持可滚动三列、无横向溢出或文字遮挡；768 px 平板和桌面视口合理增列，搜索、视图栏、管理菜单与底部批量栏均可操作；
- [x] `scripts/check-library-shelf.ps1` 作为 Windows 正式 Tauri / WebView2 gate 覆盖搜索、筛选、选择、批量移出、菜单、控制台 / 网络错误和上述视口；
- [x] `scripts/check-android-reader.ps1` 在真实运行的 `Atha_API_35_16K` AVD 上以 opt-in 书架 UI 链路覆盖导入后网格、搜索、选择、移出、返回空态、触控和应用健康；模拟器证据不称为 ARM 真机或生产证据；
- [x] 根 `design-qa.md` 的 product-design QA 关闭 P0 / P1 / P2，Svelte check / build、最小逻辑测试、AutoCorrect、`git diff --check`、required docs gate 与独立 review 通过。

## Files And Steps

1. 在 `library.ts` 增加可独立测试的本地搜索、视图排序和严格进度判定，不修改 Tauri client interface；
2. 在 `LibraryView.svelte` 接入搜索、视图、管理菜单和显式选择状态，串行复用现有移出调用；
3. 在 `library.css` 完成紧凑移动三列、平板 / 桌面响应式布局、选择覆盖层、底部动作栏和可访问状态；
4. 用 `reader/app/tests/library.test.ts` 锁定搜索、排序、进度拒绝矩阵、存储失败与批量部分失败；
5. 以 Android boolean-only WebView bridge 同步系统栏图标，加入精确 R8 keep rule，并复验 debug 五主题与 minified release；
6. 扩展 Windows 与 Android 正式脚本，产出真实 Tauri / WebView2 和专用 AVD 证据；使用 agent-browser 完成探索与截图，最终以正式脚本为准；
7. 在根 `design-qa.md` 完成 product-design QA，再更新 `docs/architecture/READER-CORE.md`、`docs/codebase/MAP.md`、`docs/roadmap/ROADMAP.md` 与 `docs/ACTIVE.md`，执行检查和独立 review。

## Checks

- `node --test reader/app/tests/library.test.ts`；
- `pnpm --dir reader/app check` 与 `pnpm --dir reader/app build`；
- `pwsh -NoProfile -File scripts/check-library-shelf.ps1`；
- `pwsh -NoProfile -File scripts/check-android-reader.ps1 -BookPath <local-book> -CleanAppData -VerifyLibraryShelfUi`；
- AutoCorrect、`git diff --check`、required docs gate、product-design QA 与 Spec / Standards review。

## Rollback

恢复原有书架组件、样式、前端 helper、测试与 gate 即可；本 change 不迁移数据库、书架记录、导入缓存或阅读状态。已经通过现有 command 移出的书仍遵循当前语义：只删除书架记录，导入内容、消息和阅读进度保留，可重新导入恢复。

## Approval

用户已明确批准该离线书架范围，并要求参考微信读书截图与 Readest v0.11.20 的成熟交互，同时保留 Atha Svelte 和现有后端。本 change 状态为 `accepted`，可按上述边界实施。

## Result

离线书架已按用户截图收敛为黑色三列本地界面：标题 / 作者搜索、默认 / 进度 / 书名 / 作者视图、严格“未开始 / 在读”投影、显式选择、当前结果全选与串行批量移出均复用现有 `LibraryBook[]` 和单本 command。导入、消息备份 / 恢复、受控封面、ReaderManifest、Locator、MessageStore 和数据 schema 未改。Android 使用只接受布尔值的原生 bridge，让黑色书架与 system / light / paper / dark 阅读主题分别选择可见的系统栏图标，不引入依赖或第二套主题状态。

## Review

- Blocking: Spec 复审发现 Windows 门只有真实空壳、同键排序以内容 ID 重排，以及 Android 隐私门未覆盖书籍 ID 与轮转日志；已分别改成隔离数据根的真实本地 EPUB picker / 搜索 / 进度 / 选择 / 移出链路、保留原顺序的稳定排序，以及对 `Atha.log*` 与完整书籍 ID 的拒绝扫描。
- Blocking: Standards 复审发现固定深色 Android 系统栏会破坏浅色与纸张阅读主题；已恢复原生 edge-to-edge 自动基线，只增加 boolean-only bridge，并在阅读偏好应用及 system media-query 变化时同步图标明暗。五种实际主题帧复验通过。
- Non-blocking: Windows 的 JavaScript 确认框由门内 confirm spy 断言调用一次后继续验证真实 UI handler、Tauri command 与空书架；Android AVD 已覆盖真实系统确认框。单书 AVD 不冒充多书排序或大书架性能证据。
- Final: 独立 Spec / Standards 复审未发现剩余 P0 / P1 / P2 或安全、许可、隐私和过度工程阻塞。
- Out-of-scope: 私密阅读、在线 / 云 / 分享 / 社交 / 有声书、推荐 / 更新、分组 / 置顶、虚拟化、依赖与 backend / DTO 改造。

## Evidence And Residual Risks

- 本地证据：`node --test reader/app/tests/library.test.ts` 为 3 / 3；Svelte check 为 0 error / 0 warning，production build、PowerShell parser、AutoCorrect、`git diff --check` 与 required docs gate 均通过；
- Windows 真实目标证据：`scripts/check-library-shelf.ps1` 在隔离 `LOCALAPPDATA` 中使用一个 `fixtures/local` 真实 EPUB，通过原生 picker、标题 / 作者搜索、未开始进度分组、选择 / 取消、确认调用、真实批量移出、返回空态、无控制台错误，并另过 360 × 800、412 × 915、768 × 1024 与 1280 × 900 四视口；不记录本机路径、书名、作者、内容或 hash；
- Android 真实目标证据：API 35 x86_64、16 KiB 页面的 `Atha_API_35_16K` 完整构建与门在 138 秒内通过干净数据安装、系统 picker、导入 / 打开 / 重启、搜索、四视图、选择 / 全选 / 取消、真实确认、批量移出、触控、应用健康和 logcat + `Atha.log*` 隐私检查。最终 APK SHA-256 为 `64343c50360c25951f917f1cb1fb85cb989402d3cfdf2a008af2fbb8a6b43f57`，gate SHA-256 为 `c236a69b800a271ef914f9fdbc6cee5c48a6d083edf529cd8a93c6d694ccb230`；
- Android 发布构建证据：R8 mapping 保留 `SystemBarsBridge` 类与 `setDarkBackground(boolean)` 方法；签名后的 minified release APK SHA-256 为 `5a96dd0bcf86952616cb044cc24b4497afae0442b7865111f5e354ed979e5c40`，安装到同一 AVD 后书架与阅读页均通过，系统栏图标在两种背景上目检可见；
- Product Design 真实帧：三张参考图与默认 / 进度 / 选择实现帧已在同一归一化输入中复核；最终 APK 又实际切换 light、paper、dark、system-dark 与 system-light，五态系统栏图标均保持可见，根 `design-qa.md` 最终为 passed；
- 当前进度记录只能证明该书曾保存合法稳定位置，不能证明阅读比例或完成状态，因此界面有意只提供二态投影。Android 最高证据仍是单书 x86_64 模拟器，不覆盖多书排序、超大书架滚动或 ARM 真机内存；发布包只补充 R8 bridge 与书架 / 阅读页烟测，不替代 debug 完整功能门；
- 当前 AVD 的 WebView 124 在 16 KiB x86_64 模拟器存在 Chromium 已于 M125 修复的 `MemoryInfra` SIGTRAP 上游问题；本次最终门全绿，若旧 provider 重现同栈，先升级匹配的 WebView / Chrome / Trichrome 再归因 Atha。

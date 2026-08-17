# 外部文档入口

本文件只保存 Atha 直接依赖的外部边界、版本事实来源和最短用法，不复制官方文档。进入任务时完整读取；当任务涉及下列边界的行为、API 语义、兼容性、错误、安全或性能时，先打开对应官方入口再诊断或修改。

## Tauri 2

- 版本事实：`reader/app/package.json`、`reader/app/src-tauri/Cargo.toml` 与 `Cargo.lock`。
- 官方入口：[从前端调用 Rust](https://v2.tauri.app/develop/calling-rust/)、[Rust API](https://docs.rs/tauri/latest/tauri/)、[Webview 拖放 API](https://v2.tauri.app/reference/javascript/api/namespacewebview/#ondragdropevent)、[打包与文件关联配置](https://v2.tauri.app/reference/config/#bundleconfig)、[单实例插件](https://v2.tauri.app/plugin/single-instance/)。
- 项目快速用法：前端统一通过 `reader/app/src/messages.ts` 或对应领域 adapter 调用；Rust command 位于 `reader/app/src-tauri/src/lib.rs` 并在 `generate_handler!` 注册。可能阻塞的工作使用 async command；读取器 command 保留 `WebviewWindow` 来源校验并返回 `Result`。桌面文件拖放使用 Tauri 原生 Webview 事件并在组件卸载时取消监听；路径只交给有来源、数量和长度校验的 LocalLibrary command，不授予前端通用 fs scope。文件关联由平台打包配置声明，启动参数仍经过现有 importer；没有运行中实例转交需求时不增加 single-instance 依赖。
- 最短检查：`pnpm --dir reader/app check`、`pnpm --dir reader/app build`、`cargo test -p atha-reader-app`；桌面候选使用 `bash scripts/check-reader-candidate.sh`。
- 必须重查：command 参数映射、async 或主线程行为、state、window/webview、IPC 权限、拖放事件、平台文件关联、bundle metadata、插件 API 和运行中实例转交需求。

## Tauri Logging

- 版本事实：`reader/app/src-tauri/Cargo.toml` 与 `Cargo.lock` 中的 `tauri-plugin-log` / `log`。
- 官方入口：[Tauri Logging](https://v2.tauri.app/plugin/logging/)、[`tauri-plugin-log` Rust API](https://docs.rs/tauri-plugin-log/latest/tauri_plugin_log/)。
- 项目快速用法：产品 Rust 日志只使用 `atha::` target 和固定 operation / event、stage、code、耗时与计数；Info 以上写 stdout 与平台 AppLog，单文件 1 MiB，保留当前文件和最近两个归档。不得记录书名、路径、正文、笔记、查询、提示词或内容哈希。
- 最短检查：运行 `bash scripts/check-reader-linux.sh` 后检查平台 AppLog 同时包含启动、打开和 reader 固定阶段事件，并确认不含 fixture 路径或内容。
- 必须重查：插件 release line、Android 日志目录 / logcat 行为、target filter、轮转语义和敏感字段。

## Web 页面生命周期与单调计时

- 版本事实：阅读页由当前 Tauri WebView 承载；浏览器行为以目标 WebKitGTK / WebView2 实测为准。
- 官方入口：[Page Visibility API](https://developer.mozilla.org/en-US/docs/Web/API/Page_Visibility_API)、[`visibilitychange`](https://developer.mozilla.org/en-US/docs/Web/API/Document/visibilitychange_event)、[`pagehide`](https://developer.mozilla.org/en-US/docs/Web/API/Window/pagehide_event)、[High precision timing](https://developer.mozilla.org/en-US/docs/Web/API/Performance_API/High_precision_timing)、[Tauri window focus](https://v2.tauri.app/reference/javascript/api/namespacewindow/#onfocuschanged)。
- 项目快速用法：阅读时长只在稳定排版、文档可见、窗口聚焦且未闲置时使用 `performance.now()` 的短区间累计；`visibilitychange` 到 hidden 和 `blur` 立即暂停并提交，`pagehide` 只作补充。超过心跳上限的区间视为休眠或调度中断并丢弃，不能用 `Date.now()` 补时；墙钟只用于把已接受时长分配到本地日期。
- 最短检查：Node 状态机测试覆盖隐藏、失焦、闲置、长间隔与跨午夜，再由 Linux Tauri / WebKitGTK 真壳验证原生窗口失焦、恢复和重开。
- 必须重查：WebKitGTK / WebView2 的 visibility 与 focus 行为、移动端 activity suspend / resume、系统时区或墙钟跳变、多窗口并发以及任何跨设备同步语义。

## Tauri Android、SAF 与平台文件

- 版本事实：`reader/app/package.json`、`reader/app/src-tauri/Cargo.toml`、`Cargo.lock`、`reader/app/src-tauri/tauri.android.conf.json` 与 `reader/app/src-tauri/gen/android/`；本项目 Android 门槛固定 Node 24.1.0、JDK 21、NDK 28.2.13676358、compile / target SDK 36、min SDK 26，当前默认目标为 API 36 x86_64 16 KiB AVD。
- 官方入口：[Tauri Android 前置条件](https://v2.tauri.app/start/prerequisites/#android)、[Tauri Dialog](https://v2.tauri.app/plugin/dialog/)、[`FilePath` Rust API](https://docs.rs/tauri-plugin-dialog/latest/tauri_plugin_dialog/enum.FilePath.html)、[Tauri File System](https://v2.tauri.app/plugin/file-system/)、[`FsExt` Rust API](https://docs.rs/tauri-plugin-fs/latest/tauri_plugin_fs/trait.FsExt.html)、[Android 16 KiB page size](https://developer.android.com/guide/practices/page-sizes)、[Android Auto Backup](https://developer.android.com/identity/data/autobackup)。
- 项目快速用法：`tauri-plugin-dialog` 只选择文件；`platform_file::PickerInput` / `PickerOutput` 对普通路径直接转发，对 Android content URI 使用官方 fs plugin 流式复制到应用 `cache/Picker`。书籍 picker 先用 Tauri 内置 `app.path().file_name(content://...)` 经 PathPlugin / `ContentResolver` 取得显示文件名，只保留 `.epub` / `.cbz` / `.fb2` / `.fbz` / `.mobi` / `.azw` / `.azw3` / `.md` / `.markdown` / `.txt` 后缀；不得从 URI 字符串或正文猜格式。输入复用领域上限：单次至多 32 本书、archive / TXT 源文件至多 512 MiB、直接 FB2 至多 64 MiB、Kindle 源文件至多 256 MiB、Markdown 至多 16 MiB、恢复制品至多 8 GiB。临时目录独占创建并由 RAII 与启动清理回收，路径、URI、标题和内容不得进入日志。
- 最短检查：日常行为先运行 `bash scripts/check-reader-linux.sh`；PCT-AL10 候选运行 `bash scripts/check-pct-reader.sh build` 与 `verify`，获准更新时再以显式 serial 运行 `install --device <serial>`。旧 AVD PowerShell 门只保留历史证据，不作为当前 Linux / PCT 入口；真机功能、触摸和性能仍按活动 change 独立验收。
- 必须重查：Tauri mobile / plugin release line、Android Gradle Plugin 与 SDK / NDK 兼容、SAF provider 的 open / truncate / failure 语义、release 签名、实体 ARM 设备的 I/O / 内存 / WebView 性能，以及 API 31+ `dataExtractionRules` 与设备厂商的备份 / 迁移行为。

## PCT-AL10 真机与 ADB 取证

- 事实入口：Atha 真机任务只使用本节、当前 APK / manifest 和仓库脚本，不引用用户级 `android-cli` skill。主机入口是 Linux Bash；先用 `adb devices -l` 找到设备 serial，再通过 `adb -s "$serial"` 显式选中 `ro.product.model=PCT-AL10` 的设备。不得把型号当 serial，也不得默认选择列表中的第一台设备。`shell` 的命令参数由 Android 设备 shell 执行，但未放入远端引号的 `>` 仍先由主机 Bash 解析；设备侧重定向必须显式写成带引号的远端命令。应用 package、activity、版本和签名必须从当前 manifest、APK badging 或已安装 package 取得，不能猜测。
- 官方入口：[Android Debug Bridge](https://developer.android.com/tools/adb)、[`dumpsys`](https://developer.android.com/tools/dumpsys)、[UI Automator](https://developer.android.com/training/testing/other-components/ui-automator)、[慢帧与冻结帧](https://developer.android.com/topic/performance/vitals/render)、[Android 10 SurfaceFlinger latency 源码](https://android.googlesource.com/platform/frameworks/native/+/refs/heads/android10-release/services/surfaceflinger/SurfaceFlinger.cpp)、[AOSP FrameTracker 的 128 条环形记录](https://android.googlesource.com/platform/frameworks/native/+/60f3ab275ef3ddf3afcdfdce4eb09b59024fec51/services/surfaceflinger/FrameTracker.h)、[FrameTimeline 平台要求](https://android.googlesource.com/platform/external/perfetto/+/refs/heads/main/docs/data-sources/frametimeline.md)。
- 安装边界：安装、更新、清数据和卸载都是 PCT-AL10 上的真实写入，必须有当前任务对精确对象与影响的批准。`android install` 不是项目入口；不得预判 `adb install` 与直接调用 `PackageInstaller` / Binder session 的调用身份、参数和厂商行为等价，也不得把任一路径宣传为华为通用静默安装方案。调查安装行为必须针对当前 platform-tools、固件与 package manager 源码追踪实际调用链，并在真机记录 calling identity、Binder 方法、`SessionParams` / session flags、状态回调和确认界面后再下结论；厂商闭源分支只按运行时实测说明。任一次获授权 session 的成功或确认只归属于该次身份、参数与固件，不能外推另一条路径。确认界面出现或未出现都只是本轮观察，不预设只能交给用户处理，也不通过关闭安装验证、锁屏或其他全局安全设置来改变结果，不默认引入 Shizuku。获准安装后先记录现有 package / version / 签名，完成后用 `pm path`、`dumpsys package` 和设备可见版本复核；未经单独批准不得降级、卸载或清数据。
- 交互与截图：截图、UI XML、录屏和日志只写入仓库已忽略的 `artifacts/local/audits/<任务名>/`，拉取后删除设备侧临时文件；设备 serial 与这些产物可能包含个人信息、书名、路径、正文、查询或笔记，分享前必须检查并脱敏。`adb -s "$serial" shell input tap "$x" "$y"` 或 `input swipe` 只能在当前截图、UI hierarchy、`wm size` 与 `wm density` 复核后使用；不得复用其他视口的硬编码坐标，`input text` 只输入不含空格的公开 ASCII 或合成文本，其他输入交给真实键盘。ADB input 和 UI Automator 是自动化取证，不等同于自然手指触摸；截图也不证明交互链路，触摸仲裁与手势体验仍由用户在 PCT-AL10 上手动验收。
- 性能与日志：先记录设备型号、Android / WebView 版本、ABI、page size、视口、密度、应用 build / package 和精确场景。内存使用同一设备、build 和场景至少做三轮成对采样并报告中位数；Atha 应用 PSS 要同时纳入主进程和归属明确的 WebView sandbox renderer，负差值按采样噪声处理。日志同时检查 logcat 与项目 AppLog `Atha.log*`；不得输出私有书籍的标题、路径、正文、查询、笔记或内容哈希。
- 帧取证：Atha package 和 system WebView package 的 `dumpsys gfxinfo <package> framestats` 都可能报告 0 帧；这表示 WebView 合成路径未被该 package 统计覆盖，不能解释为 0 卡顿。每次启动或页面变化后重新列出 SurfaceFlinger layers，结合当前窗口、截图和受控页面更新动态选中真正可见且时间戳会增长的内容 layer，不复用旧 layer 名。每轮都对该 layer 单独执行 latency clear、一个有界场景和立即 dump；FrameTracker 只有 128 条环形记录，长场景会覆盖早期帧，因此拆成短轮次并报告有效记录数与截断。PCT-AL10 的 API 29 没有 Android 12 才提供的 FrameTimeline，SurfaceFlinger latency 只能作为显示层呈现时序，不能完整归因 app 与 compositor。录屏会引入额外负载，只用于视觉复核，不从视频帧率或时间轴生成数值门槛。
- 实时帧入口：Atha 在前台且阅读页已稳定时运行 `bash scripts/check-pct-reader-fps.sh --device <serial> --duration 10` 监视真实手指。实时值是可见内容 layer 的 SurfaceFlinger 呈现更新 cadence，不是完整 app FPS；没有新 buffer 的静止轮标为 idle。自动单次短划只用于启用 WebView devtools 的诊断包：先把当前进程的 devtools socket 转发到本地端口，再运行 `bash scripts/check-pct-reader-fps.sh --device <serial> --duration 2 --swipe forward --cdp-port <port>` 或 `backward`；它在结束后另存 gfxinfo app frame duration，并校验只翻一页、Locator 已变化且动画已收束。普通 release 只使用实时手指监视；ADB swipe 仍不等于自然手指验收。

```bash
pid="$(adb -s "$serial" shell pidof "$package" | tr -d '\r' | cut -d ' ' -f1)"
socket="$(adb -s "$serial" shell cat /proc/net/unix | awk -v name="webview_devtools_remote_${pid}" '$NF ~ name { sub(/^@/, "", $NF); print $NF; exit }')"
test -n "$socket"
adb -s "$serial" forward tcp:9222 "localabstract:$socket"
bash scripts/check-pct-reader-fps.sh --device "$serial" --duration 2 --swipe forward --cdp-port 9222
adb -s "$serial" forward --remove tcp:9222
```

- 证据口径：命令输出、截图和 UI hierarchy 分别说明它们实际证明的事实。Android 模拟器证据不等于 PCT-AL10，PCT-AL10 上的 debug APK 不等于已签名 release，安装或启动成功不等于功能、触摸或性能验收。真实目标记录必须标明设备、build、场景、轮次、时间和未覆盖项，但不得包含私有内容标识。
- 最短只读探针：以下命令不安装、不启动、不输入内容；`ANDROID_SERIAL` 必须由操作者从本次 `adb devices -l` 输出中显式设置。

```bash
adb devices -l
serial="${ANDROID_SERIAL:?set ANDROID_SERIAL to the PCT-AL10 serial}"
adb -s "$serial" shell getprop ro.product.model
adb -s "$serial" shell getprop ro.build.version.release
adb -s "$serial" shell getprop ro.build.version.sdk
adb -s "$serial" shell getprop ro.product.cpu.abi
adb -s "$serial" shell getconf PAGE_SIZE
adb -s "$serial" shell wm size
adb -s "$serial" shell wm density
adb -s "$serial" shell dumpsys webviewupdate
```

- 最短截图与层级取证：先为本次任务设置唯一的 `evidence_dir` 并确认该目录保持忽略；截图尽量通过 `exec-out` 避免设备侧副本。UI XML 缺少 WebView 节点不代表页面不存在，仍需与当前截图和人工观察交叉核对。

```bash
evidence_dir="artifacts/local/audits/pct-$(date +%Y%m%d-%H%M%S)"
mkdir -p "$evidence_dir"
git check-ignore "$evidence_dir"
adb -s "$serial" exec-out screencap -p > "$evidence_dir/screen.png"
adb -s "$serial" shell uiautomator dump /sdcard/atha-ui.xml
adb -s "$serial" pull /sdcard/atha-ui.xml "$evidence_dir/ui.xml"
adb -s "$serial" shell rm -f /sdcard/atha-ui.xml
```

- 最短性能取证：先从当前 manifest、APK 或设备 package 状态取得 `package`，并在每轮前恢复同一页面与静置条件；需要交互时只使用已批准的公开 / 合成 fixture。`dumpsys meminfo --package` 的总量不能自动归因给 Atha，必须按 PID / process 核对主进程与 renderer 归属。先保存 `SurfaceFlinger --list`，再通过当前可见状态与短探针为本轮设置精确 `ATHA_VISIBLE_LAYER`；如果不能唯一归属或 latency 没有有效时间戳，就明确记录覆盖缺口而不是挑选看起来更好的 layer。logcat 只按已核实的 PID 或固定项目 target 取证，不清空或导出全设备日志；项目 AppLog 只通过已有批准入口取得。

```bash
adb -s "$serial" shell dumpsys meminfo --package "$package"
adb -s "$serial" shell dumpsys gfxinfo "$package" framestats
round="${ATHA_PERF_ROUND:?set ATHA_PERF_ROUND to the current short round}"
adb -s "$serial" shell dumpsys SurfaceFlinger --list > "$evidence_dir/layers-$round.txt"
layer="${ATHA_VISIBLE_LAYER:?set ATHA_VISIBLE_LAYER to the verified visible layer}"
adb -s "$serial" shell dumpsys SurfaceFlinger --latency-clear "$layer"
# Run exactly one short approved scenario, then dump immediately.
adb -s "$serial" shell dumpsys SurfaceFlinger --latency "$layer" > "$evidence_dir/sf-latency-$round.txt"
pid="$(adb -s "$serial" shell pidof "$package" | tr -d '\r' | cut -d ' ' -f 1)"
test -n "$pid"
adb -s "$serial" logcat --pid="$pid" -d
```

- 必须重查：ADB 授权状态、唯一 serial、当前 package / activity / 签名、安装调用链与 session flags、华为安装确认、Android / WebView / 固件版本、可见 layer 与 latency 有效记录数、进程归属、证据目录忽略状态、私有内容脱敏和用户手动触摸验收。

## Linux GNOME 目标测试

- 运行事实：主机别名、用户目录和实时工具路径只保存在用户级 `$CODEX_HOME/HOSTS.md`；仓库不硬编码地址。GNOME user manager 当前应包含 `WAYLAND_DISPLAY=wayland-0`、`DISPLAY=:0` 与 `XDG_RUNTIME_DIR=/run/user/1000`。
- 官方入口：[Tauri WebDriver](https://v2.tauri.app/develop/tests/webdriver/)、[Tauri WebDriver 手动配置](https://v2.tauri.app/develop/tests/webdriver/manual-setup/)、[WebDriver Actions](https://www.w3.org/TR/webdriver2/#actions)、[Pointer Events 3 的兼容鼠标事件](https://www.w3.org/TR/pointerevents3/#compatibility-mapping-with-mouse-events)、[`WebviewWindowBuilder::use_https_scheme`](https://docs.rs/tauri/latest/tauri/webview/struct.WebviewWindowBuilder.html#method.use_https_scheme)。
- 项目快速用法：SSH 中的 CLI 构建直接运行；需要在远程 GNOME / RDP 桌面显示并在 SSH 断开后保留的 GUI 使用 `systemd-run --user --collect --unit=<唯一名> <program>`，例如 `systemd-run --user --collect --unit=gui-code code ~/Code/Atha`。不得用 `sudo` 启动 GUI，也不得依赖 SSH shell 临时导出的 Wayland 变量。
- 测试策略：日常格式与 GUI 开发直接使用 Linux Tauri / WebKitGTK；Android 模拟器只在发布前或移动端专项验收时启动，Windows 只在用户明确要求时使用。用户已批准通过安全通道把私有 `fixtures/local` 复制到目标仓库的忽略目录；样本仍只用于本地 opt-in，不提交、不写日志、不进入公开证据或分发包。
- 最短检查：`systemctl --user show-environment | rg '^(WAYLAND_DISPLAY|DISPLAY|XDG_RUNTIME_DIR)='`；安装一次 `cargo install tauri-driver --version 2.0.6 --locked` 后运行 `bash scripts/check-reader-linux.sh`。入口用唯一 transient unit 启动真实 Tauri 壳并隔离 XDG 应用数据；测试专用 `ATHA_READER_GUI_VIEWPORT` 只接受仓库声明的五个启动尺寸，避免把当前不生效的 WebDriver resize 当作多视口证据。WebKitGTK 2.52.5 的 touch Actions 当前会挂起，正式 Linux 门因此明确请求并记录可信 `mouse` PointerEvent，不能把它称为真机 touch。
- 必须重查：SSH 别名、GNOME 会话是否仍存活、systemd user 环境、KVM / emulator、Rust Android targets、JDK / SDK / NDK、Linux WebKitGTK / Tauri prerequisites 与私有样本是否已显式放置。

## Markdown / TXT

- 版本事实：`backend/atha-backend/Cargo.toml` 与 `Cargo.lock` 固定 `pulldown-cmark 0.13.4`、`chardetng 1.0.0`、`encoding_rs 0.8.35` 和 `regex 1.13.1`。
- 官方入口：[`pulldown-cmark` 0.13.4](https://docs.rs/pulldown-cmark/0.13.4/)、[`chardetng` 1.0.0](https://docs.rs/chardetng/1.0.0/)、[`encoding_rs` 0.8.35](https://docs.rs/encoding_rs/0.8.35/)、[`regex` 1.13.1](https://docs.rs/regex/1.13.1/)；已采纳的格式取舍与内容边界由 `docs/architecture/READER-CORE.md` 维护。
- 项目快速用法：`reader::text` 直接生成现有 ReaderManifest / BookRoot。Markdown raw HTML 转义，链接 / 图片只保留惰性文本；TXT 只在至少两个高置信整行标题时生成章节 TOC，并按约 1 MiB 合并物理 sections。相同字节的 Markdown / TXT 使用不同固定身份域；EPUB / CBZ 身份不变。
- 最短检查：`mise exec -- cargo test --locked -p atha-backend --test text_import`；恢复 Markdown / TXT GUI 路线前先把对应场景接入统一 Bash Linux runner，私有 TXT 只通过显式 opt-in 使用且不输出路径、标题、正文或哈希。
- 必须重查：parser / decoder release line、`encoding_rs` 的复合 SPDX、章节规则与真实语料、ARM64 Android 性能、Markdown 新增活动资源能力及 ReaderManifest section / TOC 上限。

## FB2 / FBZ、`quick-xml` 与 `base64`

- 版本事实：`backend/atha-backend/Cargo.toml` 与 `Cargo.lock` 固定 `quick-xml 0.41`（`encoding` feature）、`base64 0.22.1`、`zip 8.6` 和 `imagesize 0.15.0`。
- 官方入口：[`quick-xml` 0.41](https://docs.rs/quick-xml/0.41.0/quick_xml/)、[`base64` 0.22.1](https://docs.rs/base64/0.22.1/base64/)、[`zip` 8.6](https://docs.rs/zip/8.6.0/zip/)。
- 项目快速用法：`reader::fb2` 对直接 FB2 或单根成员 FBZ 做两遍有界流式解析，只投影受支持正文、目录、内部链接和 JPEG / PNG binary，不解释源 stylesheet，不加载外部资源。同一 XML 的 FB2 / FBZ 共享固定格式域身份；picker 只按后缀分派，不嗅探正文。
- 最短检查：`mise exec -- cargo test --locked -p atha-backend --test fb2_import`；Linux 真实 GUI 使用 `bash scripts/check-reader-linux.sh`。
- 必须重查：FB2 schema / encoding 语料、未知元素策略、binary 图片类型、FBZ 单成员大小、依赖许可证和 Android ARM 真机内存。

## MOBI / AZW / AZW3、`boko`

- 版本事实：`backend/atha-backend/Cargo.toml` 与 `Cargo.lock` 固定 `boko 0.5.0`，关闭默认 features；`imagesize 0.15.0` 同时启用 JPEG / PNG / GIF。
- 官方入口：[`boko 0.5.0`](https://docs.rs/boko/0.5.0/boko/)、[固定源码](https://github.com/zacharydenton/boko/tree/8f412fb1a507399bce320d591feb517467cdb5f7)。
- 项目快速用法：`reader::kindle` 先独立检查 PDB record、MOBI version、compression、encoding、encryption、词典索引及正文 / HUFF 预算，再按真实 header 将纯 KF8 交给 `Format::Azw3`、其余交给 `Format::Mobi`。`boko` 只恢复只读正文、目录与图片，结果仍经 Atha XHTML / CSS / 资源白名单、唯一 TOC、ReaderManifest / BookRoot 和原子发布；同字节跨 `.mobi` / `.azw` / `.azw3` 共享 `atha/kindle/boko-0.5.0-importer-v1` 身份域。当前丢弃 `boko` 公共 raw API 无法加载的 KF8 flow stylesheet，不发布悬空 CSS 引用。
- 最短检查：`mise exec -- cargo test --locked -p atha-backend --test kindle_import`；继续 Kindle 私有样本、release benchmark 或 Linux GUI 路线前，先迁入对应 Bash 入口，不再运行旧 PowerShell 门。
- 必须重查：`boko` release line / 公共 CSS flow API、HUFF 全书预算、combo / MOBI7 / Windows-1252 / 压缩字体真实语料、源样式保真、Android ARM64 内存与许可证分发材料。

## MDict / Kindle 离线词典

- 版本事实：`backend/atha-backend/Cargo.toml` 与 `Cargo.lock` 固定 `mdict-rs 0.1.4` 并启用 `lzo`；经典 Kindle 词典只在 `reader::dictionary` 内实现 MOBI6、CP1252、HUFF/CDIC 与正排 INDX 的有界按需读取。
- 官方入口：[`mdict-rs 0.1.4`](https://docs.rs/mdict-rs/0.1.4/mdict_rs/)、[固定源码](https://github.com/Initsnow/mdict-rs/tree/d4bc67d1128e9561a27b714f085ad970dfed6c09)、[`libmobi 0.12` 固定参考源码](https://github.com/bfabiszewski/libmobi/tree/85dcfe803fc2a21020ddcf15c3eb66b93d388add)。
- 项目快速用法：`LocalDictionaries` 在应用数据根的 `Dictionaries` 目录事务导入 MDX / MDD 或经典 MOBI6 词典，按固定格式域去重，只做精确查询；MDX 链接深度有限，MDD 先解析范围与大小再流式读取。最终释义同时投影兼容纯文本和固定元素白名单富文本；来源样式、属性、地址与资源不进入结果，排版由应用 CSS 提供。Tauri command 和日志不接收或记录源路径、查询、词头、释义或资源内容。
- 最短检查：`bash scripts/check-dictionary-source.sh`；私有真实英文输出与 release benchmark 增加 `--private-fixtures fixtures/local`。Linux GUI 与 PCT 原生专项在实际续做前接入同一 Bash 入口；原生结果不能冒充应用 PSS。
- 必须重查：`mdict-rs` release line / AGPL-3.0-only 分发、MDict v1 / 加密变体、Kindle ORDT / keys / names 屈折索引、富 MDD 资源、ARM64 真机性能和任何模糊或跨词典查询需求。

## Rust 文件锁与 `fs2`

- 版本事实：`rust-toolchain.toml` 固定 Rust 1.97.1，`backend/atha-backend/Cargo.toml` 与 `Cargo.lock` 固定 `fs2 = 0.4.3`。
- 官方入口：[Rust 1.97.1 Unix `std::fs` 源码](https://github.com/rust-lang/rust/blob/1.97.1/library/std/src/sys/fs/unix.rs#L1353-L1550)、[`fs2::FileExt`](https://docs.rs/fs2/0.4.3/fs2/trait.FileExt.html)、[`fs2` 0.4.3 package metadata](https://docs.rs/crate/fs2/0.4.3/source/Cargo.toml.orig)。
- 项目快速用法：Rust 1.97.1 的 Unix 文件锁实现没有把 Android 列入 `flock` 支持集合，Android 会得到 `Unsupported`；MessageStore 维护锁只在 `store.rs` 与 `backup.rs` 通过 `fs2::FileExt` 使用 shared / exclusive try-lock。`fs2` 0.4.3 的许可为 `MIT/Apache-2.0`，不扩展为通用锁 abstraction。
- 最短检查：`cargo test -p atha-backend`，再在 Android 冷启动日志中确认 MessageStore setup 不再返回 `message-database`。
- 必须重查：Rust 升级后标准库是否原生支持 Android、网络文件系统 / 厂商内核的 advisory lock 语义、维护锁协议变化与依赖许可变化；标准库实测覆盖 Android 前不移除 `fs2`。

## 项目与依赖许可证

- 版本事实：根 `LICENSE`、根 / member Cargo manifest、独立 P0 manifest 与 `reader/app/package.json` 中的精确 SPDX。
- 官方入口：[GNU AGPL v3](https://www.gnu.org/licenses/agpl-3.0.html)、[SPDX `AGPL-3.0-or-later`](https://spdx.org/licenses/AGPL-3.0-or-later.html)、[Cargo license 字段](https://doc.rust-lang.org/cargo/reference/manifest.html#the-license-and-license-file-fields)、[Cargo workspace package 继承](https://doc.rust-lang.org/cargo/reference/workspaces.html#the-package-table)、[npm license 字段](https://docs.npmjs.com/cli/configuring-npm/package-json/#license)。
- 项目快速用法：Atha 第一方代码使用 `AGPL-3.0-or-later`；第三方代码与资产保留各自许可。依赖接入必须核对精确 `-only` / `-or-later`、链接与分发方式以及适用的源码、NOTICE、修改说明或重新链接义务。
- 最短检查：Cargo / npm manifest 解析、根许可证与 GNU 官方纯文本哈希一致、三个锁文件无 diff、required `docs` gate。
- 必须重查：首次正式分发、双许可 / CLA、CSS 社区独立仓库、`AGPL-3.0-only` 依赖、LGPL Android 链接以及任何可再分发性不明的字体、书籍、词典或 fixture。

## Svelte 5

- 版本事实：`reader/app/package.json` 与 `reader/app/pnpm-lock.yaml`。
- 官方入口：[Svelte 文档](https://svelte.dev/docs/svelte/overview)。
- 项目快速用法：应用入口位于 `reader/app/src/main.ts`，阅读器外壳位于 `reader/app/src/components/ReaderChrome.svelte`；优先复用现有组件和原生 Svelte 状态，不增加第二套 UI 状态容器。
- 最短检查：`pnpm --dir reader/app check` 与 `pnpm --dir reader/app build`。
- 必须重查：runes、生命周期、事件、组件绑定、编译器警告和升级兼容性。

## CodeMirror 6

- 版本事实：`reader/app/package.json` 与 `reader/app/pnpm-lock.yaml` 固定 `codemirror 6.0.2`、`@codemirror/lang-css 6.3.1`、`@codemirror/language 6.12.4` 和 `@codemirror/lint 6.9.7`。
- 官方入口：[参考手册](https://codemirror.net/docs/ref/)、[基础编辑器](https://codemirror.net/examples/basic/)、[自动补全](https://codemirror.net/examples/autocompletion/)、[Lint](https://codemirror.net/examples/lint/)。
- 项目快速用法：`CssEditor.svelte` 只在 CSS 模块页可见时按需加载 `basicSetup`、CSS language 和基于 syntax tree 的诊断；隐藏 textarea 仍是 `preferences.mjs` 的唯一状态入口。CodeMirror 只负责编辑体验，最终 CSS 安全与有效性仍由 `content.setStyles()` 的 CSSOM 边界判定。
- 最短检查：`mise exec -- pnpm --dir reader/app check`、`mise exec -- pnpm --dir reader/app build`；继续 CSS GUI 验收前把输入、回退与恢复场景接入统一 Bash Linux runner。
- 必须重查：升级后的包拆分、按需 chunk、WebKitGTK 输入法与无障碍、syntax tree 诊断、编辑器销毁和初始 bundle 变化。

## 微信读书真机界面证据

- 本地事实：PCT-AL10 原始截图、补充截图、逐图观察和 SHA-256 清单位于忽略目录 `fixtures/local/weread/`；Windows 目标副本位于 `E:\Code\Atha\fixtures\local\weread`。
- 项目快速用法：设计结论必须引用本地 `README.md` 的 `WR-*` 编号，并先打开对应原图复核；子 agent 摘要、文字转述或缺图报告不能替代原图证据。证据目录缺失时只报告缺失，不从旧结论重建界面事实。
- 最短检查：在证据目录运行 `sha256sum -c SHA256SUMS`；跨主机使用 PowerShell `Get-FileHash` 复核，不输出书架中的个人内容。
- 必须重查：新增截图的设备、分辨率、原始文件名、哈希、界面状态和与既有结论的对应关系。

## Readest 界面与交互证据

- 固定界面源码：[`readest/readest` 提交 `cf413b2b`](https://github.com/readest/readest/tree/cf413b2b9f1a205732062bf656e73c702f12ac02)；固定交互源码：Readest v0.11.20 [`useCapturedTurn.ts`](https://github.com/readest/readest/blob/1df1505fc5033fc949463c9908f2d53bd0fbdfa6/apps/readest-app/src/app/reader/hooks/useCapturedTurn.ts)、[`useLongPress.ts`](https://github.com/readest/readest/blob/1df1505fc5033fc949463c9908f2d53bd0fbdfa6/apps/readest-app/src/hooks/useLongPress.ts)、[`BookshelfItem.tsx`](https://github.com/readest/readest/blob/1df1505fc5033fc949463c9908f2d53bd0fbdfa6/apps/readest-app/src/app/library/components/BookshelfItem.tsx)、[`ImageViewer.tsx`](https://github.com/readest/readest/blob/1df1505fc5033fc949463c9908f2d53bd0fbdfa6/apps/readest-app/src/app/reader/components/ImageViewer.tsx) 与对应 foliate-js [`paginator.js`](https://github.com/readest/foliate-js/blob/dd71f2be356563c16a23272686189fcfb45d0b82/paginator.js)。当前公开 Web 入口为 [web.readest.com](https://web.readest.com/)，阅读与自定义能力以 [Reading](https://readest.com/docs/reading) 和 [Customization](https://readest.com/docs/customization) 为准。
- 本地事实：固定提交内的 Android、macOS、桌面原图，2026-08-08 的公开 Web 实际截图、逐图观察和 SHA-256 清单位于忽略目录 `fixtures/local/readest/`。
- 项目快速用法：设计结论必须引用本地 `README.md` 的 `RD-*` 编号并打开原图；Atha 采用 Readest 的安静图标工具、正文与侧栏并存、主题预览和设置渐进披露。交互只借鉴其小阈值后拖动优先于点击和分页收束，不复制 React / Tailwind、截图覆盖动画或 foliate 完整分页架构；表格边界必须按 Atha 的 owner 规则移交。
- 最短检查：在证据目录运行 `sha256sum -c SHA256SUMS`；需要刷新 Web 界面时通过 `agent-browser` 重新采集带日期的截图，默认分支变化后使用新提交建立新快照，不覆盖既有证据。
- 必须重查：默认分支提交、公开 Web 版本、商店截图与实际运行版本差异、屏幕尺寸、深浅主题、输入方式和与 Atha 当前设计结论的对应关系。

## Web 可视视口与触控输入

- 官方入口：[MDN Visual Viewport API](https://developer.mozilla.org/en-US/docs/Web/API/Visual_Viewport_API)、[MDN Touch events](https://developer.mozilla.org/en-US/docs/Web/API/Touch_events/Using_Touch_Events)、[MDN wheel event](https://developer.mozilla.org/en-US/docs/Web/API/Element/wheel_event)。
- 项目快速用法：软键盘适配只在 `window.visualViewport` 可用时监听 `resize` / `scroll` 并把可视高度与顶部偏移投影为消息层变量；不可用时保留既有 `dvh` 布局。预览缩放只在图片、公式或表格 dialog 内拦截双指 `touchmove` 和桌面 `wheel`，监听器必须显式 `passive: false`，单指滚动与阅读正文输入不受影响。
- 最短检查：Svelte check / build 后在 Linux WebKitGTK 运行 `scripts/check-reader-linux.sh`；Android 软键盘遮挡仍需 PCT-AL10 或等价系统 WebView 真机验证。
- 必须重查：VisualViewport 在目标 WebView 的 offset、缩放、键盘 resize / overlay 行为，touch cancellation、桌面触控板 wheel 粒度，以及输入法候选栏和横屏安全区。

## Tiptap 3

- 版本事实：`reader/app/package.json` 与 `reader/app/pnpm-lock.yaml`。
- 官方入口：[Svelte 集成](https://tiptap.dev/docs/editor/getting-started/install/svelte)、[StarterKit](https://tiptap.dev/docs/editor/extensions/functionality/starterkit)、[JSON 持久化](https://tiptap.dev/docs/guides/output-json-html)、[ProseMirror Markdown 示例](https://prosemirror.net/examples/markdown/)、[ProseMirror API](https://prosemirror.net/docs/ref/#markdown)。
- 项目快速用法：消息输入器只启用 StarterKit 中已进入 Atha 消息正文契约的文字节点与标记；持久化始终使用 Tiptap JSON，`plainText` 只是搜索与无格式回退投影。原始 Markdown 是同一 JSON 文档的临时输入视图，使用稳定的 `prosemirror-markdown` 双向转换；官方 `@tiptap/markdown` 当前仍是 Beta，不进入产品路径。编辑器与 Markdown 转换分别按需加载。
- 最短检查：`pnpm --dir reader/app test:markdown`、`pnpm --dir reader/app check` 与 `pnpm --dir reader/app build`。
- 必须重查：扩展 schema、粘贴内容、链接协议、JSON 兼容性、编辑器生命周期和只读呈现。

## WebView2

- 版本事实：Windows 目标机的 Evergreen WebView2 Runtime；产品入口由 Tauri 2 承载，`reader/atha-reader-host` 只保留为性能和安全基线。
- 官方入口：[WebView2 文档](https://learn.microsoft.com/en-us/microsoft-edge/webview2/)。
- 项目快速用法：书籍内容保持脚本、网络、路径和未知资源隔离；真实交互回归使用 `scripts/check-reader-samples.ps1`，Tauri 产品入口使用 `scripts/check-tauri-reader.ps1`。
- 必须重查：runtime 分发、浏览器能力差异、窗口缩放、输入事件、进程模型、安全边界和性能诊断。

## CSS 渲染与资源边界

- 版本事实：`reader/atha-reader.css`、`reader/web/content.mjs` 与 `backend/atha-backend/src/messages/model.rs`。
- 官方入口：[CSS Values 4](https://www.w3.org/TR/css-values-4/)、[CSS Images 4](https://www.w3.org/TR/css-images-4/)、[CSS Syntax 3](https://www.w3.org/TR/css-syntax-3/)、[CSS Containment 3](https://www.w3.org/TR/css-contain-3/)、[CSS Sizing 4](https://www.w3.org/TR/css-sizing-4/)。
- 项目快速用法：书籍和快照 CSS 不允许发起资源请求；`url()`、`src()`、`image()`、`image-set()`、`@import`、反斜杠转义和 Shadow DOM 穿透选择器在写入与显示两端均拒绝。v5 图片的私有样式在书源 CSS 前为最多 512 个唯一尺寸对生成零特异性的 `contain:size` / `contain-intrinsic-size` 规则；快照在 1 MiB 总预算内复用同一生成器，并保留前置 `@namespace` 顺序。
- 必须重查：新增 CSS 资源函数、转义语法、CSSOM 序列化、Shadow DOM selector 和浏览器实现变化。

## EPUB 3.3

- 版本事实：`backend/atha-backend/src/reader/epub/` 的导入实现与测试样本；`Cargo.toml` 与 `Cargo.lock` 固定 `imagesize 0.15.0` 和 `kamadak-exif 0.6.1`。
- 官方入口：[EPUB 3.3](https://www.w3.org/TR/epub-33/)、[`kamadak-exif 0.6.1`](https://docs.rs/kamadak-exif/0.6.1/exif/)。
- 项目快速用法：导入与不可信内容处理留在 backend；v5 缓存给同时缺少显式宽高的本地图片补充有界的原生 HTML 宽高，阅读器以前置、零特异性的 CSS 固有尺寸让作者和用户规则继续覆盖并在解码前稳定宽高比，不增加等待资源后揭示。EXIF 方向 5–8 交换尺寸；损坏 EXIF 与 IDAT 后的 PNG `eXIf` 只跳过提示，已经读到 SOF 的无 EXIF JPEG 不因后续扫描尾缺失而失去提示。v2 至 v5 完整缓存继续可读，有耐久源时 v2 至 v4 按需升级。正式行为由 `backend/atha-backend/tests/epub_import.rs` 和阅读器 Linux GUI 样本检查覆盖。
- 必须重查：容器、OPF、spine、资源、导航、媒体类型、URL 解析、安全要求、图片方向与作者 CSS 尺寸语义。

## CBZ、ComicInfo 与 `imagesize`

- 版本事实：`backend/atha-backend/Cargo.toml` 与 `Cargo.lock` 中的 `zip 8.6`、`quick-xml 0.41`、`imagesize 0.15.0`。
- 官方入口：[`imagesize 0.15.0`](https://docs.rs/crate/imagesize/0.15.0)、[`ImageType`](https://docs.rs/imagesize/0.15.0/imagesize/enum.ImageType.html)、[`blob_size`](https://docs.rs/imagesize/0.15.0/imagesize/fn.blob_size.html)、[ComicInfo schema](https://github.com/anansi-project/comicinfo)、[PKWARE APPNOTE](https://pkware.cachefly.net/webdocs/casestudies/APPNOTE.TXT)、[`HTMLImageElement.decode()`](https://html.spec.whatwg.org/multipage/embedded-content.html#dom-img-decode-dev)。
- 项目快速用法：`reader::archive` 为 EPUB / CBZ 共享 crate-private ZIP 信任边界；`reader::cbz` 只接受 JPEG / PNG，以 `imagesize` 校验魔数、非零尺寸、8192 单边和 20000000 像素预算，再生成 schema 1 ReaderManifest 与一图一 XHTML section。`imagesize` 不验证完整压缩流，最终损坏由 WebView `img.decode()` 显示受控坏页。ComicInfo 只消费有界的 Title、Writer 与唯一有效 FrontCover。
- 最短检查：`mise exec -- cargo test --locked -p atha-backend --test cbz_import`；继续 CBZ GUI / PCT 路线前把系统 picker、逐页阅读、坏页、恢复和 PSS 场景迁入 Bash 入口。
- 必须重查：新增图片格式、RTL / spread、ComicInfo 字段、ZIP feature、`imagesize` 类型 / 尺寸行为、完整 decoder 需求、Android GPU / renderer 内存与 ARM 真机门槛。

## SQLite 与 FTS5

- 版本事实：`backend/atha-backend/Cargo.toml`、`Cargo.lock` 与 `docs/codebase/DATABASE.md`。
- 官方入口：[SQLite](https://www.sqlite.org/docs.html)、[FTS5](https://www.sqlite.org/fts5.html)。
- 项目快速用法：持久化与迁移统一位于 `backend/atha-backend/src/`；消息检索不得在前端复制数据库事实。
- 最短检查：`cargo test -p atha-backend`。
- 必须重查：事务、迁移、约束、FTS 查询、排序、备份和并发语义。

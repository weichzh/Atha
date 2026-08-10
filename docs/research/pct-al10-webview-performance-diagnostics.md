---
description: 研究 PCT-AL10 Android 10 WebView 114 的触摸到呈现诊断、基准与候选优化，明确 API29 证据边界。
---

# PCT-AL10 WebView 触摸与渲染性能诊断研究

## 后续状态

本文是产品改动前的诊断设计。其 SurfaceFlinger 分层、API 29 无 FrameTimeline 和 `gfxinfo` 边界仍有效；Atha 随后已经采用 300ms 收束、20,000px 长章原生滚动回退，并落地 `scripts/check-pct-reader-fps.sh`。下文 150ms 和待证伪候选只代表研究时快照，当前实现与实测以 `docs/changes/reader-gesture-performance.md` 和 `docs/codebase/MAP.md` 为准。

## 结论先行

当前最重要的结论不是“继续改动画”，而是先把测量对象从宿主 `View` 修正为 WebView renderer、Chromium compositor 和 SurfaceFlinger：

1. `dumpsys gfxinfo com.atha.reader` 得到 0 帧，**不能证明 WebView 没有呈现，也不能证明没有卡顿**。Android 官方把这套帧统计限定在 `View` / Canvas 渲染管线；稳定后的 WebView 内容可由独立 renderer、GPU/compositor 和已有宿主 surface 更新，宿主窗口未必产生可归因的 `gfxinfo` 帧。[Android slow rendering](https://developer.android.com/topic/performance/vitals/render)、[WebView 架构（Chromium 114 固定提交）](https://github.com/chromium/chromium/blob/bdabf09e0e69fd4ba0be83dc8269b2ebf99a9c94/android_webview/docs/architecture.md)。
2. PCT-AL10 是 Android 10 / API 29，**没有 Android 12 才加入的 FrameTimeline**。本机可用的最高层级证据是：WebView/CDP 或 `TracingController` 的主线程、raster 和 compositor trace，加上 SurfaceFlinger layer 的 actual-present 时间；两者只能建立受控单手势下的关联，不能伪装成系统提供的端到端因果链。[Perfetto FrameTimeline](https://android.googlesource.com/platform/external/perfetto/+/refs/heads/main/docs/data-sources/frametimeline.md)。
3. Atha 当前热路径已经有正确基础：起手缓存几何、move 合并为一个 rAF、只写 transform、公式图片显式尺寸并 `decode()`。首要待证伪假设反而是 **DPR 物理像素布局 + 逆向缩放、长 CSS columns 整章 layer、恒等 `brightness(1)` filter** 的组合是否放大 tile/raster/compositor 成本；不能先加永久 `translateZ(0)` 或 `will-change`。
4. 第一阶段无需改 APK：确认真实 WebView provider、build、刷新率和温度；用 debug 包连接 WebView DevTools；为一个手势分别采集短 Perfetto、CDP Performance/Layers 和动态 SurfaceFlinger layer latency。第二阶段才建立专用测试 APK：debug 诊断构建与非 debuggable、`profileable` 的 release-like benchmark 构建分开。
5. 真机门应按每个 fixture、方向和手势类型分别做 **5 次预热 + 20 次测量**，每个 measured round 只做一次手势。保留 20 条原始记录，报告 median、nearest-rank P95（排序后第 19 条）、max 和语义失败数；不同场景、刷新率、build 不合并。

因此，本报告不主张立即改产品代码。它给出两个可执行层级：当前设备上的无代码诊断，以及需要新 accepted change 才实施的测试 APK instrumentation。

## 范围和证据状态

本轮只读研究仓库、Android/AOSP/Chromium/Perfetto/Jetpack 官方资料和固定上游源码；没有操作 PCT-AL10、没有安装 APK、没有使用用户级 `android-cli` skill，也没有修改产品代码。

| 事实 | 当前证据 | 边界 |
| --- | --- | --- |
| Atha application id 是 `com.atha.reader`，min SDK 26，compile/target 36，依赖 `androidx.webkit:webkit:1.14.0` | [`build.gradle.kts`](../../reader/app/src-tauri/gen/android/app/build.gradle.kts) | 这是源码配置，不证明目标机当前安装包来自该工作树 |
| debug build `debuggable=true`，release build 非 debuggable | Gradle build type 与当前 merged manifest | build 产物只是本地辅助证据；真机仍需 `dumpsys package` 核对 flags/version/signature |
| 用户目标是 PCT-AL10、Android 10 / API 29、华为 WebView 114 | 当前任务上下文 | provider 包名、完整版本、multiprocess 模式和 Chromium 偏差都需现场重验 |
| Linux 可信输入 5+20 已过 | 当前 change 记录 | 不覆盖 Blink、Android touch cadence、ARM GPU 或真实屏幕呈现 |
| PCT `gfxinfo` 曾为 0 | 既有设备记录 | 只能证明该入口没捕获可报告宿主帧，不能推出 WebView 帧数 |

华为 provider 可能基于 Chromium 114 但带厂商补丁；下文的 Chromium 114 固定源码用于建立上游模型，不等同目标 provider 的逐行实现。现场必须先保存 `dumpsys webviewupdate` 和 provider package version。

## 1. 为什么 `gfxinfo = 0` 不足以诊断

Android 的 slow-rendering 文档说明，`dumpsys gfxinfo ... framestats` 与相关 vitals 面向由 `View` hierarchy / Canvas 产生的帧。`Window.OnFrameMetricsAvailableListener` 同样是窗口渲染指标，并不是 WebView renderer 的通用 presentation oracle。[`Window.OnFrameMetricsAvailableListener`](https://developer.android.com/reference/android/view/Window.OnFrameMetricsAvailableListener)。

Chromium 114 的 Android WebView 架构把 browser 代码放在 app process；renderer 通常位于隔离进程，GPU/network 仍可能位于 app process。64-bit O+ 通常 OOP renderer，但低内存 32-bit API 26-30 仍可能 in-process。[architecture.md](https://github.com/chromium/chromium/blob/bdabf09e0e69fd4ba0be83dc8269b2ebf99a9c94/android_webview/docs/architecture.md)、[renderer README](https://github.com/chromium/chromium/blob/bdabf09e0e69fd4ba0be83dc8269b2ebf99a9c94/android_webview/renderer/README.md)。

所以应同时观察四层：

| 层 | 能回答 | 不能回答 |
| --- | --- | --- |
| JS `performance.mark` / Event Timing | 事件到 handler、rAF intent、next rendering update | 像素已扫描到屏幕 |
| CDP / WebView trace | renderer main thread、layout/paint、raster、commit、compositor | Android display 的 actual present |
| 宿主 FrameMetrics / JankStats | Activity window 的被跟踪帧 | 独立 renderer 的所有更新；0 帧不等于无 jank |
| SurfaceFlinger latency | 指定 layer 的 desired/ready/actual-present 时间 | 哪次 JS 事件导致该 buffer；静止间隔不是 jank |

[`JankStats`](https://developer.android.com/topic/performance/jankstats) 可作为 API 29 的宿主补充，目前稳定版是 [`androidx.metrics:metrics-performance:1.0.0`](https://developer.android.com/jetpack/androidx/releases/metrics)，但不能替代 CDP/SF 证据。

## 2. 当前设备可立即执行的诊断

以下是**待真机验收时执行**的命令模板，本轮没有运行。开始前遵循 [`docs/agents/references.md`](../agents/references.md) 的 PCT-AL10 项目流程，并把每轮输出保存在项目 `.tmp/`；不得把 provider 名、layer 名或 serial 写死。

### 2.1 固定运行环境

```bash
serial='<live-serial>'
package='com.atha.reader'

adb -s "$serial" shell getprop ro.build.fingerprint
adb -s "$serial" shell getprop ro.build.version.sdk
adb -s "$serial" shell dumpsys webviewupdate
adb -s "$serial" shell dumpsys package "$package" | rg 'versionName|versionCode|DEBUGGABLE|PROFILEABLE|signatures'
adb -s "$serial" shell pidof "$package"
adb -s "$serial" shell ps -A | rg "$package|webview|sandboxed_process"
adb -s "$serial" shell dumpsys display | rg -i 'mActiveMode|refreshRate|supportedModes|DisplayMode'
adb -s "$serial" shell dumpsys thermalservice
adb -s "$serial" shell dumpsys battery | rg 'level|status|temperature|voltage'
```

温度状态优先看 Android 10 已公开的 `PowerManager.getCurrentThermalStatus()`；`battery temperature` 只是电池温度，不能代表 SoC 是否 throttling。[API 29 PowerManager 变更](https://developer.android.com/sdk/api_diff/29/changes/android.os.PowerManager)。刷新率以 `Display.Mode.refreshRate`、`dumpsys display` 和 SurfaceFlinger 报告的实际 period 交叉验证，[`Display.Mode`](https://developer.android.com/reference/android/view/Display.Mode)。

### 2.2 连接 WebView DevTools / CDP

WebView 113.0.5656.0 起，应用 `android:debuggable=true` 时会自动启用 Web Contents debugging；非 debuggable 默认关闭。[`setWebContentsDebuggingEnabled`](https://developer.android.com/reference/android/webkit/WebView#setWebContentsDebuggingEnabled(boolean))。Atha debug build 加 WebView 114 理论上无需产品代码即可连接，但仍须现场 feature probe。旧版 DevTools 文档中“manifest debuggable 不影响”的说法已被新 API 行为取代，不能照抄旧结论。[Remote debugging WebViews](https://developer.chrome.com/docs/devtools/remote-debugging/webviews)。

先用桌面 Chrome `chrome://inspect/#devices` 做人工确认；需要可重复导出时，再动态发现抽象 socket：

```bash
pid="$(adb -s "$serial" shell pidof "$package" | tr -d '\r' | cut -d ' ' -f1)"
adb -s "$serial" shell cat /proc/net/unix | rg "webview_devtools_remote_${pid}\b"
adb -s "$serial" forward tcp:9222 "localabstract:webview_devtools_remote_${pid}"
curl -fsS http://127.0.0.1:9222/json/version
curl -fsS http://127.0.0.1:9222/json
curl -fsS http://127.0.0.1:9222/json/protocol
adb -s "$serial" forward --remove tcp:9222
```

Chromium 的 socket 命名可在固定 114 源码 [`aw_devtools_server.cc`](https://github.com/chromium/chromium/blob/bdabf09e0e69fd4ba0be83dc8269b2ebf99a9c94/android_webview/browser/aw_devtools_server.cc) 中核对；厂商实现或多进程模式可能不同，因此必须以 `/proc/net/unix` 的实际 socket 为准。端口 `9222` 只是例子，脚本应先选空闲端口。

CDP 协议不能按当前 ToT 客户端猜测：保存 `/json/version`、`/json/protocol`，只调用目标 WebView 实际声明的方法。[Chrome DevTools Protocol](https://chromedevtools.github.io/devtools-protocol/)、[Tracing domain](https://chromedevtools.github.io/devtools-protocol/tot/Tracing/)。debug trace 会受 debuggable 和 inspector 开销影响，只用于定位，不作为 release-like 性能数值。

DevTools Performance/Layers 每轮重点保存：

- Event/Pointer handler、Long Task、style/layout/paint、raster task、commit 和 compositor 时间；
- `.book` layer 的物理尺寸、tile 数、estimated memory、paint count；
- `book.scrollWidth`、page count、DPR、viewport CSS/device pixels；
- 当前页第一次露出的公式 SVG、图片和表格是否触发 decode/raster；
- `performance.mark` 到 trace slice 的对应关系，而不是把 rAF 当作 presented frame。

### 2.3 采集 API 29 Perfetto

Android 10 起官方推荐 Perfetto；`atrace` categories 可以作为 Perfetto data source，并用 `-a package` 启用 app atrace。[System tracing](https://developer.android.com/topic/performance/tracing)、[record_android_trace](https://perfetto.dev/docs/getting-started/atrace)、[atrace data source](https://perfetto.dev/docs/data-sources/atrace)。先保存目标机实际支持列表：

```bash
adb -s "$serial" shell atrace --list_categories

# host 侧 Perfetto 工具；只保留上一条列表中存在的 category。
record_android_trace \
  -o .tmp/pct-single-gesture.perfetto-trace \
  -t 10s -a "$package" \
  sched freq idle am wm gfx view webview input
```

Android 10 InputDispatcher 的 `input` trace 主要暴露 queue/wait counters，普通 user build 未必有逐事件到逐帧 flow；不要声称 Perfetto 一定能给出 input-to-display。[Android 10 InputDispatcher](https://android.googlesource.com/platform/frameworks/native/+/refs/heads/android10-mainline-release/services/inputflinger/InputDispatcher.cpp)、[`atrace.cpp` input category](https://android.googlesource.com/platform/frameworks/native/+/e6406e3589fa490bfa435384d2a3a6ac678e08fd/cmds/atrace/atrace.cpp)。

### 2.4 读取动态 SurfaceFlinger layer

Android 10 的 `SurfaceFlinger --latency` 输出第一行为 vsync period，随后是 desired-present、frame-ready、actual-present 三元组；实现只保留 128 条循环记录。[SurfaceFlinger.cpp](https://android.googlesource.com/platform/frameworks/native/+/refs/heads/android10-release/services/surfaceflinger/SurfaceFlinger.cpp)、[FrameTracker.h](https://android.googlesource.com/platform/frameworks/native/+/60f3ab275ef3ddf3afcdfdce4eb09b59024fec51/services/surfaceflinger/FrameTracker.h)。

```bash
adb -s "$serial" shell dumpsys window windows | rg -i 'mCurrentFocus|mFocusedApp'
adb -s "$serial" shell dumpsys SurfaceFlinger --list | rg -i "$package|MainActivity|SurfaceView"

layer='<exact-live-layer-name>'
adb -s "$serial" shell dumpsys SurfaceFlinger --latency-clear "$layer"
# 恢复固定起始页后，只做一次目标手势，并立即执行：
adb -s "$serial" shell dumpsys SurfaceFlinger --latency "$layer" > .tmp/sf-latency.txt
```

每次 app 重启、页面状态或方向变化都重新选 layer；只有该 layer 的 timestamp 在目标手势期间增长、当前窗口与截图也匹配，才能纳入。128 条 ring buffer 不适合把 20 次手势堆在一次 clear 中：150ms settle 在 60Hz 已接近 9 帧，20 次会覆盖旧数据。

最小 actual-present 间隔解析器：

```bash
awk '
NR == 1 { printf "vsync_ms=%.3f\n", $1 / 1000000; next }
$3 > 0 && $3 < 9223372036854770000 {
  if (previous > 0) printf "%.3f\n", ($3 - previous) / 1000000
  previous = $3
}' .tmp/sf-latency.txt
```

只统计预先标记的 active-gesture 窗口。静止前后的长间隔不是 jank；`>1.5 × 实测 period` 可作为诊断标记，不应在没有 PCT baseline 前变成产品硬门。SurfaceFlinger 证明 buffer 被显示，不证明它来自某次 pointer event。

## 3. 可重复的 5+20 真机基准

### 场景语料

使用已净化或合成 fixture，固定内容哈希和定位点，至少覆盖：

1. 纯文本基线；
2. 密集 SVG 公式页；
3. 高像素图片页；
4. 宽表格 / `pre` 中部与边界；
5. 图片、公式、表格混合重页。

公式必须按 Atha 当前真实表示测量：它是固定尺寸的 SVG `<img>` 并在批次中 `image.decode()`，不是 native MathML。[`content.mjs` L430-L471](../../reader/web/content.mjs#L430-L471)。native MathML 只能作为未来隔离 PoC，不能冒充当前优化。

### 每轮步骤

每个 fixture × 方向 × 手势类型独立执行：

1. 冷启动或恢复到同一 build、同一章节和同一 page/overflow offset；
2. 等待内容稳定；instrumented build 可等待 `postVisualStateCallback`，但它只代表下一次 `onDraw` 可画，不代表已经显示；
3. 记录 provider、进程、orientation、viewport、DPR、brightness、电量、充电状态、thermal status、active refresh period；
4. 前 5 次作为 warmup，不计入统计；
5. 每个 measured round 重新恢复起点、重选 layer、`latency-clear`、开始短 trace、只做一次手势、立即停 trace 和 dump latency；
6. 验证语义：恰好翻一页、取消不翻页、表格中部只滚表格、边界新手势才交给 page；
7. 重复 20 次并保留原始记录。温度等级或 refresh period 改变时中止、冷却并重做该组。

建议每条原始记录至少保存：

```text
fixture,build,provider,round,gesture,direction,start_page,end_page,
semantic_ok,event_duration_ms,input_delay_ms,processing_ms,
sf_vsync_ms,sf_active_frames,sf_gap_p50_ms,sf_gap_p95_ms,sf_gap_max_ms,
thermal_start,thermal_end,refresh_start_hz,refresh_end_hz,trace_path
```

20 条排序后的第 19 条是 nearest-rank P95。报告 median、P95、max、20 次语义失败数和 raw artifact path；不得把不同 fixture、方向、手动触摸、ADB input、debug/release-like build 合并。ADB/UIAutomator 有重复性，但不是自然手指；产品验收仍要保留真实触摸。

现有 Linux 阈值可作趋势参考，不能直接移植成 PCT 门：引擎、输入采样、刷新率与 presenter 都不同。先采相同设备、相同 provider、相同 build lane 的 A/B baseline，再制定数值门。[Android performance measurement](https://developer.android.com/topic/performance/measuring-performance)。

## 4. 需要测试 APK 的第二级方案

这部分会改变 Android build 与观测面，实施前应新建 accepted change。两条 lane 必须分开：

| lane | 配置 | 用途 | 禁止结论 |
| --- | --- | --- | --- |
| diagnosis-debug | `debuggable=true`，CDP 可用 | DevTools、JS marks、DOM/layer 尺寸、快速假设定位 | 数值不能代表 release |
| benchmark-profileable | `debuggable=false`，`<profileable android:shell="true"/>`，release-like 优化 | Macrobenchmark、Perfetto、`TracingController`、SF 5+20 | 不在 shipping release 开 WebView debugging |

API 29/30 的 app tracing 需要 debuggable 或 profileable；benchmark lane 应使用 profileable 而不是 debug。[AndroidX Trace](https://developer.android.com/reference/androidx/tracing/Trace)、[Macrobenchmark instrumentation args](https://developer.android.com/topic/performance/benchmarking/macrobenchmark-instrumentation-args)。

### 4.1 原生触摸标记

`MotionEvent.getEventTime()` 在 API 29 以 `uptimeMillis` 表示原始事件时间，只有毫秒精度；`getEventTimeNanos()` 是 API 34 才公开，不能在 PCT 上假装纳秒精度。[`MotionEvent`](https://developer.android.com/reference/android/view/MotionEvent)。Activity 可在 `dispatchTouchEvent` 记录：

```kotlin
val eventNs = event.eventTime * 1_000_000L
val dispatchNs = System.nanoTime()
Trace.beginAsyncSection("atha-touch-${event.actionMasked}", sequenceId)
// JS/visual callback 完成后用同一个 sequenceId endAsyncSection。
```

`uptimeMillis` 和 `System.nanoTime` 都适合设备未休眠期间的 interval，但原始 event 只有约 ±1ms 量化；报告不得制造亚毫秒准确度。[Android 10 `SystemClock`](https://android.googlesource.com/platform/frameworks/base/+/android10-release/core/java/android/os/SystemClock.java)、[API 29 `Trace` async additions](https://developer.android.com/sdk/api_diff/29/changes/android.os.Trace)。

### 4.2 WebView trace 与 visual-ready 标记

`TracingController` 从 API 28 起可记录进程内所有 WebView，输出 Chrome JSON；选择 `ANDROID_WEBVIEW`、`INPUT_LATENCY`、`RENDERING`、`JAVASCRIPT_AND_RENDERING`、`FRAME_VIEWER` 等实际支持类别。[`TracingController`](https://developer.android.com/reference/android/webkit/TracingController)、[`TracingConfig`](https://developer.android.com/reference/android/webkit/TracingConfig)。它不要求在 release-like build 开 CDP。

`WebView.getWebViewRenderProcess()` 在 API 29 可确认是否存在 renderer handle，但 [`WebViewRenderProcess`](https://developer.android.com/reference/android/webkit/WebViewRenderProcess) 是不透明句柄，不能拿到 PID；进程归属仍以 runtime trace/`ps` 为准。

`postVisualStateCallback` 只保证当回调发生时，当前 DOM state 已准备好在下一次 WebView `onDraw` 中绘制，不保证 SurfaceFlinger 已实际呈现。[WebView API](https://developer.android.com/reference/android/webkit/WebView#postVisualStateCallback(long,android.webkit.WebView.VisualStateCallback))、[WebViewCompat](https://developer.android.com/reference/androidx/webkit/WebViewCompat#postVisualStateCallback(android.webkit.WebView,long,androidx.webkit.WebViewCompat.VisualStateCallback))。因此可用它结束“JS 到 ready-to-draw”区间，再以 SF actual-present 作为更下游但非严格因果的显示证据。

### 4.3 JS Event Timing 与 marks

Event Timing 可覆盖 trusted `pointerdown` / `pointerup` / `click` 到下一次 rendering update，但连续 `pointermove` 不在其交互报告范围；duration 也会按隐私规则量化。[W3C Event Timing](https://www.w3.org/TR/event-timing/)。WebView 114 必须 feature-detect：

```javascript
globalThis.__athaEventTimings = [];
if (PerformanceObserver.supportedEntryTypes?.includes("event")) {
  new PerformanceObserver((list) => {
    for (const entry of list.getEntries()) {
      if (entry.interactionId > 0) {
        globalThis.__athaEventTimings.push({
          name: entry.name,
          interactionId: entry.interactionId,
          duration: entry.duration,
          inputDelay: entry.processingStart - entry.startTime,
          processing: entry.processingEnd - entry.processingStart,
          presentation: entry.startTime + entry.duration - entry.processingEnd,
        });
      }
    }
  }).observe({ type: "event", buffered: true, durationThreshold: 16 });
}
```

只保存数值和交互类型，不保存 DOM target、书籍正文或文件路径。`performance.mark()` 在 Chrome 114 DevTools 中可见，[Chrome 114 DevTools](https://developer.chrome.com/blog/new-in-devtools-114/)，但 rAF/mark 仍只是 renderer intent，不是硬件 frame。

### 4.4 Macrobenchmark 的正确角色

当前稳定版是 [`androidx.benchmark:benchmark-macro-junit4:1.4.1`](https://developer.android.com/jetpack/androidx/releases/benchmark)。Macrobenchmark 应放在独立 test module，驱动非 debuggable、profileable 的 target，并为每个 iteration 保存 JSON 和 Perfetto trace。[Macrobenchmark overview](https://developer.android.com/topic/performance/benchmarking/macrobenchmark-overview)。

API 29 没有 `frameOverrunMs`；`FrameTimingMetric` 只能给 `frameDurationCpuMs`，且仍可能继承宿主窗口/WebView 归因盲区。[Macrobenchmark metrics](https://developer.android.com/topic/performance/benchmarking/macrobenchmark-metrics)。所以它负责启动、恢复、20 次 raw iteration、环境记录和 trace 归档，**不能成为 WebView presentation 的唯一 oracle**。官方也要求实体设备、原始结果留存并考虑 thermal throttling。[Benchmarking in CI](https://developer.android.com/topic/performance/benchmarking/benchmarking-in-ci)。

## 5. Atha 当前管线与优先假设

### 已有的正确基础

- `pointerdown` 缓存 reader rect、layout scale 和 overflow 几何；page owner 的 `pointermove` 只更新内存并由一个 rAF 写 transform。[`interaction.mjs` L236-L309](../../reader/web/interaction.mjs#L236-L309)、[`pagination.mjs` L600-L619](../../reader/web/pagination.mjs#L600-L619)。
- `.book` 只在 dragging 期间设置 `will-change: transform`，不是永久 promotion；这与 W3C“谨慎、提前启用、尽快撤销”的原则一致。[`atha-reader.css` L975-L996](../../reader/atha-reader.css#L975-L996)、[CSS Will Change](https://www.w3.org/TR/css-will-change/)。
- 公式 SVG `<img>` 有显式宽高并预先 `decode()`；表格和 `pre` 被限制在 `.atha-structured-overflow`。[`content.mjs` L430-L471](../../reader/web/content.mjs#L430-L471)、[`atha-reader.css` L1132-L1154](../../reader/atha-reader.css#L1132-L1154)。

### 按信息增益排序的 A/B

| 优先级 | 假设与仓库证据 | 最小 A/B | 命中信号 | 命中后才考虑 |
| --- | --- | --- | --- | --- |
| P0 | `syncViewportDeviceSize()` 把 layout width/height 乘 DPR，再在父级 `scale(1/DPR)`；字体也按 DPR 换算。[`pagination.mjs` L228-L243](../../reader/web/pagination.mjs#L228-L243)、[`preferences.mjs` L30-L37](../../reader/web/preferences.mjs#L30-L37) | 当前物理像素布局 vs CSS-pixel layout PoC | layer/tile/raster/memory 随 DPR 或 `scrollWidth × DPR` 显著下降，语义与清晰度仍正确 | 改 viewport/pagination 尺度契约 |
| P0 | `.reader` 始终有 `filter: brightness(var(--reader-brightness))`，默认值 1，并同时 scale。[`atha-reader.css` L939-L947](../../reader/atha-reader.css#L939-L947) | `brightness(1)` vs `filter:none` | composite/raster/actual-present gap 显著改善 | 默认亮度 100 时跳过 filter |
| P0 | 单 section CSS columns 形成很长的横向 `.book`，整章 transform 会暴露新 tiles | 短章 vs 长章；记录 layer/tile/paint | 卡顿随 page count/scrollWidth、tile raster 增长 | CSS-pixel layout、native scroll 或 section 分片 |
| P1 | 新页含 SVG/高像素图片/宽表，可能在第一次露出时 decode/raster | 纯文本 vs 各单一重内容 fixture | decode/raster task 与 first expose 重合 | 调整有界预解码、像素预算或占位尺寸 |
| P1 | JS handler、annotation/page-shown subscriber 可能压主线程 | 同内容、关闭非必要 observer 的测试 build | Long Task / handler slice 消失 | 移出热路径或分批 |
| P2 | renderer/GPU scheduling 或 thermal/refresh 变化 | 相同 APK、冷却前后；provider/period 固定 | app trace 轻但 SF gap 变坏且与环境相关 | 先控制环境/系统，不误改产品 |

CSS transform **可能**走 compositor，但只有 layer/tile/raster 条件满足时才避免主线程工作；rAF 仍需 renderer event loop、commit 和 compositor pipeline。[Chromium `how_cc_works`](https://github.com/chromium/chromium/blob/bdabf09e0e69fd4ba0be83dc8269b2ebf99a9c94/docs/how_cc_works.md)、[GPU accelerated compositing](https://www.chromium.org/developers/design-documents/gpu-accelerated-compositing-in-chrome/)、[RenderingNG](https://developer.chrome.com/docs/chromium/renderingng-architecture)。长 layer 由 tiles raster，不能从“只改 transform”推出零成本。[Inside look at a modern browser, part 3](https://developer.chrome.com/blog/inside-browser-part3)。

`content-visibility:auto` 可跳过离屏渲染，但布局测量会迫使内容重新渲染，size containment 也会影响 layout。Atha 依赖全章 `scrollWidth`、column page count 和 locator，因此只能做隔离正确性 PoC，不能全局套用。[content-visibility](https://web.dev/articles/content-visibility)。同理，canvas snapshot 会新增 DPR 尺寸 buffer、capture 和内存成本；只有 trace 已证明 live DOM 大 layer 是主因时才值得研究。

## 6. 成熟项目给出的可迁移原则

| 项目 | 固定源码证据 | 对 Atha 的意义 |
| --- | --- | --- |
| Readium Kotlin `f8e6f93d…` | debug build 才开启 WebView debugging；`postVisualStateCallback` 表示 visual-ready。[R2BasicWebView.kt](https://github.com/readium/kotlin-toolkit/blob/f8e6f93db81570c7cc0833279b2628f4c65d8efe/readium/navigator/src/main/java/org/readium/r2/navigator/R2BasicWebView.kt)、[WebViewUtil.kt](https://github.com/readium/kotlin-toolkit/blob/f8e6f93db81570c7cc0833279b2628f4c65d8efe/readium/navigators/web/internals/src/main/kotlin/org/readium/navigator/web/internals/webview/WebViewUtil.kt) | 诊断与发布构建分离；ready-to-draw 不冒充 presented |
| Readium legacy JS 同提交 | native viewport 以 CSS pixel 表示，并在 rAF 更新 scroll。[utils.js](https://github.com/readium/kotlin-toolkit/blob/f8e6f93db81570c7cc0833279b2628f4c65d8efe/readium/navigator/src/main/assets/_scripts/src/utils.js) | 是 P0 DPR A/B 的成熟对照，不是直接复制方案 |
| foliate-js `78914aef…` | sandbox iframe 一次只保留一个 section，CSS columns 配合 native scroll，先加载新 section 再卸旧 section。[paginator.js](https://github.com/johnfactotum/foliate-js/blob/78914aef4466eb960965702401634c2cb348e9b1/paginator.js) | section isolation 和 native scroll 是大 layer 命中后的候选 |
| Readest `1df1505f…` | captured turn 把离场页画入 2D canvas 后 transform。[pageSlide.ts](https://github.com/readest/readest/blob/1df1505fc5033fc949463c9908f2d53bd0fbdfa6/apps/readest-app/src/utils/pageSlide.ts) | snapshot 是有成本的 fallback，不是未测先用的默认优化 |

Readium、Foliate、KOReader、CREngine 和 epub.js 的手势、缓存、资源边界比较见[成熟阅读器手势、分页与重内容性能研究](./mature-reader-gesture-performance-landscape.md)；Readest Android 大 section 保护、末样本和测试盲区见[Readest 手势动画性能研究](./readest-gesture-animation-performance.md)。成熟项目的共同经验不是“永久强制 GPU layer”，而是限制昂贵工作的生命周期、隔离内容范围、保留 trace 能力并用平台真实边界做回退。

## 7. 录屏、温度和真实 input-to-display

Android `screenrecord` 会创建 mirrored virtual display，经 SurfaceFlinger buffer 和 MediaCodec encoder 输出视频；它本身参与 display/composition/encode 管线，影响量需设备实测，但不应混入正式 latency round。[Layers and displays](https://source.android.com/docs/core/graphics/layers-displays)。MP4 帧率或时间轴也不能当成应用 presented-frame 数。

正式 5+20 使用：

- Perfetto + CDP/WebView trace + SF latency 记录时序；
- 轮次开始/结束仅做稀疏 `screencap` 核对页面语义；
- 需要物理 input-to-photon 时，用外部高帧率相机同时拍手指与屏幕；
- 录屏只用于复现和视觉交流，另跑一组，不纳入性能统计。

API 29 没有 FrameTimeline，`postVisualStateCallback` 又不是 display callback，所以“MotionEvent 到 actual photon”的精确数值不能由当前内建接口直接得出。测试 APK 最多形成：原始 event time → Activity dispatch → JS handler/rAF → WebView ready-to-draw → 受控手势附近的 SF actual-present；最后一跳仍是关联推断，外部相机才是物理闭环。

不要 root、锁 clocks 或写厂商刷新率设置来制造漂亮数字。官方性能测量建议真实 UX/jank 使用同一实体设备、同一 OS 的 A/B，并记录而非掩盖 thermal 变化。[Measuring performance](https://developer.android.com/topic/performance/measuring-performance)。

## 8. 决策顺序与停止条件

1. **先判语义。** 任一手势多翻页、误触链接或错误吞掉表格滚动，性能数字作废。
2. **再判 JS/main。** handler、layout、Long Task 占满帧预算时，只修命中的同步读写或 subscriber，不先改 compositor。
3. **再判 raster/layer。** layer/tile/raster 与 DPR、长章或重内容相关时，依次试 `filter:none`、CSS-pixel layout、原生 scroll/section isolation；每次只改一个变量。
4. **最后判系统。** app trace 轻而 SF gap 与 thermal、refresh/provider 变化相关时，先控制环境或确认 provider，不把系统调度误判成产品缺陷。
5. 每个 A/B 在 5 warmup + 20 measured 后若 effect 小于自然波动、语义退化或新增长尾内存，停止该候选，不继续堆优化。

现阶段明确不做：把 `gfxinfo=0` 当通过、永久 `translateZ(0)` / `will-change`、默认 canvas screenshot、全局 `content-visibility`、从 screenrecord 估 FPS、把 native MathML 当当前公式路径、在 release 开 CDP、用 Android 12 FrameTimeline 命令描述 API 29。

## 交付建议

当前 change 的 PCT 验收先落第一级无代码诊断：**环境快照 → debug CDP Performance/Layers → 一次手势短 Perfetto → 动态 SF latency**。它能最低成本区分主线程、raster/layer 与系统三类假设。

只有第一级确认观测缺口或需要稳定回归后，再开独立 accepted change 实施第二级：debug diagnostic lane、profileable release-like lane、原生/JS trace marks、`TracingController`、Macrobenchmark 驱动和 5+20 raw artifact schema。没有这些真机证据前，不应继续把动画代码改得更复杂。

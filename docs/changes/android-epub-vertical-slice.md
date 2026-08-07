# Android EPUB 纵向闭环

## Status

implemented

## Problem

Atha 的 Tauri 产品壳目前只在 Windows 验证。Android 构建缺少 mobile crate 输出与入口，无条件依赖 Windows host / Tao，启动状态目录依赖 `LOCALAPPDATA`，手建窗口调用 desktop-only API；Android 系统文件选择器返回的 content URI 又会被现有 `FilePath::into_path` 拒绝。继续叠加格式或词典前，必须先让现有 EPUB 主循环在 Android 上真实运行且可诊断。

## Scope

- 使用 Tauri 2 官方 Android 工程与入口：补齐 mobile crate type、mobile entry point，并只在 desktop 编译验证 host / Tao 与 CLI 验收路径；
- 保留 Windows 既有 `%LOCALAPPDATA%\Atha` 数据根，Android 使用 Tauri `app_local_data_dir`；保留 Windows 验收入口，不把 Android 产品启动耦合到 desktop 窗口参数；
- 直接采用锁文件中已有的官方 `tauri-plugin-fs`，以流式复制和应用 cache 临时文件把 Android content URI 适配到现有 path-based EPUB、消息导出、备份与恢复接口；临时文件成功或失败都清理；
- 生成并版本化最小 Android 工程，使用本机有效 JDK / SDK / NDK 与 Rust target 构建；
- 增加一个可重复的 Android 检查入口，覆盖构建、安装、冷启动、AppLog / logcat、系统 picker 导入和打开一个合法的本机 EPUB；
- 更新平台 adapter、代码地图、验证事实与当前工作指针。

## Non-Goals

- 不在本切片增加 EPUB2 / NCX 或其他格式，不接入 foliate-js、词典、CSS 或统计；
- 不做 iOS、文件关联、Play Store 签名 / 发布、自动更新或远程 telemetry；
- 不迁移 Svelte / reader kernel，不创建通用移动平台层、虚构文件系统接口或新的日志栈；
- 不把模拟器结果称为 ARM 真机性能证据；没有实体机时只关闭功能纵切，性能门禁留给后续真机切片。

## Architecture Impact

present

- Design purpose: 让 Tauri adapter 吸收 Android 生命周期、应用目录和 SAF content URI 差异，backend 的 EPUB / MessageStore 与 reader kernel 继续只处理已验证的本地路径和现有契约。
- Drivers / quality scenarios: `A-AND-01`（最高业务重要性 / 高技术风险，负责人：Tauri Adapter）；刺激源是 Android 用户，刺激是在干净安装后冷启动、从系统 picker 选择 EPUB 并打开，环境是离线 Android 系统 WebView，制品是 Tauri app、平台 AppLog、Library 与 ImportedBooks，响应是应用启动完成、流式导入并进入现有 reader，度量是没有 startup / invalid-source / protocol failure，日志出现固定 startup ready、library import / open 与 reader first-stable / ready；同一版本 Windows 启动仍读取 `%LOCALAPPDATA%\Atha`，不因移动端接入改变既有数据位置。`A-URI-01`（高业务重要性 / 高技术风险，负责人：Tauri Adapter）；刺激是 picker 返回非 file URL 的 content URI，响应是用官方 fs plugin 打开描述符、在应用 cache 与现有 path API 间复制并清理，度量是导入、导出、备份、恢复均不调用 content URI 的 `into_path`，32 个导入上限与内容安全边界保持不变。
- Modules / Interfaces / Seams / Adapters: `reader/app/src-tauri` 拥有 mobile composition 与 picker bridge；`tauri-plugin-dialog` 只选择 URI，`tauri-plugin-fs` 只打开系统描述符，backend 不感知 Android；Android 生成工程只承载 Tauri，不复制产品规则。
- Candidate and tradeoffs: 采用官方插件与 cache bridge；拒绝一次性把大文件读入 `Vec<u8>`，避免词典 / 大书峰值内存；拒绝重写 importer / backup 为 Android 专用接口；拒绝让 Windows host 在 Android 编译。Rust 1.97.1 标准库锁在 Android 返回 `Unsupported`，因此只为现有 MessageStore 维护锁固定引入 `fs2 0.4.3`（`MIT/Apache-2.0`），不新增锁 abstraction。
- Evidence / ADR / review trigger: 官方 Tauri mobile、dialog 与 fs 文档；Android build / install / logcat / picker smoke；现有 Windows Tauri gate；独立 Spec / Standards review。只有 cache 复制在真机 benchmark 中成为显著瓶颈，才考虑 importer 的流式输入接口或持久 URI 权限。

## Acceptance Criteria

- [x] Android 工程可从仓库配置重建，x86_64 debug APK 在项目 AVD 安装并冷启动；mobile crate / entry point、target-gated desktop 依赖和 Android 应用目录通过编译，Windows 数据根仍为 `%LOCALAPPDATA%\Atha`；
- [x] 系统 picker 的 content URI 可导入本机合法 EPUB，书架打开后出现现有 reader first-stable / ready；重启后书架与最后稳定位置仍可用；
- [x] 导入、消息导出、全库备份与恢复共用一个最小流式 cache bridge，成功与错误路径均清理临时文件，不把路径、URI、书名或正文写入日志；
- [x] Android 检查入口记录 build / install / cold-start / logcat / picker 证据；模拟器证据与后续 ARM 真机性能证据明确区分；
- [x] 现有 Rust / Svelte / Windows Tauri 检查、docs gate 与独立 Spec / Standards review 通过。

## Files And Steps

1. 现场确认工具链与官方版本要求，生成最小 Android 工程；
2. 修正 Tauri mobile 编译、应用目录和 desktop / mobile 窗口 composition；
3. 用官方 fs plugin 建立 content URI ↔ cache 的单一流式 adapter，并把四处 picker path seam 接入；
4. 在 AVD 执行构建、安装、冷启动、导入、打开、重启与日志 smoke；
5. 回归 Windows，更新事实所有者、证据和双轴 review。

## Checks

- Rust 单元 / 集成测试与 Clippy `-D warnings`；
- `pnpm check`、`pnpm build`；
- Tauri Android init / debug build；
- AVD 安装、冷启动、系统 picker EPUB 导入、reader ready、重启恢复与 logcat 固定字段检查；
- `scripts/check-tauri-reader.ps1` Windows 回归；
- AutoCorrect、required docs gate、Spec / Standards review。

## Rollback

删除 Android 生成工程与 mobile 配置，恢复 desktop-only manifest / composition，并移除 fs plugin 直接依赖和 picker bridge；backend、reader kernel、Library 与 MessageStore 数据格式不迁移，回滚不需要改写用户数据。

## Approval

用户于 2026-08-08 明确要求优先完成 Android、先补齐日志、复用成熟库并在目标设备验证性能；日志基础已完成，本 change 是后续格式、词典、CSS 与统计之前的 Android 最小纵切。

## Result

已实现：正式 Android EPUB gate、消息 SAF opt-in 链路、Windows 回归、双轴复审与 docs gate 均已通过。

- 已生成并版本化 Tauri Android 工程；min SDK 26、compile / target SDK 36，Node 24.1.0、JDK 21、NDK 28.2.13676358 与四个 Rust Android target 已就绪；
- mobile entry、target-gated Windows host、Android `app_local_data_dir` 与 Windows `%LOCALAPPDATA%\Atha` 兼容路径已实现；
- `PickerInput` / `PickerOutput` 已把导入、消息导出、全库备份 / 恢复接到同一流式 SAF cache bridge，保留单次 32 本、EPUB / 导出 512 MiB、备份 / 恢复 8 GiB 及 backend 内部边界；
- MessageStore 维护锁已从 Android 不支持的 Rust 1.97.1 标准库调用切换到 `fs2 0.4.3`；Android app storage 拒绝 hard link 后，备份只在独占 Picker cache 内使用相邻 rename，非 Android no-replace hard-link 路径不变；
- 导入、消息 export / backup / restore 的失败日志增加固定 adapter stage，备份 / 恢复内部再区分锁、快照、校验与发布阶段；仍只记录 operation、stage、稳定 code 与耗时；
- Android manifest 已设置 `allowBackup=false` 和 API 31+ `dataExtractionRules`，并排除 cloud backup 与 device transfer；未使用且覆盖整个外部存储的生成态 `FileProvider` 已删除；
- 移动端交互诊断已移出 composition root，固定事件 token 复用 backend 校验，Windows 继续使用既有 Recorder；
- Gradle Wrapper 8.14.3 二进制已登记上游、版本和 Apache-2.0 全文；路线图已切换到 Android EPUB2 / NCX 与后续非 PDF 格式顺序。

## Review

- Blocking: 无。Standards 初审提出 Gradle Wrapper 许可、composition root 重复诊断规则、过宽且未使用的 `FileProvider`、过期路线图四项；修复后独立复审为 0 findings。Spec 最终复审为 0 findings，无缺失、scope creep 或错误实现。
- Non-blocking: Android SAF provider 不提供跨 provider 原子 replace / delete；16 KiB x86_64 AVD 的 M124 WebView 存在上游 MemoryInfra 崩溃；ARM 真机性能仍属于后续门禁。
- Out-of-scope: 非 EPUB 格式、ARM 真机性能、发布签名与商店交付在后续切片。

## Evidence And Residual Risks

- 静态审计已确认 mobile crate / entry、Windows-only host、`LOCALAPPDATA`、desktop window API 与 content URI 是当前 P0 阻塞；现有日志基础覆盖 startup、import / open、reader first-stable / ready / failure。
- `scripts/check-android-reader.ps1` 已在 `Atha_API_35_16K`（API 35、x86_64、16,384-byte page）成功完成 debug APK build、badging / permission、16 KiB ZIP / ELF alignment、install、cold start、进程存活与 startup setup / ready 日志检查；其 compile / target SDK 为 36，min SDK 为 26。
- 同一 AVD 的手工系统 picker 已成功完成合法 EPUB 导入与打开、reader first-stable / ready、应用重启后的书架 / 位置恢复、消息 export、全库 backup / restore；各流程结束及重启后 `cache/Picker` 为空，日志未记录路径、URI、书名或正文。
- `scripts/check-android-reader.ps1 -EpubPath <local.epub> -CleanAppData` 已从干净应用数据自动完成 DocumentsUI 导入、打开、first-stable / ready、强停重启、书架持久与重开，并把脱敏证据写入 ignored `artifacts/local/android/reader-gate-epub.json`；最终 APK 的消息 backup / restore / export 真实 picker 成功路径分别为 67 / 106 / 48 ms。
- 损坏备份在同一 APK 上返回 backend `stage=archive-open` 与 adapter `stage=backend`、`code=invalid-message-backup`，耗时 13 ms，失败后 Picker cache 为空；对应脱敏证据写入 ignored `artifacts/local/android/message-saf-gate.json`。
- `scripts/check-tauri-reader.ps1` 已通过 Windows Svelte build、Rust / Tauri 测试、真实 WebView2 打开 / 导入 / 窗口行为和既有性能门槛；workspace Clippy `-D warnings`、Android final build、独立 Spec / Standards 复审与 required docs gate 也已通过。
- `ACTION_CREATE_DOCUMENT` 先创建 provider 目标；完整 cache 制品向 content URI 复制时若 I/O 失败或进程中止，外部文档可能残留不完整内容。Atha 会报告失败并清理自身 cache，但当前不承诺跨 provider 删除该残留。
- 16 KiB x86_64 AVD 的 WebView / Chrome / Trichrome 124.0.6367.219 出现 MemoryInfra `SIGTRAP`，高度匹配 Chromium M125 workaround [`0634c6c`](https://chromium.googlesource.com/chromium/src/+/0634c6cbbbaa6db0064b26ed469f03e2265b9da8)（16 KiB emulator memory dump crash；真机不发生）；不改产品，需换匹配 M125+ provider 后长跑复证。
- 模拟器能够验证功能和生命周期，但不能代表低端 ARM 设备的内存、I/O、WebView 或词典性能；后续性能结论必须来自实体机。
- 本机书籍只用于 opt-in 本地验收，不进入 Git、公共 CI、日志或分发包。

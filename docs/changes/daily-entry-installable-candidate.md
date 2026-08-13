---
description: 日常导入入口、书架列表视图与内部可安装候选的范围、验收和关闭证据。
---

# 日常入口与内部可安装候选

## Status

implemented

## Problem

资料库已经能通过系统选择器导入书籍，但桌面用户仍不能把文件直接拖入书架，也不能从系统文件关联冷启动 Atha；书架只有封面网格，在书目较多时不便快速扫描。当前 Linux 日常门运行的是 debug 可执行文件，Android 虽已有候选脚本，也尚未与本轮产品入口组合成一组明确的内部安装包门禁。

Readest 的 `RD-01` 与 `RD-07` 都把搜索、导入和视图切换收在安静的书架工具区，公开 Web 空书架还提供整窗拖放入口。Atha 只借鉴这一入口层级，继续复用现有 importer、LocalLibrary、系统 WebView 和内容安全边界。

## Scope

- 桌面资料库通过 Tauri 原生 `onDragDropEvent` 接收文件路径；拖入窗口时显示明确状态，放下后走现有导入器、格式与大小上限、去重和失败投影，不给 Web 内容通用文件系统权限；
- 在现有书架增加网格 / 列表图标分段控件；列表复用同一 `LibraryBook`、搜索、排序、进度分组、选择、移出、删除和打开行为，不增加第二套书架状态；
- 为 Linux 与 Windows 桌面包声明现有十种书籍后缀的文件关联；冷启动收到纯书籍路径参数时导入全部有效文件并打开第一本，既有 Windows 诊断参数保持原义；
- 生成 Linux AppImage 内部候选，检查文件关联元数据，并由候选内 `AppRun` 完成冷启动关联、完整 Linux Tauri 回归与内容无关日志；分页稳定等待在平台不回调动画帧时有限时退化，不能让候选永久停在内容已载入状态；Windows 只落打包配置，不把 Linux 结果外推为 NSIS 验收；
- 复用 `check-pct-reader.sh` 构建并验证同一提交的签名 arm64 Android APK；在已授权 PCT-AL10 上只做同包、同签名、不降级且不请求清数据的覆盖安装和启动烟测。

## Non-goals

- 不增加单实例插件、运行中实例的文件转交、多窗口、目录监视或后台导入队列；本轮文件关联只保证冷启动；
- 不持久化网格 / 列表偏好，不新增书架数据库字段、缩略图管线或虚拟列表；
- 不让 Android 注册桌面文件关联，Android 继续使用 SAF 系统选择器；
- 不生成 macOS / iOS 包，不发布、上传或自动更新，也不把内部测试签名候选称为生产发行版；
- 不把 ADB 启动烟测称为自然触摸、完整移动功能或性能验收。

## Architecture Impact

present

- 资料库继续只有一套 `LocalLibrary`、`LibraryBook[]` 与导入结果；系统选择器、桌面拖放和冷启动文件关联只共用同一个有界 staging 入口。
- 新增的桌面路径 command 只允许资料库根路由，路径数量、空值和长度先在 IPC 边界校验，再由既有 importer 复核普通文件、格式、大小与内容；书内文档和 Android 均未获得通用文件系统权限。
- Linux / Windows 打包拓扑增加文件关联，Linux 产生内部 AppImage；没有引入单实例插件、后台队列或新的数据 schema。若后续要求运行中实例接收文件或生产发布，再建立独立 change 复查单实例、签名与升级链路。
- 备选方案是只保留系统选择器，或现在引入单实例转交与持久导入队列；前者不能覆盖日常桌面入口，后者超出已证明需求，因此本轮只实现冷启动组合。

## Acceptance Criteria

- `ENTRY-DROP-01`：Linux Tauri 真壳在有书与空书架状态接收 enter / leave / drop；状态层不会遮挡后续操作，支持格式被加入书架，重复文件保持单一记录，不支持文件显示内容无关错误且应用不崩溃。
- `ENTRY-TRUST-01`：拖放 IPC 只允许资料库根页面、每次至多 32 个非空有界路径；所有输入仍由 LocalLibrary 校验格式、普通文件、大小和内容。资料库 URL、错误状态、AppLog 和交付输出不包含源路径、文件名、书名或正文；阅读页只沿用既有 16 字符 opaque state key，不新增完整内容身份或路径投影。
- `ENTRY-LIST-01`：在 `360x760`、`1000x760` 与 `1280x800` 切换网格 / 列表；封面、书名、作者、选择态和操作均无重叠、裁切或横向溢出，搜索、排序、进度分组、批量选择与打开继续作用于同一结果集。
- `ENTRY-ASSOC-01`：Linux AppImage 的 desktop metadata 声明受支持 MIME，Windows NSIS 配置声明十种后缀；候选内 Linux 二进制以两个不同格式的公开书籍和一个非普通路径冷启动，导入两个有效文件、拒绝非普通路径并打开第一本；重复启动保持完整书籍身份集合不变，普通启动仍进入资料库。
- `CANDIDATE-DESKTOP-01`：从干净候选目录生成唯一 AppImage，记录 SHA-256；解包后运行候选 `AppRun` 完成完整 `check-reader-linux.sh`，而不是回退到 debug 产物。
- `CANDIDATE-ANDROID-01`：`check-pct-reader.sh build` 与 `verify` 通过 package、SDK、arm64、签名、权限、ZIP / ELF 16 KiB 对齐检查；同签名覆盖安装后 package、版本、签名和首次安装时间符合脚本约束，应用可启动且脚本未请求清数据。

## Files And Steps

1. 复用资料库导入 command 的共用 staging 循环，增加有来源校验的桌面路径入口和纯路径启动参数识别。
2. 在 `LibraryView` 注册 Tauri 拖放监听、共用导入结果处理，并用现有书卡 DOM 增加最小列表布局。
3. 增加 Linux / Windows 平台打包配置；Android 保持 SAF 与现有 manifest 权限边界。
4. 扩充 Linux 正式 runner 以测试合成 Tauri 拖放事件、列表回归和指定候选二进制；为分页稳定等待增加有限时帧兜底，并增加可重复的 AppImage 构建 / 元数据 / 解包入口。
5. 运行桌面候选门、Android build / verify、获准 PCT-AL10 覆盖安装与启动烟测，更新事实所有者并独立复审。

## Checks

- `pnpm --dir reader/app check && pnpm --dir reader/app build`；
- `cargo test --locked -p atha-reader-app` 与受影响前端单元检查；
- `bash scripts/check-reader-candidate.sh` 与 `bash scripts/check-reader-linux.sh`；
- `ATHA_ANDROID_KEYSTORE_PASSWORD=... bash scripts/check-pct-reader.sh build`、`verify`，以及获准的显式设备 `install`；
- `autocorrect --fix/--lint` 仅针对本次中文 Markdown；
- `project_workflow.py station <task> --activity verification --gate docs`。

## Result

- 桌面资料库接入 Tauri 原生拖放事件，拖放与系统选择器共用 `ImportReport` 和现有 LocalLibrary staging；每次最多 32 个非空、最长 32,768 字符的路径，command 只接受资料库根路由。失败结果只返回稳定错误码，界面按错误种类汇总内容无关文案，不投影文件名或路径；拖入状态使用不可交互的整窗提示，完成后继续复用搜索、排序、选择、移出、删除和打开链路。
- 书架增加不持久化的网格 / 列表图标分段控件；列表只改变同一书卡 DOM 的布局，没有新增书目 DTO、store 或数据库字段。
- Linux 与 Windows 包声明十种既有后缀；桌面冷启动只把全部参数均解析为受支持书籍路径时进入关联导入，导入全部有效文件并一次性打开第一本。普通启动仍进入资料库，Windows 既有诊断参数保持原义，Android 配置不继承桌面关联。
- Linux 候选门从干净目录构建唯一 AppImage，检查 desktop metadata 后直接驱动解包的 `AppRun` 完成冷启动关联和全部 Linux Tauri 回归；分页的单帧等待只在平台 100ms 内不回调 `requestAnimationFrame` 时退化，正常帧路径不变。
- PCT-AL10 候选复用既有脚本构建、签名、校验并覆盖安装；版本、签名与首次安装时间符合安全更新约束，未请求清数据，安装后主进程和 `MainActivity` 正常。

## Review

Standards 与 Spec 两轴独立审查先后发现失败结果泄露文件名、Android 可见桌面路径 command、候选回退 debug 产物、PCT 启动证据不足，以及关联门未覆盖多文件完整去重。实现分别收紧为固定错误码、desktop-only command / permission、候选 `AppRun` 完整门、正式 PCT 启动 / 存活 / 焦点检查，以及两个有效文件完整身份集合复跑。最终工作树两轴复审均无实现阻塞。

## Evidence And Residual Risks

- 静态与本地证据：冻结源码通过 Svelte check、15 个 Tauri Rust 测试、10 个资料库前端测试、Clippy `-D warnings`、三份 Bash 脚本语法与 ShellCheck；Windows 十种扩展与 Android 关联隔离只完成配置静态检查。
- Linux AppImage 真实目标证据：`bash scripts/check-reader-candidate.sh` 从干净目录生成 `target/release/bundle/appimage/Atha_0.1.0_amd64.AppImage`，大小 157,759,992 字节，SHA-256 为 `00107963d2aee65521e5b19cdc7002021626768693287f74812e6819d1467db4`。解包候选的 `AppRun` 在 X11 Tauri / WebKitGTK 0.55.1 中以公开 FB2、Markdown 和非普通路径冷启动，两个有效文件全部入库、首本打开、无效项拒绝；重复启动的完整身份集合不变，普通启动回到资料库。
- 同一候选 `AppRun` 在真实 `360x760`、`600x760`、`1000x760`、`1280x800` 与 `1600x900` 启动视口完成合成 Tauri 拖放事件、空 / 有书架、网格 / 列表、数据管理、阅读记忆、桌面工作区与 AppLog 隐私回归。13 个场景各 5 次预热、20 次测量，共记录 220 次可信 `mouse` Pointer Actions；输入到首次可视更新、点按、拖动帧间隔、最大帧间隔和松手稳定 P95 分别为 31、7、17、17 与 342ms。WebKitGTK touch Actions 当前会挂起，因此该门不声称触摸证据。
- PCT-AL10 真实目标证据：冻结源码构建的 arm64 APK 大小 28,205,228 字节，SHA-256 为 `8f023cd785b2460faaeafb1c6da988bfe5a4ad3b40a63a5642b0ddb1073de8ac`，versionCode 1000；package、SDK、签名、权限、ZIP / ELF 16 KiB 检查通过。同包同签名非降级覆盖安装达到 terminal success，候选 hash、版本和证书匹配，首次安装时间保持且 `data_clear_requested=false`；`am start -W` 返回成功，3 秒后主进程仍存活且 `MainActivity` 聚焦。这里不声称书架或设置连续性已重新实测。
- 未覆盖真实 OS 鼠标拖放、Windows NSIS 构建 / 关联、自然手指触摸、PCT 完整移动功能 / 性能、生产签名、自动更新、发布与分发；这些结果也不等同于生产验收。

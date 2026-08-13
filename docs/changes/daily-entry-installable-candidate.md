---
description: 日常导入入口、书架列表视图与内部可安装候选的范围、验收和关闭证据。
---

# 日常入口与内部可安装候选

Status: accepted

## Problem

资料库已经能通过系统选择器导入书籍，但桌面用户仍不能把文件直接拖入书架，也不能从系统文件关联冷启动 Atha；书架只有封面网格，在书目较多时不便快速扫描。当前 Linux 日常门运行的是 debug 可执行文件，Android 虽已有候选脚本，也尚未与本轮产品入口组合成一组明确的内部安装包门禁。

Readest 的 `RD-01` 与 `RD-07` 都把搜索、导入和视图切换收在安静的书架工具区，公开 Web 空书架还提供整窗拖放入口。Atha 只借鉴这一入口层级，继续复用现有 importer、LocalLibrary、系统 WebView 和内容安全边界。

## Scope

- 桌面资料库通过 Tauri 原生 `onDragDropEvent` 接收文件路径；拖入窗口时显示明确状态，放下后走现有导入器、格式与大小上限、去重和失败投影，不给 Web 内容通用文件系统权限；
- 在现有书架增加网格 / 列表图标分段控件；列表复用同一 `LibraryBook`、搜索、排序、进度分组、选择、移出、删除和打开行为，不增加第二套书架状态；
- 为 Linux 与 Windows 桌面包声明现有十种书籍后缀的文件关联；冷启动收到纯书籍路径参数时导入全部有效文件并打开第一本，既有 Windows 诊断参数保持原义；
- 生成 Linux AppImage 内部候选，检查文件关联元数据、候选内二进制冷启动关联、完整 Linux Tauri 回归与内容无关日志；Windows 只落打包配置，不把 Linux 结果外推为 NSIS 验收；
- 复用 `check-pct-reader.sh` 构建并验证同一提交的签名 arm64 Android APK；在已授权 PCT-AL10 上只做同包、同签名、不降级且保留数据的覆盖安装和启动烟测。

## Non-goals

- 不增加单实例插件、运行中实例的文件转交、多窗口、目录监视或后台导入队列；本轮文件关联只保证冷启动；
- 不持久化网格 / 列表偏好，不新增书架数据库字段、缩略图管线或虚拟列表；
- 不让 Android 注册桌面文件关联，Android 继续使用 SAF 系统选择器；
- 不生成 macOS / iOS 包，不发布、上传或自动更新，也不把内部测试签名候选称为生产发行版；
- 不把 ADB 启动烟测称为自然触摸、完整移动功能或性能验收。

## Acceptance

- `ENTRY-DROP-01`：Linux Tauri 真壳在有书与空书架状态接收 enter / leave / drop；状态层不会遮挡后续操作，支持格式被加入书架，重复文件保持单一记录，不支持文件显示内容无关错误且应用不崩溃。
- `ENTRY-TRUST-01`：拖放 IPC 只允许资料库根页面、每次至多 32 个非空有界路径；所有输入仍由 LocalLibrary 校验格式、普通文件、大小和内容，URL、页面、AppLog 和交付输出不包含本地路径或私有书籍标识。
- `ENTRY-LIST-01`：在 `360x760`、`1000x760` 与 `1280x800` 切换网格 / 列表；封面、书名、作者、选择态和操作均无重叠、裁切或横向溢出，搜索、排序、进度分组、批量选择与打开继续作用于同一结果集。
- `ENTRY-ASSOC-01`：Linux AppImage 的 desktop metadata 声明受支持 MIME，Windows NSIS 配置声明十种后缀；候选内 Linux 二进制以公开书籍路径冷启动后导入并打开该书，重复启动不复制记录，普通启动仍进入资料库。
- `CANDIDATE-DESKTOP-01`：从干净候选目录生成唯一 AppImage，记录 SHA-256；解包后运行候选内二进制完成完整 `check-reader-linux.sh`，而不是回退到 debug 产物。
- `CANDIDATE-ANDROID-01`：`check-pct-reader.sh build` 与 `verify` 通过 package、SDK、arm64、签名、权限、ZIP / ELF 16 KiB 对齐检查；同签名覆盖安装后 package、版本、签名和首次安装时间符合脚本约束，应用可启动且未清数据。

## Files And Steps

1. 复用资料库导入 command 的共用 staging 循环，增加有来源校验的桌面路径入口和纯路径启动参数识别。
2. 在 `LibraryView` 注册 Tauri 拖放监听、共用导入结果处理，并用现有书卡 DOM 增加最小列表布局。
3. 增加 Linux / Windows 平台打包配置；Android 保持 SAF 与现有 manifest 权限边界。
4. 扩充 Linux 正式 runner 以测试合成 Tauri 拖放事件、列表回归和指定候选二进制；增加可重复的 AppImage 构建 / 元数据 / 解包入口。
5. 运行桌面候选门、Android build / verify、获准 PCT-AL10 覆盖安装与启动烟测，更新事实所有者并独立复审。

## Checks

- `pnpm --dir reader/app check && pnpm --dir reader/app build`；
- `cargo test --locked -p atha-reader-app` 与受影响前端单元检查；
- `bash scripts/check-reader-candidate.sh` 与 `bash scripts/check-reader-linux.sh`；
- `ATHA_ANDROID_KEYSTORE_PASSWORD=... bash scripts/check-pct-reader.sh build`、`verify`，以及获准的显式设备 `install`；
- `autocorrect --fix/--lint` 仅针对本次中文 Markdown；
- `project_workflow.py station <task> --activity verification --gate docs`。

## Result

实施后回填。

## Review

候选提交前执行 Standards 与 Spec 两轴独立审查并回填。

## Evidence And Residual Risks

实施后回填；至少区分静态 / 单元、Linux AppImage 真壳、PCT-AL10 安装与启动烟测，并明确 Windows NSIS、真实 OS 鼠标拖放、自然触摸和生产签名未覆盖。

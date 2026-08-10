---
description: 研究 PCT-AL10 Android 10 上 Shizuku、Sui 与 PackageInstaller 的真实调用链，界定不安装 Shizuku 时的更新能力与华为验证边界。
---

# PCT-AL10 上 Shizuku 与 PackageInstaller 调用链研究

## 结论先行

针对“已有 ADB shell、没有 Shizuku、没有 root，能否把 Atha 更新到当前用户且不触发华为 USB 安装 / 九宫格验证”，源码能支持的结论是：

1. **更新能力存在。** PCT-AL10 当前用户是 0，ADB shell 是 UID 2000，`com.android.shell` 实测持有 `android.permission.INSTALL_PACKAGES`。Android 10 的 `PackageInstaller` session 可以由这个身份更新同包名、同签名且版本规则允许的 Atha。
2. **不触发华为验证无法保证。** 无论 `adb install`、`cmd package`、ADB 启动的 Shizuku，还是一次性 `app_process` DEX，最终调用 `PackageInstallerService` 的身份仍是 UID 2000。AOSP Android 10 会强制给 shell/root session 加 `INSTALL_FROM_ADB`；PCT-AL10 自带的华为 PackageInstaller 又存在明确的 ADB 分支和口令 / 图案 / PIN 验证能力。因此 Shizuku 不是把“ADB 安装”改造成“非 ADB 安装”的魔法开关。
3. **官方 Shizuku 不能原样免安装复用。** 当前 starter 来自已安装 Manager APK 的 native library，启动时还要定位该 Manager APK；server 随后再次查询 Manager，找不到就退出。只把 starter 推到设备上不够。旧版 `start.sh` 和新版 `libshizuku.so` 的包装形式不同，但都依赖已安装的 Shizuku Manager。
4. **可以自建一次性 shell DEX，但不是绕过。** 它可以直接用平台 AIDL 调 `IPackageInstaller`，减少 `adb` 客户端和 `pm` 参数解析的不确定性，也便于完整记录 session 回调；它不会改变 UID、SELinux 域或 `INSTALL_FROM_ADB`，所以只适合作为诊断 A/B，不是华为验证绕过方案。
5. **不采用 vvb2060/PackageInstaller 的“Bypass”做法。** 该项目不仅通过 Shizuku 包装 PackageInstaller Binder，还主动把全局 `verifier_verify_adb_installs` 改成 0。那是安全设置写入，不等于 PackageInstaller session 自然静默，也没有证据表明它能消除 PCT-AL10 的九宫格验证。本项目不应复制。

本轮只读检查了固定上游源码和 PCT-AL10 当前状态，拉取并静态分析了设备上的系统 PackageInstaller APK；**没有安装 APK、没有创建或提交安装 session、没有修改设备设置，也没有操作验证界面**。

## 直接回答

| 问题 | 回答 | 证据边界 |
| --- | --- | --- |
| Shizuku 是什么？ | 一个在 shell 或 root 身份运行的 Java server；普通 app 通过它转发 Android Binder 调用 | 官方源码与文档 |
| 无 root 的 ADB 模式获得什么权限？ | 获得 shell UID 2000 已有的权限，不是 root，也不是 system UID | Shizuku `transactRemote()` 与 PCT 实测 |
| Shizuku 最终用什么 API 安装？ | 仍是 Android `PackageInstaller` / `IPackageInstaller` session：create、open、write、commit | vvb installer 源码与 AOSP |
| 不装 Shizuku app 能跑官方 starter 吗？ | 不能原样跑通；starter 和 server 都依赖已安装 Manager APK | Shizuku starter/server 源码 |
| 可否直接跑自有 DEX？ | 技术上可以，`app_process` 会继承 adb shell 身份 | AOSP 与设备只读探针；未在本轮执行 |
| 自有 DEX 会避开华为 ADB 安装识别吗？ | 不会；AOSP 服务按 calling UID 强制加入 `INSTALL_FROM_ADB` | AOSP Android 10 源码 |
| 能否承诺没有九宫格？ | 不能；华为安装器明确区分 ADB 路径，并具备凭据验证 UI | PCT 系统 APK 静态证据；尚无受控安装 A/B |
| Sui 能否替代？ | 不能；Sui 要求 Magisk/root | Sui 官方源码文档 |

## 证据分层

### 当前 PCT-AL10 的只读事实

2026-08-09 现场探针得到：

```text
model=PCT-AL10
sdk=29
fingerprint=HONOR/PCT-AL10/HWPCT:10/HUAWEIPCT-AL10/10.1.0.162C00:user/release-keys
security_patch=2020-08-01
current_user=0
shell_identity=uid=2000(shell), context=u:r:shell:s0
selinux=Enforcing
shizuku_manager=absent
shell_install_packages=granted
verifier_verify_adb_installs=null
app_process=executable
packageinstaller_version_code=1300001301
packageinstaller_sha256=431be203cb2338968950bd98af7cab429485dcb7d5a8715ca6fd2ecc4f4ca6f2
```

系统安装器是 `com.android.packageinstaller`，实际 APK 位于 `/system/priv-app/PackageInstaller/PackageInstaller.apk`。这些事实证明“shell 能调用安装 API”的前置条件成立，不证明某次 commit 会跳过厂商策略。

### 固定源码

本轮固定到以下提交或 tag，避免用当前网页分支替代被审计代码：

| 项目 | 固定版本 | 用途 |
| --- | --- | --- |
| RikkaApps/Shizuku | `b844bc491f1790c72328e1a8e5b2349f8978f0ea` | starter、server 与 Manager 依赖 |
| RikkaApps/Shizuku-API | `a27f6e4151ba7b39965ca47edb2bf0aeed7102e5` | Binder 转发实现 |
| RikkaApps/Sui | `2f5fd2a04bc061eb2a8431cc3ede9066954f5a7c` | root/Magisk 边界 |
| vvb2060/PackageInstaller | `3d113a5e000c62a712e6165cb75cbca63fb912aa` | 一个成熟 Shizuku 安装器的真实调用链 |
| AOSP | `android-10.0.0_r47` | API 29 PackageInstaller 服务语义 |

### 未验证项

- 未在这台 PCT-AL10 上 commit 任何测试 session，因此没有本轮实测的 `STATUS_PENDING_USER_ACTION`、华为 Activity 栈或安装结果。
- 华为 framework / PackageManager 的闭源策略端不在系统 PackageInstaller APK 内；静态分析只能证明客户端存在 ADB 分支和凭据 UI，不能逐行还原触发条件。
- 之前某次更新如果成功，只能证明当时 APK、系统状态、调用入口和策略组合成功；没有保留 caller、session flags、回调和 UI 栈时，不能推广成“Shizuku 类方案总能绕过”。

## Shizuku ADB starter 实际做了什么

### 新版启动链

当前 Manager 的 `Starter.kt` 把 starter 定位到已安装应用的 `nativeLibraryDir/libshizuku.so`，展示给用户的命令是 `adb shell <starter-file>`；内部启动则额外传入 Manager 的 `sourceDir`。[`Starter.kt`](https://github.com/RikkaApps/Shizuku/blob/b844bc491f1790c72328e1a8e5b2349f8978f0ea/manager/src/main/java/moe/shizuku/manager/starter/Starter.kt#L6-L15)

native starter 的步骤是：

1. 只接受 UID 0 或 2000；普通 app UID 直接退出。
2. 清理旧的 `shizuku_server` 进程。
3. 从 `--apk=` 读取 Manager APK 路径；没有参数时执行 `pm path moe.shizuku.privileged.api`。
4. 仍找不到 Manager APK 就以 `fatal: can't get path of manager` 退出。
5. 设置 `CLASSPATH=<manager-apk>`，执行 `/system/bin/app_process ... rikka.shizuku.server.ShizukuService`。

对应源码见 [`starter.cpp` 的 app_process 组装](https://github.com/RikkaApps/Shizuku/blob/b844bc491f1790c72328e1a8e5b2349f8978f0ea/manager/src/main/jni/starter.cpp#L48-L114) 和 [Manager APK 解析](https://github.com/RikkaApps/Shizuku/blob/b844bc491f1790c72328e1a8e5b2349f8978f0ea/manager/src/main/jni/starter.cpp#L185-L279)。

即使人为给 starter 传入一个可读 APK，server 构造时还会通过 PackageManager 查询安装在 user 0 的 Manager；结果为空立即以 `MANAGER_APP_NOT_FOUND` 退出，卸载 Manager 后也会退出。[`ShizukuService.java`](https://github.com/RikkaApps/Shizuku/blob/b844bc491f1790c72328e1a8e5b2349f8978f0ea/server/src/main/java/rikka/shizuku/server/ShizukuService.java#L81-L122)

### 旧版 `start.sh`

官方指南中的 Android 10 及以下旧入口是：

```text
adb shell sh /sdcard/Android/data/moe.shizuku.privileged.api/start.sh
```

它同样以安装后的 Manager 数据目录和 APK 为基础。Shizuku 13.6 把启动入口更新为可执行的 native starter，解决 shell 执行脚本等兼容问题；这不是取消 Manager 依赖。[官方启动指南](https://shizuku.rikka.app/zh-hans/guide/setup/)、[Shizuku v13.6.0](https://github.com/RikkaApps/Shizuku/releases/tag/v13.6.0)

因此，在当前 `moe.shizuku.privileged.api` 缺失的 PCT-AL10 上，“先启动官方 Shizuku，再用它安装 Atha”是循环依赖：为了得到 Shizuku，先要安装 Shizuku Manager，而第一次安装本身可能进入同一个华为验证链。

## Binder 身份没有被升级

Shizuku client 用 `ShizukuBinderWrapper` 包装目标 system-service Binder。server 收到转发请求后校验已授权 client，读取目标 Binder 和 transaction code，调用 `Binder.clearCallingIdentity()`，再执行 `targetBinder.transact(...)`。[`Service.java`](https://github.com/RikkaApps/Shizuku-API/blob/a27f6e4151ba7b39965ca47edb2bf0aeed7102e5/server-shared/src/main/java/rikka/shizuku/server/Service.java#L136-L168)、[`ShizukuBinderWrapper.java`](https://github.com/RikkaApps/Shizuku-API/blob/a27f6e4151ba7b39965ca47edb2bf0aeed7102e5/api/src/main/java/rikka/shizuku/ShizukuBinderWrapper.java)

`clearCallingIdentity()` 清掉的是 client app 通过 Binder 带来的身份，随后的 transaction 以 server 进程身份发出：

- ADB 启动 Shizuku：UID 2000 / shell；
- root 启动 Shizuku：UID 0 / root；
- Sui：Magisk 提供的 root 身份。

所以无 root ADB Shizuku 的价值是：让普通 app 高效、结构化地复用 shell 能力，并管理授权；它不会凭空得到 system UID，也不会把 shell 安装伪装成普通商店安装。官方 Shizuku-API README 也把“先安装 Shizuku 或 Sui”列为使用前提。[Shizuku-API README](https://github.com/RikkaApps/Shizuku-API/blob/a27f6e4151ba7b39965ca47edb2bf0aeed7102e5/README.md)

Sui 不是无 root 后备方案。官方 README 明确说明它为 root app 提供 API，并要求 Magisk。[Sui README](https://github.com/RikkaApps/Sui/blob/2f5fd2a04bc061eb2a8431cc3ede9066954f5a7c/README.md#L1-L12)

## 安装最后仍落到 PackageInstaller session

vvb2060/PackageInstaller 是观察成熟实现的直接证据：

1. 它把 `IPackageManager`、`IPackageInstaller` 和打开后的 `IPackageInstallerSession` Binder 都包进 `ShizukuBinderWrapper`。[`Hook.kt`](https://github.com/vvb2060/PackageInstaller/blob/3d113a5e000c62a712e6165cb75cbca63fb912aa/app/src/main/java/io/github/vvb2060/packageinstaller/model/Hook.kt#L52-L105)
2. 它构造 `SessionParams`，调用 `createSession()`、`openSession()`，把 APK 写入 session，再用本地 `IntentSender` 接收 `commit()` 结果。[`InstallRepository.kt` staging](https://github.com/vvb2060/PackageInstaller/blob/3d113a5e000c62a712e6165cb75cbca63fb912aa/app/src/main/java/io/github/vvb2060/packageinstaller/model/InstallRepository.kt#L118-L167)、[`commit()`](https://github.com/vvb2060/PackageInstaller/blob/3d113a5e000c62a712e6165cb75cbca63fb912aa/app/src/main/java/io/github/vvb2060/packageinstaller/model/InstallRepository.kt#L367-L383)
3. installer package name 可以写为自身、Play 或 `com.android.shell`，但 Android 服务仍以 Binder calling UID 做权限和来源判断，不能只靠字符串改变身份。[`createSessionParams()`](https://github.com/vvb2060/PackageInstaller/blob/3d113a5e000c62a712e6165cb75cbca63fb912aa/app/src/main/java/io/github/vvb2060/packageinstaller/model/InstallRepository.kt#L317-L365)

这与 `adb install` / `cmd package install-*` 在服务端走的是同一类 session 管线。AOSP `PackageManagerShellCommand` 本身也是创建、写入、提交 PackageInstaller session，而不是另一套隐藏安装引擎。[AOSP `PackageManagerShellCommand.java`](https://android.googlesource.com/platform/frameworks/base/+/refs/tags/android-10.0.0_r47/services/core/java/com/android/server/pm/PackageManagerShellCommand.java)

## Android 10 为什么仍把它标成 ADB 安装

AOSP Android 10 `PackageInstallerService.createSessionInternal()` 先读取 `Binder.getCallingUid()`。调用者为 shell 或 root 时，服务端无条件加入 `PackageManager.INSTALL_FROM_ADB`；调用者不是 shell/root 时反而清掉该 flag。非 system UID 还会被清掉 `INSTALL_DISABLE_VERIFICATION`。[`PackageInstallerService.java`](https://android.googlesource.com/platform/frameworks/base/+/refs/tags/android-10.0.0_r47/services/core/java/com/android/server/pm/PackageInstallerService.java#471)

这意味着：

- 不论 client 自己怎样构造 `SessionParams`，UID 2000 都会在服务端被标为 ADB 来源；
- 把 `installerPackageName` 改成别的包名不能抵消 calling UID；
- 一次性 DEX 与 Shizuku server 都是 shell，因此结果相同；
- shell 不能合法保留 `INSTALL_DISABLE_VERIFICATION`。

stock AOSP 的用户确认规则又是另一层。`PackageInstallerSession.needToAskForPermissionsLocked()` 会检查 installer UID 是否拥有 `INSTALL_PACKAGES`；拥有时通常不需要 stock 确认，除非存在 force prompt 等条件。确需用户操作时，commit 通过 `ACTION_CONFIRM_INSTALL` 返回确认 Intent。[`PackageInstallerSession.java` 权限判定](https://android.googlesource.com/platform/frameworks/base/+/refs/tags/android-10.0.0_r47/services/core/java/com/android/server/pm/PackageInstallerSession.java#382)、[确认 Intent](https://android.googlesource.com/platform/frameworks/base/+/refs/tags/android-10.0.0_r47/services/core/java/com/android/server/pm/PackageInstallerSession.java#1376)

PCT 实测 `com.android.shell` 持有 `INSTALL_PACKAGES`，所以**仅按 stock AOSP**，shell 可跳过 stock PackageInstaller 的普通确认；但华为可以在 `INSTALL_FROM_ADB` 来源上叠加自己的安全策略。这正是“Android 权限足够”与“厂商不会弹验证”不能画等号的原因。

Android 10 也没有后来公开的 `SessionParams.setRequireUserAction()` 可调策略；即使较新 Android 有该 API，它也不是让无权调用者绕过 OEM 安全策略的承诺。[Android `PackageInstaller.SessionParams`](https://developer.android.com/reference/android/content/pm/PackageInstaller.SessionParams)

## PCT-AL10 华为安装器的本机证据

从设备只读拉取的 `/system/priv-app/PackageInstaller/PackageInstaller.apk` 不是 stock AOSP 安装器。受检文件 SHA-256 为 `431be203cb2338968950bd98af7cab429485dcb7d5a8715ca6fd2ecc4f4ca6f2`，对应上述 `10.1.0.162C00` 固件和 PackageInstaller `versionCode=1300001301`。其 manifest 和 DEX 包含华为纯净模式、风险控制、系统管理器与静默更新相关组件。对安装入口的静态分析发现：

1. `InstallStartImpl` 读取布尔 extra `hw_adb_install`；命中后把来源标记为 ADB，并使用 `PCUSB` / `adb_installer_name`，本机中文资源显示为“PC 工具”。
2. 后续 `PackageInstallerActivity` 继续传递 `adb_install=true` 与 ADB session id，并包含 `session install continue, isAdb:`、`adb install continue` 等分支日志。
3. 安装控制结果类型 `InstallationControlResult` 提供 `getPwdCheckType()`；凭据模板包含 `passwordTitle`、`patternTitle`、`pinTitle`，也包含指纹和人脸标题。说明华为安装 UI 确实具备由策略结果控制的密码、图案或 PIN 验证能力。

这些是当前 PCT 系统 APK 的真实静态证据，比拿 stock AOSP 猜华为行为更接近目标机。但必须保留两条边界：

- `hw_adb_install` 很可能由厂商 PackageManager / framework 根据来源写入；闭源服务端不在该 APK 内，本轮不能证明它只由某一个 flag 决定。
- 凭据 UI 的存在不能单独证明用户之前看到的九宫格就是这一个类触发；需要受控 commit、Activity 栈和日志才能完成因果确认。

它仍足以否定“只要改成 Shizuku Binder，华为就看不出 ADB”的假设：官方 ADB Shizuku 和自有 shell DEX 都没有改变服务端可见的 UID 2000。

## 为什么不采用 vvb 安装器的 verifier 修改

vvb 安装器初始化时先调用 `Hook.disableAdbVerify(context)`。该方法通过 Shizuku 包装全局 Settings provider 的 Binder，在现值非 0 时执行：

```kotlin
Settings.Global.putInt(contentResolver, "verifier_verify_adb_installs", 0)
```

源码见 [`Hook.kt`](https://github.com/vvb2060/PackageInstaller/blob/3d113a5e000c62a712e6165cb75cbca63fb912aa/app/src/main/java/io/github/vvb2060/packageinstaller/model/Hook.kt#L79-L117) 和调用点 [`InstallRepository.kt`](https://github.com/vvb2060/PackageInstaller/blob/3d113a5e000c62a712e6165cb75cbca63fb912aa/app/src/main/java/io/github/vvb2060/packageinstaller/model/InstallRepository.kt#L60-L82)。

这有三层问题：

1. 它是持久全局安全设置写入，不是单次 PackageInstaller 参数。
2. AOSP 中该设置面向 ADB verifier；华为九宫格可能来自独立安装控制服务，二者不能等同。
3. 当前 PCT 的 `settings get global verifier_verify_adb_installs` 返回 `null`。把它写为 0 会扩大本次任务的安全边界，却没有目标机证据证明有效。

所以不能把该项目宣传的“Bypass Play Protect”当作 Shizuku 天然能力，更不能在未经独立批准时复制这一步。

## 各路径的等价性与价值

| 路径 | 服务端 calling UID | 最终 API | 依赖 | 是否改变 ADB 来源 | 本项目判断 |
| --- | --- | --- | --- | --- | --- |
| `adb install` | 2000 | PackageInstaller session | adb 客户端 | 否 | 最短，但之前已触发厂商门 |
| `cmd package install-*` | 2000 | PackageInstaller session | system shell command | 否 | 无代码基线，适合记录 session |
| 一次性 `app_process` DEX | 2000 | 直接 `IPackageInstaller` | 自有 DEX、隐藏 AIDL stubs | 否 | 仅作为 parser/client A/B 与完整回调采集 |
| ADB 启动 Shizuku | 2000 | 转发 `IPackageInstaller` | 已安装 Manager、授权与 server | 否 | 当前设备缺 Manager；不是绕过 |
| root Shizuku / Sui | 0 | 转发 `IPackageInstaller` | root / Magisk | 身份不同 | 当前设备不适用 |
| vvb Shizuku installer | 2000 | 转发 PackageInstaller + Settings 写入 | Shizuku Manager | 否；另改 verifier | 不复制全局设置写入 |

从最小实现和性能看，Shizuku 的 Binder 复用对于长期、多调用 app 很成熟；但对“由开发机更新一次 Atha”而言，新增 Manager、授权 UI 和常驻 server 没有收益。`cmd package` 已经是无自研代码的 session 入口。一次性 DEX 只有在需要拿到更精确的 params、session id 和 commit callback 时才值得做。

## 一次性 DEX 能做什么，不能做什么

如果后续获得明确的真机写入批准，可构建一个 API 29 单用途 helper：

1. 运行前只读核对目标 package、当前 user、已装版本、签名证书摘要和待装 APK 摘要；不尝试降级或签名不一致更新。
2. 把 helper DEX 与待装 APK 放入 `/data/local/tmp`，以 `CLASSPATH=<helper.dex> app_process /system/bin <main-class>` 启动。
3. 从 `ServiceManager` 取得 package service，调用 `IPackageManager.getPackageInstaller()`，创建 full-install session，写入 `base.apk`，用 `LocalIntentReceiver` 接收 commit 状态。
4. 记录实际 caller UID、user、session id、服务端返回的 flags、installer package、status、legacy status、message 和 pending-user-action Intent component。
5. 一旦出现 `STATUS_PENDING_USER_ACTION`、华为安装 Activity、凭据 UI 或无法解释的 vendor 分支，立即停止；不注入按键、不代填图案、不修改 verifier / 安全设置。

未执行伪代码只应表达结构，不应包装成“静默安装命令”：

```text
shell app_process
  -> ServiceManager["package"]
  -> IPackageManager.getPackageInstaller()
  -> createSession(params, installerPackageName, userId=0)
  -> openSession(sessionId)
  -> write("base.apk")
  -> commit(localIntentSender)
  -> record status; stop on pending user action
```

它不能：

- 把 UID 2000 变成 UID 1000 或可信商店 UID；
- 移除服务端强制加入的 `INSTALL_FROM_ADB`；
- 绕开 SELinux 或华为闭源安装控制；
- 保证不弹图案；
- 在不匹配签名或不允许降级时合法更新。

## 受控 A/B 的最小设计

在用户明确批准真机安装写入后，先比较同一 APK 的两个入口，不先安装 Shizuku：

| 组 | 调用入口 | 目的 |
| --- | --- | --- |
| A | `cmd package install-create/write/commit` | 无自研代码的 UID 2000 基线 |
| B | 一次性 DEX 直接 Binder session | 排除 shell command 参数解析与回调丢失 |

两组都必须固定：同一待装 APK 哈希、同一已装版本与签名、同一 user 0、同一设备状态；每次只做一组，出现交互即停。建议捕获：

- `dumpsys package` 中安装前后的版本、签名摘要与 installer；
- 创建后的 session info 与 flags，重点看 `INSTALL_FROM_ADB`；
- commit 回调的 status / message / pending Intent；
- `dumpsys activity activities` 的前台 component；
- 只过滤 PackageInstaller / PackageManager 的 logcat，不记录或传输用户凭据。

若 A、B 都进入同一个华为验证界面，说明差异不在 adb 客户端或 `pm` parser；应停止继续造包装层。若 B 的行为不同，也只能说明存在入口差异，仍需保存上述证据后分析，不能直接把一次成功描述为安全策略已被普遍绕过。

## 失败信号与停止条件

| 阶段 | 明确信号 | 动作 |
| --- | --- | --- |
| 官方 starter | `fatal: can't get path of manager` | 确认 Manager 缺失后停止，不尝试伪造已安装状态 |
| Shizuku server | `MANAGER_APP_NOT_FOUND` | 停止；说明裸 server 不成立 |
| create/open/write | `SecurityException`、无效 user、session 创建或写入失败 | 记录原始错误，abandon session |
| commit | `STATUS_PENDING_USER_ACTION` | 记录 Intent component，停止自动化 |
| 华为 UI | 安装器、系统管理器或凭据 Activity 前台 | 不输入、不点击、不更改设置 |
| 包校验 | 签名不一致、版本降级、split 缺失 | 停止，不用卸载清数据掩盖问题 |
| 来源标记 | `INSTALL_FROM_ADB` 或 `hw_adb_install=true` | 视为预期的 UID 2000 结果，不再声称“非 ADB” |

## 回滚边界

- **commit 前：**关闭流、abandon session，删除 `/data/local/tmp` 中本轮 helper 和 APK；这部分可完全回滚。
- **commit 失败：**确认 session 已 abandoned，再清理临时文件；不要清应用数据，不要改全局 verifier。
- **commit 成功：**更新本身不是无条件可逆。回到旧版需要同签名旧 APK、系统允许降级，并可能仍触发厂商验证；卸载会造成数据丢失，不能作为自动回滚。
- **开始前留证：**保存当前 APK path、versionCode/versionName、签名证书摘要和已知可用的旧构建产物。不能假定从 `/data/app` 拉出的 split 集合一定能在所有状态原样恢复。

## 项目建议

1. 不为这次更新先安装 Shizuku。它会先触发一次同类安装，还不会改变 UID 2000 来源。
2. 把“能调用 PackageInstaller”与“华为允许无凭据确认”分成两个验收项。前者已有源码和只读设备证据，后者尚未验收。
3. 获批真机写入后，先跑 `cmd package` 与一次性 Binder helper 的单次 A/B；出现 UI 立即停，不自动处理九宫格。
4. 若两个入口等价，结束这一研究线，不继续堆叠 Shizuku wrapper。后续发布体验应选择明确的人工确认安装，或研究华为官方企业设备管理 / MDM 能力，而不是把普通 ADB shell 包装成“静默”。
5. 若未来 Atha 本身需要长期调用多种 shell API，再评估 Shizuku 集成；那是产品能力和授权 UX 的独立 change，不应与开发包更新绑在一起。

## 来源

- [Shizuku 官方介绍](https://shizuku.rikka.app/introduction/)
- [Shizuku 官方启动指南](https://shizuku.rikka.app/zh-hans/guide/setup/)
- [Shizuku starter 固定源码](https://github.com/RikkaApps/Shizuku/blob/b844bc491f1790c72328e1a8e5b2349f8978f0ea/manager/src/main/jni/starter.cpp)
- [Shizuku server 固定源码](https://github.com/RikkaApps/Shizuku/blob/b844bc491f1790c72328e1a8e5b2349f8978f0ea/server/src/main/java/rikka/shizuku/server/ShizukuService.java)
- [Shizuku API Binder 转发固定源码](https://github.com/RikkaApps/Shizuku-API/blob/a27f6e4151ba7b39965ca47edb2bf0aeed7102e5/server-shared/src/main/java/rikka/shizuku/server/Service.java)
- [Sui 固定源码 README](https://github.com/RikkaApps/Sui/blob/2f5fd2a04bc061eb2a8431cc3ede9066954f5a7c/README.md)
- [vvb2060/PackageInstaller 固定源码](https://github.com/vvb2060/PackageInstaller/tree/3d113a5e000c62a712e6165cb75cbca63fb912aa)
- [AOSP Android 10 PackageInstallerService](https://android.googlesource.com/platform/frameworks/base/+/refs/tags/android-10.0.0_r47/services/core/java/com/android/server/pm/PackageInstallerService.java)
- [AOSP Android 10 PackageInstallerSession](https://android.googlesource.com/platform/frameworks/base/+/refs/tags/android-10.0.0_r47/services/core/java/com/android/server/pm/PackageInstallerSession.java)
- [Android Debug Bridge 官方文档](https://developer.android.com/tools/adb)

华为 PackageInstaller 结论来自当前 PCT-AL10 系统 APK 的本地只读静态分析，没有可引用的厂商公开源码；它在本文中被明确标为“目标机静态证据”，不冒充官方保证或已完成的安装验收。

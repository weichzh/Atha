# Android 系统栏、键盘与正文选区

## Status

implemented

## Problem

Android 阅读壳已有 edge-to-edge 和 IME 避让，Svelte 入口只在开书返回按钮读取了系统安全区；书架、正文控制条和选区菜单未共享这套边界。正文长按时系统浮动菜单与 Atha 的选区操作重叠；软键盘收起后搜索框仍保留焦点。

## Scope

- 从 `188df1a` 独立拆分已授权交付，保留 #8 的开书、原生返回和 C4 书架往返、#11 的页内脚注返回；不依赖书架多选分支。
- 只改 Android/Svelte 的安全区、系统栏状态、搜索框 IME 收起和正文选区交接。外观设置、笔记、翻页实现、电量显示与发布不在本切片。

## Architecture Impact

present

- 由 Android host 消费 IME insets；由同源主框架消息桥将可用视区传给壳，不给子框架、外部页面或书籍资源协议授权。
- 仅有实际正文选区时让位给 Atha 选区菜单；Android 原生手柄与浮动 ActionMode 生命周期保留。普通控件、编辑器和非正文菜单仍由平台负责。

## Acceptance Criteria

- 书架和阅读控件避开安全区；阅读沉浸隐藏状态栏，打开控制条、弹层或返回书架时恢复且不重排正文。
- IME 仅避让一次；键盘收起让当前搜索框失焦但保留查询，不关闭弹层、不清编辑器焦点。
- 正文选区交给现有 SelectionPill；正文之外不隐藏原生菜单；脚注原生返回和书架往返继续可用。

## Files And Steps

关联 [Issue #1](https://github.com/weichzh/Atha/issues/1)。先收紧 Activity 的桥与选区范围，再接壳、阅读器状态及安全区样式，最后检查 Android 编译和浏览器交互。

## Approval

用户授权拆分和本地验证；此切片不安装、不推送、不改原工作区。

## Checks

- `mise exec -- pnpm --dir reader/app check`：0 错误、0 警告；`build` 通过。Node 回归 154 项通过。
- `node reader/app/tests/android-reader-insets.browser.mjs <production-preview-url>`：390 × 844 CSS px、DPR 2、减少动态，使用模拟 AndroidX 消息端口，覆盖四边 inset、错误载荷、正文按实际视区分页、系统栏显隐指令、搜索框 IME 去焦点与普通编辑器保焦、正文选区命令、脚注关闭及同文档返回焦点。
- 既有 `reader-app.browser.mjs` 通过，复验无原生桥的开书 / 返回、脚注和阅读记忆路径。
- Android 本机 Kotlin：Temurin 21.0.12、compile SDK 36，`:app:compileX86_64DebugKotlin :app:compileX86_64ReleaseKotlin` 通过；使用与锁定版本对应的 Tauri/Wry 生成源码。未运行 APK 打包、安装或设备自动化。
- `bash scripts/check-docs.sh` 与 `git diff --check` 通过；Markdown 由全局 Git 提交钩子处理。

## Result

- 安全区在显示或隐藏状态栏时采用同一份稳定 inset；IME 上升时宿主缩短 WebView，并扣除已由 IME 避让的底部 inset。开书返回、书架、阅读框、脚注及选区定位共享 CSS 边界。Svelte 正文分页用真实阅读框高度；独立静态阅读入口继续使用原默认高度。
- 原版的减少动态样式给所有元素设置了非零过渡时间，导致高 DPR 开书时阅读框在图片自检期间改变尺寸。在 390 px、DPR 2 下复现 `image-load` 后，将 CSS 过渡时间改为 0；同一开书、脚注和返回链路通过。
- WebView 将正文选区交给 Atha 菜单时，只暂时隐藏属于该 WebView 的浮动 ActionMode 的菜单项；保留 callback、选区生命周期和原生手柄。输入或移出正文时恢复。状态栏与选区命令分开，正文之外不主动静默原生菜单。

## Review

- 校对了固定 Tauri/Wry 的入口加载顺序、应用导航限制与 CSP。AndroidX 只在 `https://tauri.localhost` 注入消息对象，宿主只接收该 WebView 主框架的列举指令；书籍资源协议、外部来源与子框架不具备权限。不支持该 AndroidX 能力时使用原生安全区边距，保持 Android 同文档开书和默认菜单。
- AndroidManifest 原有 `adjustResize` 保留。#8 的返回分发与 #11 的脚注优先级保持原有路径。无 #13 或其他未合入分支依赖。

## Evidence And Residual Risks

浏览器证据覆盖 Svelte/JS 与布局，Android 证据为 Kotlin 和 Android 资源编译。真实 WebView 尚未验证状态栏瞬时行为、IME 事件顺序、原生手柄拖动及非正文 ActionMode；获准安装候选后，用同一公开样本在 Android 上回归这些场景后再签收。

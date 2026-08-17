---
description: 阅读内容缩放、书架菜单与封面、设置入口、主题和软键盘适配的统一变更记录。
---

# 阅读交互与书架整理

## Status

implemented

## Problem

图片、表格和公式预览缺少触控与桌面连续缩放；书架的选择和书籍操作依赖显式按钮，首次导入又可能在准备完成前显示无封面。阅读设置入口位于顶栏但面板从底部展开，应用壳与阅读页的主题颜色分散在多个 CSS 文件，手机软键盘还会遮住标注笔记和消息输入器。这些问题共同破坏了输入方式、入口位置和视觉状态的一致性。

## Scope

- 图片、表格和公式预览支持双指缩放，桌面支持滚轮缩放，并保留按钮和键盘缩放；
- 移动端长按书籍进入既有选择模式并显示底部操作栏；桌面右键和键盘上下文菜单显示单书菜单；
- 单书菜单可更换或恢复封面；首次导入在返回书架前尽力完成同一准备流程，使内置封面立即可见；
- 阅读设置从顶栏移到底栏，仍使用同一个设置面板和偏好状态；
- 新增唯一应用主题调色板，书架与阅读壳只消费语义 token，不再各自维护明暗主题颜色；
- 标注笔记 dialog 与消息输入器共用 `VisualViewport` 可视区域，在软键盘出现时保持编辑器和操作按钮可见；
- 更新本地资料备份、事实所有者和受影响的正式检查。

## Non-goals

- 不新增封面裁剪、在线搜索、自动封面识别、书籍分组或同步；
- 不改变书内正文缩放、分页缩放、阅读偏好 schema、消息正文 schema 或输入法实现；
- 不复制 Readest 的 React 状态结构、完整菜单项、元数据编辑器或图片查看器架构；
- 不在本次范围安装真机候选、推送或发布。

## Architecture Impact

present

- `Library/` 增加严格命名、受限图片类型和尺寸的自定义封面文件；它由 `LocalLibrary` 校验和读取，并进入 `.atha-data` 完整备份。ADR-0011 记录该持久化边界。
- `reader/theme.css` 成为应用壳和阅读壳颜色 token 的唯一事实所有者；功能 CSS 只保留布局和语义引用。

## Acceptance Criteria

- `INTERACTION-ZOOM-01`：图片、表格和公式预览可在 0.5–4 倍间双指缩放；桌面滚轮在预览内连续缩放，单指滚动、关闭和既有按钮 / 键盘缩放可用。
- `INTERACTION-SHELF-01`：触控长按 500 ms 进入选择模式，移动超过 10 px 会取消；桌面右键及 `ContextMenu` / `Shift+F10` 打开单书菜单，菜单可由 Escape 或点击外部关闭。
- `INTERACTION-SHELF-02`：选择模式底栏保留移出和删除，并在单选时提供更换封面；菜单可打开、选择、移出、删除、更换封面和恢复内置封面，破坏性操作继续确认。
- `INTERACTION-COVER-01`：首次导入的有效内置封面在导入完成后立即显示；JPEG、PNG、WebP 自定义封面通过后端类型、大小和像素边界校验，重开、备份恢复和恢复内置封面后状态正确。
- `INTERACTION-SETTINGS-01`：阅读顶栏不再显示设置，底栏显示设置且从同一底部面板打开；窄屏和桌面均无入口与面板方向冲突。
- `INTERACTION-THEME-01`：应用壳和阅读壳的系统 / 浅色 / 深色 / 纸张状态由共享语义 token 管理；受控 CSS 检查会拒绝功能样式重新引入颜色字面量。
- `INTERACTION-KEYBOARD-01`：可视视口缩小时，标注笔记 dialog 及普通、全屏消息输入器均位于软键盘上方，列表可滚动且当前输入与提交操作保持可见；无 `VisualViewport` 时保持既有布局。
- `INTERACTION-REGRESSION-01`：受影响的 Rust、Svelte、Node、Linux Tauri / WebKitGTK 和文档检查通过，无新增控制台、网络、隐私或横向溢出错误。

## Files And Steps

1. 在现有 `ContentDialog` 增加原生 wheel / touch 事件，不建立第二个 viewer。
2. 复用书架现有选择状态和底部操作栏，增加输入方式适配及一个原生上下文菜单。
3. 由 `LocalLibrary` 拥有自定义封面校验、持久化、备份和读取；Tauri 只负责可信窗口 picker 与 command。
4. 把同一个阅读设置 `details` 移到底栏；抽取共享主题 token，功能样式不再声明调色板。
5. 用 `VisualViewport` 的 resize / scroll 更新共享可视高度、顶部和中心，使标注笔记 dialog 与消息层使用同一几何事实。
6. 扩展最小后端、前端和 Linux GUI 检查，完成独立 Standards / Spec 复审。

## Checks

- `mise exec -- cargo test --locked -p atha-backend --test local_data`；
- `mise exec -- cargo test --locked -p atha-reader-app --no-run`；
- `mise exec -- pnpm --dir reader/app check`；
- `mise exec -- pnpm --dir reader/app build`；
- `mise exec -- node --test reader/app/tests/library.test.ts`；
- `mise exec -- node --test reader/web/*.test.mjs`；
- `bash scripts/check-reader-linux.sh`；
- `autocorrect --fix/--lint` 仅针对本次中文 Markdown；
- `project_workflow.py station reader-interaction-shelf-theme --activity verification --gate docs`。

## Result

图片、公式和表格继续共用 `ContentDialog`，新增 0.5–4 倍双指与滚轮连续缩放，不建立第二套 viewer。书架复用既有选择状态：500 ms 触控长按进入底部操作栏，桌面右键与键盘上下文键打开单书菜单；菜单和单选底栏均可更换封面，自定义封面由 `LocalLibrary` 校验、崩溃恢复、备份和回退。导入 worker 在返回书架前复用 `open_book`，冷导入即可取得 importer 封面。

阅读设置复用原面板并统一移到底部第五个入口。`reader/theme.css` 成为唯一颜色调色板，同时进入文档、书籍 Shadow DOM 和 Snapshot；运行时把系统明暗统一解析为 `data-*-tone`，不再复制系统主题调色板，功能 CSS 的颜色字面量检查作为回归门。标注笔记 dialog 与消息层共用 `VisualViewport` 的高度、顶部和中心，软键盘出现时当前输入及保存 / 发送操作保持在可视区域。

## Review

第一轮 Standards / Spec 复审发现三项实现或验证缺口：共享主题最初没有进入书籍 Shadow DOM 与 Snapshot，直接 Wry / Tao Windows 基线也未提供 `theme.css`；冷导入封面检查在移出书架后复用了旧缓存，不能证明首次准备。最终实现让 `content.mjs` 同时加载主题与阅读样式、为 Windows 受控协议增加主题资源，并在冷导入检查前物理清除同身份缓存。WebKitDriver 不产生可观察的右键 `contextmenu`，正式门改为合成同一 DOM 事件验证菜单状态，未把驱动限制写成产品能力。

第二轮 Standards / Spec 复审又发现三项阻塞：真实的标注笔记 dialog 未接入可视视口；移出书架先删自定义封面，记录删除失败时会丢失封面；共享主题仍复制系统明暗分支。最终修正把可视视口变量提升到阅读根，覆盖标注笔记和消息输入；先耐久删除书架记录，再清理封面，并在启动时回收中断留下的孤立封面；应用壳、阅读壳和 Snapshot 均把明暗状态解析为单个 tone 后消费唯一调色板声明。

修正后重新检查全部变更、所有 `LibraryError` 分类、资料备份允许列表、书架输入路径、设置位置、颜色字面量、软键盘几何和旧 host 资源映射，未发现剩余 Standards / Spec 阻塞项。

## Evidence And Residual Risks

- 静态 / 本地：`cargo fmt --all -- --check`、`cargo clippy --workspace --all-targets --locked -- -D warnings` 和 `cargo test --workspace --all-targets --locked` 通过；完整 Rust 测试无失败，私有 fixture 专项按声明忽略。
- 静态 / 本地：`pnpm --dir reader/app check` 为 0 error / 0 warning，production build 通过，资料库与阅读 Web Node 检查均为 13 / 13 通过；功能 CSS 没有颜色字面量或重复系统主题媒体分支。
- Linux 真实壳：`bash scripts/check-reader-linux.sh` 在 WebKitGTK 0.55.1 通过。隔离冷导入立即显示封面；360–1600 px 视口无横向溢出；右键菜单和 500 ms 长按底栏状态正确；图片预览滚轮到 127%、合成双指到 160%；阅读设置只在底栏；模拟 320 px 高可视视口时标注笔记、保存操作、对话层与消息输入器均位于可视区；220 次既有可信鼠标手势和 AppLog 隐私检查继续通过。
- 边界：Linux 门的长按、双指和右键 DOM 事件是合成输入，`VisualViewport` 用可控几何验证；尚未在 PCT-AL10 用自然手指和真实输入法复核，也未在 Windows WebView2 实际运行新增 `theme.css` 路由。未安装真机候选、推送或发布。

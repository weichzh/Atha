# R3 排版与阅读偏好

## Status

implemented

## Problem

R2 已稳定内容位置与导航，但字号仍是 pagination 的临时验证参数，主题只能跟随系统，行距、页边距、字体、书源样式和用户覆盖层没有统一所有者。继续把这些参数分别写进 CSS、控件或会话，会让后续恢复和每书状态再次重构。

Readest 的全局与单书 view settings 合并顺序值得采用，但不复制其全局 store 和巨型 viewer。微信读书研究表明常用排版只需少量应用级选项，深度 CSS 应作为默认按书生效、可停用和撤销的逃生舱。

## Scope

- 新增一个内存中的 Preferences module：应用默认值拥有主题、字号、字体和阅读密度；本书覆盖只拥有书源样式开关与用户 CSS；
- 主题支持跟随系统、亮色和暗色；字体支持书源、衬线和无衬线；阅读密度以紧凑、标准、舒展三档同时设置绝对像素页边距与行高；
- 排版变化统一经过 Navigation 捕获 Locator、应用偏好、等待稳定布局并恢复内容位置；不直接复用旧页码；
- 书源 CSS、Atha 阅读样式、用户 CSS 保持固定层叠顺序；用户 CSS 只作用于封闭书籍 Shadow DOM，拒绝 `@import`、`url()` 和 Shadow 边界选择器；
- 使用一个原生“阅读偏好”局部面板即时预览，提供应用默认恢复、本书样式启停、用户 CSS 应用与本书恢复；不设计最终沉浸控制层；
- 正式 WebView2 验证每种主题、字体、密度和字号不裁切内容、不破坏 Locator，书源样式和用户 CSS 可启停、错误可恢复，既有安全与 benchmark 保持通过。

## Non-Goals

- 不持久化应用或每书偏好；R5 与耐久位置一起确定本地保存边界；
- 不实现 JavaScript 扩展、远程样式、样式社区、导入文件或自动修书；
- 不增加自由边距/行距滑杆、亮度、翻页动画或第二种页面模式；三档密度不足时再扩展；
- 不重做目录、进度、引用入口或最终控制层视觉；
- 不增加状态库、renderer adapter、配置数据库或跨平台抽象。

## Acceptance Criteria

- [x] Preferences 严格验证并合并应用默认值与本书覆盖，两个范围职责明确且均可恢复默认；
- [x] 系统/亮/暗主题、书源/衬线/无衬线字体、24/32/40px 字号与三档密度均即时生效；页面仍使用固定设备像素，系统 DPI 只缩放显示层；
- [x] 每次排版变化后，变化前 Locator 仍位于当前页面，文字、公式、图片、表格和代码无裁切；
- [x] 书源样式可以停用再恢复；安全用户 CSS 可以应用、停用和清空，危险 CSS 被拒绝且不会破坏当前阅读会话；
- [x] 原生偏好面板键盘可达、状态与动作相邻，关闭面板后不占用阅读页几何；
- [x] 四样本实际 host、明暗主题、安全断言、Rust 检查和 benchmark 保持通过；
- [x] 独立规格与标准 review 无 blocking，事实所有者和 `ACTIVE` 与最终实现一致。

## Files And Steps

1. 固定应用默认值、本书覆盖与有效偏好的领域词汇和验证边界；
2. 复用现有 Shadow DOM 三层 style，补上安全、可撤销的书源与用户样式控制；
3. 让 Navigation 成为所有排版重排的单一串行入口，并用原生局部面板驱动；
4. 扩展真实诊断，覆盖主题、字体、密度、字号、样式层、Locator 恢复与错误后继续操作；
5. 运行页面、Rust、实际 host、Agent Browser、benchmark、文档和独立 review，更新事实所有者并关闭 R3。

## Checks

- 所有页面 JavaScript module 的 `node --check`；
- `cargo fmt --all --check`、`cargo clippy --workspace --all-targets --locked -- -D warnings`、`cargo test --workspace --all-targets --locked`；
- `pwsh -NoProfile -File scripts/check-reader-samples.ps1`；
- `pwsh -NoProfile -File scripts/check-reader-slice.ps1`；
- workflow `docs` gate；
- 本次中文 Markdown 的 `autocorrect --fix` 与 `autocorrect --lint`；
- `git diff --check`。

## Rollback

回滚本 change 的提交即可恢复 R2 默认排版；R3 不迁移或写入用户数据。

## Approval

用户明确授权依据现有 Readest 与微信读书研究继续实现到 M2 结束，并要求缺少规格时补规格。本 change 只落实路线图 R3。

## Result

已实现内存 Preferences module、原生局部偏好面板和书源/Atha/用户三层样式。应用默认值只拥有主题、字号、字体和密度，本书覆盖只拥有书源样式开关与安全 CSS；Navigation 在同一串行入口中恢复排版前 Locator。书内 style、stylesheet link 与 inline style 均由书源样式开关统一控制，外链与内联 CSS 保留原 DOM 顺序。没有增加持久化、JavaScript 扩展、状态库或自由参数面板。

## Review

- Spec：初检发现用户 CSS 缺少独立启停、行高证据与书内 style/link 未纳入书源层等 blocking；逐项修复后复审无 blocking、无 non-blocking；
- Standards：初检发现重复字号绑定、CSS 转义绕过、inline style 生命周期、手动暗色图片、字体稳定与验证缓存释放等 blocking；修复后再消除书源 CSS 顺序偏差，最终复审无 blocking、无 non-blocking。

## Evidence And Residual Risks

- 静态与本地证据：八份页面 module 语法、Cargo fmt/clippy/test、资源与遥测 3/3、host 参数 2/2 通过；
- 真实目标证据：四样本实际 Windows host 与 Agent Browser 明暗主题通过；模块诊断覆盖全部主题、字体、字号和密度，书源与用户样式启停、危险 CSS 拒绝、拒绝后的队列恢复、Locator 可见与无裁切；实际原生面板完成密度修改和应用默认恢复，开合前后固定页面几何不变；
- 性能证据：10 次样本中位数为冷启动 885.044ms、首个稳定页 209.500ms、热打开 20.800ms、翻页 6.250ms、字号重排 27.800ms；没有同时间旧代码对照；
- 当前偏好只在单次进程中生效；R5 再按内容版本持久化应用默认、本书覆盖和最后稳定位置；
- 用户 CSS 采用浏览器 CSSOM parser 与明确安全禁项，包含转义后的危险函数与 Shadow 边界选择器；不尝试修复部分无效规则，自定义 JavaScript 仍不在信任边界内。

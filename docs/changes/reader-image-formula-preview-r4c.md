# R4C 图片与公式预览

## Status

implemented

## Problem

R4B 已闭合文本、链接与脚注，但正文图片和公式仍只能按分页后的缩放尺寸查看。较大的插图或公式被缩放到页宽后无法单独检查，图片也没有键盘焦点和可操作语义。直接复用书内 HTML 或恢复资源默认导航会把不可信内容带入应用壳，并绕过既有书根边界。

Readest 的图片查看链路证明独立预览是必要能力，但其图集、保存、缩放、拖拽和跨平台手势超出当前 reader-only 范围。Atha 只复用浏览器图片解码、现有受控 URL 和 R4B 原生 dialog，先交付可关闭的适配窗口预览。

## Scope

- 书内非链接图片获得键盘焦点、按钮语义和可见焦点状态；单击、Enter 或 Space 打开预览；
- 普通图片和公式都在 R4B 的原生 dialog 中显示同一受控资源，普通图片保留原色，公式跟随明暗主题；
- dialog 标题和图片替代文本只使用安全文本，关闭后焦点回到触发图片；
- 链接包裹的图片继续作为链接处理，不产生嵌套交互目标；
- 预览不改变正文 DOM、分页、Locator、资源许可或网络策略。

## Non-Goals

- 不实现图集、前后图片、原始像素缩放、平移、双击缩放、触摸捏合或旋转；
- 不实现保存、分享、复制图片、OCR、图片注释或右键菜单；
- 不复制书源 HTML、样式或脚本到应用壳；
- 不处理表格和代码预览；留给 R4D。

## Acceptance Criteria

- [ ] 普通图片可用鼠标和键盘打开适配窗口预览，dialog 可由 Escape 或关闭按钮退出并恢复焦点；
- [ ] 公式可用同一路径预览，明暗主题下保持与正文一致的可读处理，普通图片不被反色；
- [ ] 链接图片仍只进入链接策略，预览资源仍是已校验书根 URL，未产生外部请求或活动 HTML；
- [ ] 打开和关闭预览不翻页、不改变 Locator 或 section，图片焦点状态具备基本读屏语义；
- [ ] 四样本实际 host、明暗浏览器、Rust 检查和 benchmark 保持通过；
- [ ] 独立规格与标准 review 无 blocking，事实所有者和 `ACTIVE` 与最终实现一致。

## Files And Steps

1. 在内容完成安全校验时为非链接图片补最小操作语义，不改变资源 URL；
2. 扩展现有内容动作与原生 dialog，以独立图片元素显示普通图片或公式；
3. 扩展真实诊断，覆盖鼠标、键盘、焦点、明暗过滤、链接图片互斥和零网络变化；
4. 运行实际 host、四样本、benchmark、文档 gate 和独立 review。

## Checks

- 所有页面 JavaScript module 的 `node --check`；
- `cargo fmt --all --check`、`cargo clippy --workspace --all-targets --locked -- -D warnings`、`cargo test --workspace --all-targets --locked`；
- `pwsh -NoProfile -File scripts/check-reader-samples.ps1`；
- `pwsh -NoProfile -File scripts/check-reader-slice.ps1`；
- workflow `docs` gate；
- 本次中文 Markdown 的 `autocorrect --fix` 与 `autocorrect --lint`；
- `git diff --check`。

## Rollback

回滚本 change 的提交即可恢复 R4B 行为；不涉及耐久数据、资源迁移或外部写入。

## Approval

用户明确授权依据当前路线图继续实现到 M2 结束，并要求缺少规格时补规格。本 change 只实现 R4 剩余内容中的图片与公式预览。

## Result

复用现有 `content` 安全校验和原生 `content-dialog` 完成图片与公式预览。非链接图片在进入 Shadow DOM 前获得按钮语义、`tabindex` 和安全操作标签；链接图片移除冲突的按钮语义并继续进入既有链接策略。内容动作按链接优先级处理单击，并支持图片 Enter 与 Space；预览只设置独立图片元素的既有受控 URL、标题和替代文本，关闭后清理资源并恢复触发焦点。

预览没有增加依赖或新模块，也没有复制书源 HTML。普通图片保持原色；公式在系统或显式暗色主题下沿用正文反色。诊断验证了资源一致性、主题过滤、链接图片互斥，以及 dialog 前后的 section、页码和 Locator 不变；正式 Agent Browser 验收另使用真实鼠标、Space、Enter 与 Escape 验证打开、关闭和焦点返回。

## Review

- Spec：独立复审无 blocking；初次提出的链接策略计数、真实鼠标/键盘证据和冲突 ARIA 状态均已收紧；
- Standards：初次 blocking 指出 `aria-hidden` 会与新增按钮语义冲突；已在同一图片归一化入口移除 `aria-hidden`、`aria-disabled`，使用最长 160 字符的 `alt` 生成图片或公式标签；最终复审确认 blocking 清零。

## Evidence And Residual Risks

- 最高证据等级：Windows 真实目标证据；
- `scripts/check-reader-samples.ps1 -BasePort 19400` 通过四个样本的实际 WebView2 host 与 Agent Browser 明暗验收；《数学及其历史》首章验证 23 个公式与 2 张普通图片；真实鼠标、Space、Enter、Escape 与焦点返回均通过；
- 等待图片解码后，`scripts/check-reader-samples.ps1 -Manifest .tmp/r4c-math-samples.json -BasePort 19440` 再次通过《数学及其历史》明暗视觉验收；临时 manifest 已删除；
- `scripts/check-reader-slice.ps1` 通过 Rust fmt、clippy、test、实际 host 自检与 10 样本 benchmark；run `1785695361880-36444` 的中位数为冷启动 869.154ms、首个稳定页面 213.350ms、热打开 20.800ms、翻页 6.200ms、字号重排 27.850ms；
- 首次 manifest host 验证暴露自检误把 XHTML 当图片资源并触发 `undeclared-resource`；修正为直接检查图片交互语义后，单本真实 host 与四样本总检均通过；
- 未执行旧提交的同时间对照，性能数据只证明当前正式门槛保持通过；
- 图集、缩放、平移、保存、OCR、表格和代码交互仍按 Non-Goals 保留。

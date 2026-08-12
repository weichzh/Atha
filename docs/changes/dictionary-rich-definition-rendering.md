---
description: 离线词典安全富文本释义与内建词典排版。
---

# 词典富文本释义

## Status

implemented

## Problem

当前后端把 MDict 与经典 Kindle 词条统一压成单行纯文本，前端也只用文本节点显示，词性、音标、段落、义项、例句、列表和表格等原有结构全部丢失。用户已明确要求词典具备充分样式，不能继续使用纯文本呈现。

## Scope

- 后端继续以现有 `dom_query` 解析释义，同时输出兼容旧契约的纯文本回退和经过固定元素白名单净化的富文本；
- 移除脚本、样式、表单、嵌入、媒体、图片、资源地址、事件属性和来源 CSS，只保留无活动能力的词典语义结构；
- 前端在现有词典面板呈现词头、词性 / 音标提示、段落、义项、例句、列表、引用、表格、上下标、ruby 与代码等内建排版；
- 保持离线、精确查词、隐私日志、MDD 资源边界和桌面浮层 / 移动底部抽屉不变。

## Non-Goals

- 不加载词典自带 CSS、图片、音频、字体、脚本、网络地址或可点击外链；
- 不新增 sanitizer 依赖、provider、缓存、搜索方式或词形查询；
- 不为单一私有词典建立格式专属模板或复制其受版权保护的内容。

## Architecture Impact

present

- Design purpose: 在不放宽不可信词典边界的前提下，恢复词条的语义层级和可读排版。
- Drivers / quality scenarios: `A-DICT-STYLE-01` 要求常见词典结构不再退化为单行文本；`A-DICT-SEC-01` 要求富文本仍不能执行代码、加载资源、导航或影响词典容器之外的界面。
- Modules / interfaces: `reader::dictionary` 拥有净化并同时生成 `definition` 与 `definitionHtml`；Tauri 继续透传结构化结果；Svelte 面板只渲染后端固定白名单结果；`atha-reader.css` 拥有应用内词典排版。
- Candidate and tradeoffs: 复用当前 `dom_query` 并使用固定 HTML 元素白名单，不增加 sanitizer 包；相比保留来源 CSS，这会舍弃词典品牌视觉，但安全边界更小且跨词典一致。纯文本字段保留为兼容和无障碍回退。
- Evidence / review trigger: 公共单测必须证明结构保留、危险节点与全部来源属性删除；Svelte check / build、词典正式门和桌面 / 移动视口检查通过。只有实际词典证明内建语义排版不足时，才另行评估受限 MDD 资源协议。

## Acceptance Criteria

- [x] 普通文本、段落、强调、词性、音标、义项、例句、列表、引用、表格、ruby 和上下标有清晰且一致的层级；
- [x] `script`、来源 `style`、表单、嵌入、媒体、图片、链接地址、事件属性、ID、class 和任意内联样式均不能进入渲染结果；
- [x] 旧 `definition` 纯文本语义和私有输出哈希不变，新 `definitionHtml` 只包含固定白名单；
- [x] 桌面与移动词典面板可滚动、无文本遮挡，深浅阅读主题下可读；
- [x] 相关 Rust、词典正式门、Svelte、文档与 diff 检查通过。

## Files And Steps

1. 扩展后端安全释义投影并补一个覆盖结构保留与活动内容拒绝的单测。
2. 扩展查词结果类型，在现有面板显示可见词头和安全富文本。
3. 为词典语义元素增加一套受容器约束的响应式排版。
4. 更新词典事实所有者，运行正式检查并完成独立 review。

## Checks

- `bash scripts/check-dictionary-source.sh --private-fixtures fixtures/local`；
- `mise exec -- pnpm --dir reader/app check`；
- `mise exec -- pnpm --dir reader/app build`；
- `bash scripts/check-docs.sh`；
- `git diff --check`；
- `agent-browser` 桌面与移动视口检查。

## Approval

用户于 2026-08-13 明确要求“词典要有充分的样式，不能纯文本”，批准本文件限定的安全富文本与内建排版范围。

## Result

`DictionaryLookup` 现在同时返回既有 `definition` 和新的 `definitionHtml`。后端复用 `dom_query` 删除活动或资源节点，把未知元素降为 `span`，清除全部来源属性，并只为常见词典角色生成受控标记；普通文本统一包装为段落。富文本过滤只剩空容器时，前端回退到既有纯文本字段，不会显示空释义。前端显示可见词头，以同一内建样式呈现段落、音标、词性、义项、例句、列表、引用、表格、ruby、上下标和代码；桌面浮层加宽至 520 px，移动抽屉保持原高度、遮罩和滚动行为。没有增加依赖、来源 CSS、MDD 资源或网络能力。

## Review

独立规格 review 未发现实现层问题；标准 review 发现旧纯文本投影和富文本空容器回退两处兼容风险，均已在共同后端投影与面板边界修复，并补入回归测试，复核后零发现。生命周期文档已在 review 后统一收口。

## Evidence And Residual Risks

`bash scripts/check-dictionary-source.sh --private-fixtures fixtures/local` 已通过公共安全测试、私有 MDict / Kindle 兼容纯文本哈希和 release benchmark；最终 Linux 冷 / 热 P95 分别为 Kindle 6.296 / 5.319 ms、MDict 0.877 / 0.891 ms、MDD 0.459 / 0.465 ms，峰值 RSS 27,644 KiB。`mise exec -- pnpm --dir reader/app check`、`build` 以及 `cargo clippy --locked -p atha-backend --all-targets -- -D warnings` 通过。

`agent-browser` 通过合成 Tauri IPC 驱动真实 Svelte 词典组件，验证 1280 × 900 桌面浅色、390 × 844 移动浅色和移动深色的面板边界、内部滚动、末项可达及无横向溢出。截图位于 `artifacts/local/audits/dictionary-rich-definition/`，SHA-256 分别为 `1065e3439e773f14b6bc8e979b45149578abcc218d73cb7b65e1353dbd122569`、`20ff5405e383b369ac1033e2c788a6d75cb96e91deed4e1bf5f26151cc5bfb2a` 与 `407dc3e76d5674be3ecceaadd1aa2ce0a7d86190be23cb904a511f6d75d3bcbe`。

最高证据是 Linux 本地私有后端加合成浏览器组件，不是当前富文本在 Linux Tauri / WebKitGTK 或 PCT-AL10 上的真实目标验收；既有真机查词与抽屉证据早于本变更。来源 CSS、图片、音频和其他富 MDD 资源仍按范围拒绝，个别依赖自定义 class 才能表达层级的词典可能只得到通用段落排版。

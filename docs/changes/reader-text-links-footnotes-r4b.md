# R4B 文本、链接与脚注交互

## Status

implemented

## Problem

R4A 已避免翻页输入抢走选择和链接，但正文仍没有定义选择/复制、书内链接、外部链接与脚注的实际行为。当前校验器只允许同页 fragment，跨章节链接和外部参考会让整章加载失败；若直接恢复浏览器默认导航，又会绕过 manifest、Navigation 与外部访问边界。

## Scope

- 文本选择与复制保留浏览器原生行为，Atha 不改写选中文本、不接管系统剪贴板；
- 内容校验允许同书 XHTML 链接以及无凭据的 HTTP/HTTPS 外部链接，继续拒绝脚本 scheme、下载、目标窗口和其他主动导航能力；
- 所有书内链接点击由一个内容动作 module 截获，经 Navigation 按 manifest section 和 fragment 跳转，未知 section 或 fragment 明确回落；
- HTTP/HTTPS 外部链接不在 WebView 内打开，也不发起网络；使用原生 dialog 明确告知已阻止并显示目标站点；
- 同章 `noteref` 点击在同一原生 dialog 显示脚注纯文本，关闭后焦点回到触发链接；其他书内链接正常跳转；
- dialog 使用原生焦点、Escape 与关闭按钮语义，不设计最终脚注或外链视觉。

## Non-Goals

- 不调用系统浏览器、不加入按书允许外链、域名白名单、链接预览或网络资源加载；
- 不实现跨章节脚注弹层、脚注历史、返回栈、悬浮预览或复杂 EPUB 脚注兼容规则；
- 不实现图片、表格、代码或公式交互；留给 R4C；
- 不生成 selection range locator、标注工具条或 `SourceAnchor`；留给 R7；
- 不自定义复制菜单、剪贴板格式或 DRM 行为。

## Acceptance Criteria

- [x] 鼠标可形成正文选择，Ctrl+C 进入浏览器原生复制事件链，选中文字不被应用改写；
- [x] 同章 fragment 和跨 section 链接只通过 Navigation 跳转，未知目标安全回落且阅读会话继续可用；
- [x] 外部 HTTP/HTTPS 链接不导航、不请求网络，并以键盘可关闭的原生 dialog 明确反馈；危险 scheme、target 与 download 继续在内容边界拒绝；
- [x] 同章 noteref 显示脚注纯文本，关闭后焦点返回触发链接；脚注内内容不作为活动 HTML 执行；
- [x] R4A 翻页输入不会抢走链接、dialog 或有效文本选择，既有 Locator、偏好和安全检查保持有效；
- [x] 四样本实际 host、明暗浏览器、Rust 检查和 benchmark 保持通过；
- [x] 独立规格与标准 review 无 blocking，事实所有者和 `ACTIVE` 与最终实现一致。

## Files And Steps

1. 在内容边界分类并规范化书内与 HTTP/HTTPS 链接，补危险 scheme 负例；
2. 复用 Navigation 的 section/fragment 跳转路径，新增薄内容动作 module 与原生 dialog；
3. 扩展真实诊断，覆盖选择、复制、同章/跨章链接、外链阻止、脚注焦点和 R4A 互斥；
4. 运行页面、Rust、实际 host、benchmark、文档和独立 review，更新事实所有者并关闭本 change。

## Checks

- 所有页面 JavaScript module 的 `node --check`；
- `cargo fmt --all --check`、`cargo clippy --workspace --all-targets --locked -- -D warnings`、`cargo test --workspace --all-targets --locked`；
- `pwsh -NoProfile -File scripts/check-reader-samples.ps1`；
- `pwsh -NoProfile -File scripts/check-reader-slice.ps1`；
- workflow `docs` gate；
- 本次中文 Markdown 的 `autocorrect --fix` 与 `autocorrect --lint`；
- `git diff --check`。

## Rollback

回滚本 change 的提交即可恢复 R4A 行为；不涉及耐久数据、网络许可或 schema。

## Approval

用户明确授权依据当前路线图继续实现到 M2 结束，并要求缺少规格时补规格。本 change 只落实 R4 的文本、链接与脚注切片。

## Result

已保留 closed Shadow DOM 中的浏览器原生选择与复制，新增薄 `content-actions` module 和一个原生 dialog。内容边界只允许同书 XHTML 与无凭据 HTTP/HTTPS 链接；书内链接统一进入 Navigation，fragment 会跳过目标开头不可见空白，外链只显示目标站点且不导航，同章 noteref 只投影目标纯文本。没有增加系统浏览器调用、网络许可、脚注历史或标注接口。

## Review

- Spec：PASS；鼠标选择、trusted Ctrl+C、默认 copy 负例、链接与 dialog 证据和规格一致，无 blocking；
- Standards：PASS；协议、受控激活、fragment offset 正确性与最坏单节点复杂度、焦点和验证链无 blocking。

## Evidence And Residual Risks

- 静态与本地证据：十份页面 module 语法、Cargo fmt/clippy/test、资源与遥测 3/3、host 参数 2/2 通过；
- 真实目标证据：四样本实际 Windows host 与 Agent Browser 明暗主题通过；真实鼠标拖选均产生非空 selection，trusted Ctrl+C copy 事件均到达正文，验证探针只在该次事件阻止写入系统剪贴板；程序化负例另证明正常监听不取消 copy 默认行为或改写选文；
- 链接证据：同章有效 fragment 到达目标 offset，尾部空锚点映射最后页，缺失 fragment 与未知 section 均安全回落，多章节样本验证跨 section；唯一 `.invalid` 外链在浏览器网络记录中为零请求，脚注 dialog 验证纯文本、关闭焦点返回和 PageDown 不翻动背景；
- 性能证据：10 次样本中位数为冷启动 832.747ms、首个稳定页 163.050ms、热打开 20.800ms、翻页 6.200ms、字号重排 27.800ms；fragment 搜索先排除无布局 text node，只对有布局节点二分 UTF-16 offset；没有同时间旧代码对照；
- 外部链接当前明确阻止；只有产品 shell 确定系统浏览器授权与审计边界后才增加“打开”动作；跨章节脚注按普通书内链接跳转，不弹层；
- 正式浏览器探针改用 agent-browser stdin，避免 Base64 参数在 Windows 多章节样本上超过命令行长度。

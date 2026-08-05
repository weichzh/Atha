---
description: 基于 Telegram Desktop 官方源码研究消息列表、回复、元信息、菜单、定位与自适应，并给出 Atha 阅读对话界面规格。
---

# Telegram Desktop 消息界面与 Atha 阅读对话研究

## 结论

Telegram Desktop 值得 Atha 学习的不是左右两种气泡颜色，而是消息界面的信息压缩方式：消息始终保持一条按时间排列的线性流，回复关系压缩进消息顶部的短预览；正文是唯一高对比主体，时间、编辑状态等元信息退到末行；低频操作按当前选择和权限进入上下文菜单；点击回复可以载入目标附近、把目标带入视口并短暂高亮；窄窗口改变栏位和可用宽度，不改变消息语义。

Atha 应据此把当前“每条卡片下铺满按钮、原文反复出现、引用复选框长期展开”的原型改成一条阅读对话时间线：顶部只保留一次原文上下文，消息正文保持单列，回复用一行预览表示，操作收到显式的“更多”菜单，输入区固定在底部。Atha 是单人阅读工具，不应复制 Telegram 的收发两侧、头像、15 分钟发言人分组、发送/已读勾、反应和转发体系。

本研究只给出界面与交互规格，不批准或实施 UI 变更。

## 研究范围与证据边界

研究对象是 Telegram 官方桌面客户端仓库 [`telegramdesktop/tdesktop`](https://github.com/telegramdesktop/tdesktop)。官方 README 明确说明该仓库包含官方桌面客户端的完整源码，并采用带 OpenSSL 例外的 GPLv3。[来源](https://github.com/telegramdesktop/tdesktop/blob/8e921d1166db642a7027e7d0c256c8db6f42eafb/README.md#L1-L13)

本文固定到 `dev` 分支提交 [`8e921d1166db642a7027e7d0c256c8db6f42eafb`](https://github.com/telegramdesktop/tdesktop/commit/8e921d1166db642a7027e7d0c256c8db6f42eafb)，读取范围集中在 `Telegram/SourceFiles/history/`、`ui/chat/` 和 `window/`。结论来自静态源码，没有构建或运行 Telegram Desktop，也没有把源码、图标或样式资源复制进 Atha。源码可以证明布局和交互规则，不能替代实际视觉验收；若进入实现，仍需以 Atha 的真实窗口截图、键盘操作和读屏结果验收。

## Telegram 的消息视图如何组织

### 数据、列表与单条消息是分开的

Telegram 没有让一个页面组件同时查询数据、排版消息和处理所有操作。`ListWidget` 通过 `ListDelegate::listSource(aroundId, limitBefore, limitAfter)` 获取目标位置附近的一段 `MessagesSlice`，列表用 `Element` 表示可绘制的历史项，普通消息再由 `Message final : Element` 负责具体布局。[列表数据接口](https://github.com/telegramdesktop/tdesktop/blob/8e921d1166db642a7027e7d0c256c8db6f42eafb/Telegram/SourceFiles/history/view/history_view_list_widget.h#L116-L128)；[列表类](https://github.com/telegramdesktop/tdesktop/blob/8e921d1166db642a7027e7d0c256c8db6f42eafb/Telegram/SourceFiles/history/view/history_view_list_widget.h#L307-L378)；[消息类](https://github.com/telegramdesktop/tdesktop/blob/8e921d1166db642a7027e7d0c256c8db6f42eafb/Telegram/SourceFiles/history/view/history_view_message.h#L164-L180)

绘制时，列表根据脏矩形用二分查找找到首尾可见项，只绘制这个区间；命中测试和当前顶部项也通过纵坐标定位。它不是 Web 前端可以原样照抄的“虚拟列表库”，但说明成熟消息流需要明确的可见区、稳定顺序和局部刷新，而不是每次操作重建整个界面。[可见区绘制](https://github.com/telegramdesktop/tdesktop/blob/8e921d1166db642a7027e7d0c256c8db6f42eafb/Telegram/SourceFiles/history/view/history_view_list_widget.cpp#L2569-L2673)；[按纵坐标定位](https://github.com/telegramdesktop/tdesktop/blob/8e921d1166db642a7027e7d0c256c8db6f42eafb/Telegram/SourceFiles/history/view/history_view_list_widget.cpp#L3035-L3078)

对 Atha 的直接启示是：保留现有正式消息模型和 `conversation()` 查询，UI 只投影 `ConversationView`；第一版无需引入虚拟列表。只有真实长对话 benchmark 证明 DOM 数量成为瓶颈时，才按消息 id 和可见区做窗口化。

### 关系用预览表达，时间线仍然是线性的

Telegram 的回复不是树形缩进列表。所有消息继续按时间排列，子消息顶部嵌入一块 `Reply` 预览来表达父关系。这使深层回复不会不断挤窄正文，也允许用户自然地回看时间顺序。

`Reply::update` 会根据场景选择手工引用文本、被回复消息的短文本、投票/任务或媒体摘要，并另外计算发送者名称、颜色和可选媒体缩略图。[回复内容选择](https://github.com/telegramdesktop/tdesktop/blob/8e921d1166db642a7027e7d0c256c8db6f42eafb/Telegram/SourceFiles/history/view/history_view_reply.cpp#L330-L459) 预览有独立的最大宽度、高度和窄宽重排逻辑；绘制时使用强调竖线/引用形状、名称、单行省略文本和可选 32px 媒体预览，而不是复制一张完整消息卡。[回复尺寸](https://github.com/telegramdesktop/tdesktop/blob/8e921d1166db642a7027e7d0c256c8db6f42eafb/Telegram/SourceFiles/history/view/history_view_reply.cpp#L630-L741)；[回复绘制](https://github.com/telegramdesktop/tdesktop/blob/8e921d1166db642a7027e7d0c256c8db6f42eafb/Telegram/SourceFiles/history/view/history_view_reply.cpp#L782-L1035)

对 Atha 应采用同样的关系表达：

- 对话顶部显示一次根原文，普通消息不再重复渲染同一段 `source.selectedText`；
- 回复非根消息时，在正文上方显示一条紧凑预览，内容为父消息的一行摘要；
- 消息仍按创建顺序排列，不按 `replyToMessageId` 做层层缩进；
- `referenceIds` 是非父级关联，应显示为“引用 2 条”之类的次级入口，展开后再列出，而不是让复选框列表常驻输入区；
- 被回复消息已删除时保留关系和“消息已删除”占位，不让回复失去来路。

### 气泡分组由明确规则计算

Telegram 把“附着到上一条消息”作为 `Element` 状态计算，而不是靠 CSS 猜相邻节点。普通场景下，只有同一发送者、时间差小于 15 分钟、同一话题，且中间没有日期、未读条、服务消息等分隔项时才附着；附着状态变化会触发布局重算。[15 分钟规则](https://github.com/telegramdesktop/tdesktop/blob/8e921d1166db642a7027e7d0c256c8db6f42eafb/Telegram/SourceFiles/history/view/history_view_element.cpp#L95-L96)；[完整附着条件](https://github.com/telegramdesktop/tdesktop/blob/8e921d1166db642a7027e7d0c256c8db6f42eafb/Telegram/SourceFiles/history/view/history_view_element.cpp#L2198-L2248)；[重算相邻关系](https://github.com/telegramdesktop/tdesktop/blob/8e921d1166db642a7027e7d0c256c8db6f42eafb/Telegram/SourceFiles/history/view/history_view_element.cpp#L2340-L2360)

样式进一步把附着消息的顶部间距降为零，并限制普通消息宽度为 160–430px，使用紧凑内边距；头像只在一组消息的末条出现。[消息宽度、边距和回复预览尺寸](https://github.com/telegramdesktop/tdesktop/blob/8e921d1166db642a7027e7d0c256c8db6f42eafb/Telegram/SourceFiles/ui/chat/chat.style#L14-L67)；[末条显示头像](https://github.com/telegramdesktop/tdesktop/blob/8e921d1166db642a7027e7d0c256c8db6f42eafb/Telegram/SourceFiles/history/view/history_view_message.cpp#L3246-L3248)

Atha 只有一个本地用户，没有发言人切换，因此不能把 Telegram 的发送者规则照搬成假的分组。适合 Atha 的规则更简单：相邻普通消息保持稳定的短间距；出现根原文、删除占位或日期分隔时增加间距；回复预览负责表达关系。不要为了“像 Telegram”制造头像、昵称或左右气泡。

## 消息内部的信息层级

### 正文优先，元信息压到末行

Telegram 的 `BottomInfo` 将时间、编辑标记、作者签名、浏览/回复计数、置顶状态以及发送状态组合成一块低权重末行，并根据可用宽度重新计算。它不是在正文下面放一组带文字的操作按钮。[元信息布局](https://github.com/telegramdesktop/tdesktop/blob/8e921d1166db642a7027e7d0c256c8db6f42eafb/Telegram/SourceFiles/history/view/history_view_bottom_info.cpp#L103-L230)；[图标与状态绘制](https://github.com/telegramdesktop/tdesktop/blob/8e921d1166db642a7027e7d0c256c8db6f42eafb/Telegram/SourceFiles/history/view/history_view_bottom_info.cpp#L264-L389)

“已编辑”与时间被拼成同一段本地化文本，内容过长时会省略作者而保留日期；发送中、已发送和已读通过末端图标表达。[编辑与时间文本](https://github.com/telegramdesktop/tdesktop/blob/8e921d1166db642a7027e7d0c256c8db6f42eafb/Telegram/SourceFiles/history/view/history_view_bottom_info.cpp#L480-L525)；[消息状态映射](https://github.com/telegramdesktop/tdesktop/blob/8e921d1166db642a7027e7d0c256c8db6f42eafb/Telegram/SourceFiles/history/view/history_view_bottom_info.cpp#L650-L710)

Atha 的末行只需要真实存在的语义：创建或最近修订时间、“已编辑”和删除状态。Atha 是本地持久化，不存在对方已读、网络发送中、浏览数和转发数，不应伪造 Telegram 的双勾。完整修订历史仍保留在菜单中；它是 Atha 的领域能力，不必塞进气泡正文。

当前 `MessageView` 没有创建或更新时间，只有修订查询带 `createdAt`，因此实施时间元信息前应由后端查询直接返回当前修订时间，不能为每条消息额外调用一次 `revisions()`。

### 低频操作进入按状态生成的菜单

Telegram 的菜单不是固定动作清单。菜单构建器先判断当前是否选中文本或多条消息，再按消息是否允许回复、编辑、删除、复制、转发等条件加入动作。[菜单总装配](https://github.com/telegramdesktop/tdesktop/blob/8e921d1166db642a7027e7d0c256c8db6f42eafb/Telegram/SourceFiles/history/view/history_view_context_menu.cpp#L1452-L1600) 回复、编辑和删除各自还有独立的可用性检查；多选动作与单条动作也明确分开。[回复与编辑](https://github.com/telegramdesktop/tdesktop/blob/8e921d1166db642a7027e7d0c256c8db6f42eafb/Telegram/SourceFiles/history/view/history_view_context_menu.cpp#L630-L770)；[删除](https://github.com/telegramdesktop/tdesktop/blob/8e921d1166db642a7027e7d0c256c8db6f42eafb/Telegram/SourceFiles/history/view/history_view_context_menu.cpp#L865-L983)

Atha 应删除每条 `.message-card-actions` 中常驻的七个文字按钮，改为：

- 桌面鼠标悬停或键盘聚焦时显示一个“回复”快捷图标和一个“更多”按钮；
- “更多”使用应用自己的弹出菜单，依次提供复制、编辑或添加笔记、跳回原文、查看引用、修订历史、删除；
- 没有原文的回复不显示“跳回原文”和“历史引用”；删除占位只保留修订历史；
- 回复和编辑进入输入区上方的上下文条，不能弹出另一套表单；
- 继续禁止 WebView 默认右键菜单。若以后支持右键，也只能触发同一个应用菜单；菜单键、`Shift+F10` 和可聚焦的“更多”按钮必须同样可达。

最后一点比复刻 Telegram 的原生右键更适合 Atha：既保持既有的书籍 WebView 安全边界，也避免把关键能力只藏在鼠标右键中。

## 定位、滚动与反馈

点击 Telegram 的回复预览不是简单调用一个坐标。链接保存目标消息、返回消息以及引用文本/偏移；目标在别的会话或尚未加载时也能请求对应位置。到达后，列表会按目标高度选择居中或贴顶，引用只占消息一部分时按高亮范围调整视口。[回复链接与返回位置](https://github.com/telegramdesktop/tdesktop/blob/8e921d1166db642a7027e7d0c256c8db6f42eafb/Telegram/SourceFiles/history/view/history_view_reply.cpp#L471-L535)；[目标位置计算](https://github.com/telegramdesktop/tdesktop/blob/8e921d1166db642a7027e7d0c256c8db6f42eafb/Telegram/SourceFiles/history/view/history_view_list_widget.cpp#L806-L877)

如果目标不在当前切片，`showAtPosition` 会先围绕目标刷新数据；定位完成后按距离选择立即、完整动画或部分动画，并调用高亮管理器短暂标出消息或精确引用范围。[加载并显示目标](https://github.com/telegramdesktop/tdesktop/blob/8e921d1166db642a7027e7d0c256c8db6f42eafb/Telegram/SourceFiles/history/view/history_view_list_widget.cpp#L1005-L1090)；[高亮目标或引用](https://github.com/telegramdesktop/tdesktop/blob/8e921d1166db642a7027e7d0c256c8db6f42eafb/Telegram/SourceFiles/history/history_view_highlight_manager.cpp#L18-L143)

列表重排前还会保存“顶部消息 id + 相对偏移”，重排后按同一锚点恢复，避免新内容或尺寸变化把用户送到别处。[滚动锚点保存与恢复](https://github.com/telegramdesktop/tdesktop/blob/8e921d1166db642a7027e7d0c256c8db6f42eafb/Telegram/SourceFiles/history/view/history_view_list_widget.cpp#L1100-L1145)

Atha 的对应规格应是：

1. 点击回复预览或关联消息，按稳定 `message.id` 定位同一线性列表；
2. 目标进入视口后短暂强调，并把程序焦点移到目标 `article`，让键盘和读屏用户得到同一反馈；
3. “跳回原文”沿现有 source locator 返回阅读页并高亮原文；
4. 关闭原文、修订或引用详情后恢复发起消息和列表滚动位置；
5. 新增、编辑或删除消息时，若用户不在列表底部则保持当前锚点，不自动抢回输入区附近。

第一版可以用 DOM 的 `scrollIntoView()`、消息 id 和 CSS 高亮完成，不需要移植 Telegram 的动画管理器。

## 桌面宽窗与窄窗口

Telegram 在应用级根据左栏、主聊天栏和第三栏的最小宽度选择一栏、两栏或三栏；宽度不足时主聊天栏独占可用区域，而不是把三栏一起压窄。[栏位计算](https://github.com/telegramdesktop/tdesktop/blob/8e921d1166db642a7027e7d0c256c8db6f42eafb/Telegram/SourceFiles/window/window_session_controller.cpp#L2591-L2640) 聊天区尺寸变化时会重新计算顶部栏、固定底部输入区和中间滚动区；若原本在底部则继续贴底，否则保留原滚动增量。[聊天区重排](https://github.com/telegramdesktop/tdesktop/blob/8e921d1166db642a7027e7d0c256c8db6f42eafb/Telegram/SourceFiles/history/view/history_view_chat_section.cpp#L3030-L3118)

单条消息也按当前宽度重新限制内容宽度，普通文本仍受最大气泡宽度约束；`Narrow` 模式会减少边缘留白，而不是隐藏正文或改变关系模型。[消息窄宽重排](https://github.com/telegramdesktop/tdesktop/blob/8e921d1166db642a7027e7d0c256c8db6f42eafb/Telegram/SourceFiles/history/view/history_view_message.cpp#L6380-L6448)

Atha 不需要复制 Telegram 的三栏聊天壳，但应采用相同原则：

- 窄窗口和当前优先的移动竖屏布局中，阅读对话使用完整页面，顶部栏和底部输入区固定，中间只有一块独立滚动的消息列表；
- 较宽窗口中可以让阅读对话成为右侧面板，保留原文页面作为上下文；不再使用当前可纵向 resize、悬浮在正文上的底部卡片；
- 消息正文设可读的最大行宽，窗口继续变宽时增加外部留白，不把短消息拉成全宽卡片；
- 断点由 Atha 容器实际可用宽度决定，不复制 Telegram 的 380/880px 常量；具体值在真实窗口截图验收时确定。

## 键盘与可访问性

Telegram 不把整块画布当成一个不可读控件。消息列表有可访问名称，子项暴露为可聚焦、可选择的 `ListItem`；每条消息的可访问名称会串联发送者、回复摘要、正文和状态，另提供发送者、回复、正文、编辑状态、时间等结构化子项。[列表项角色与状态](https://github.com/telegramdesktop/tdesktop/blob/8e921d1166db642a7027e7d0c256c8db6f42eafb/Telegram/SourceFiles/history/view/history_view_list_widget.cpp#L5470-L5535)；[消息可访问文本](https://github.com/telegramdesktop/tdesktop/blob/8e921d1166db642a7027e7d0c256c8db6f42eafb/Telegram/SourceFiles/history/history_inner_widget_accessibility.cpp#L30-L109)；[结构化字段标签](https://github.com/telegramdesktop/tdesktop/blob/8e921d1166db642a7027e7d0c256c8db6f42eafb/Telegram/SourceFiles/history/history_inner_widget_accessibility.cpp#L375-L475)

读屏模式下，上下方向键逐条移动，Page Up/Down 按一屏移动，Home/End 到首尾，焦点出视口时同步滚动；`Ctrl+Space`、`Shift+方向键`、复制、删除和 Escape 都有明确行为。[键盘导航与选择](https://github.com/telegramdesktop/tdesktop/blob/8e921d1166db642a7027e7d0c256c8db6f42eafb/Telegram/SourceFiles/history/view/history_view_list_widget.cpp#L3083-L3305) 异步无障碍动作不依赖会随插入删除变化的数组下标，而给消息分配稳定身份后回到主线程解析当前索引。[稳定无障碍身份](https://github.com/telegramdesktop/tdesktop/blob/8e921d1166db642a7027e7d0c256c8db6f42eafb/Telegram/SourceFiles/history/view/history_view_list_widget.cpp#L5690-L5729)

Atha 的 DOM 已有语义基础，但当前把整个列表设为 `aria-live="polite"`，每次 `replaceChildren` 都可能让读屏重复朗读所有消息。实施时应：

- 用有序列表或 `role="list"` 承载消息，每条 `article`/`role="listitem"` 可程序聚焦；
- 只让独立状态区报告“消息已发送”“删除失败”等变化，不让整条历史成为 live region；
- 回复预览使用按钮并给出“回复某消息：摘要”的可访问名称；
- “更多”按钮、弹出菜单、修订和引用详情形成完整的焦点进入与返回链；
- 支持 Escape 关闭详情/菜单、`Ctrl+Enter` 发送，并保留标准文本复制；逐条方向键导航可在真实键盘需求出现后增加，不必为了模仿 Telegram 先拦截所有方向键。

## Atha 第一版界面规格

### 页面骨架

阅读对话应由三个稳定区域组成：

1. **顶部栏**：返回；一行原文摘要；“跳回原文”；更多菜单中的导出。原文摘要可展开，但不随每条消息重复。
2. **消息时间线**：单列、独立滚动；根标注作为起点，后续消息按创建时间排列；日期变化时才插入轻量日期分隔。
3. **输入区**：固定底部；上方只有当前“回复 / 编辑”上下文条；正文输入和发送按钮始终在同一位置；选择额外引用时才打开临时选择面板。

### 单条消息

从上到下只包含：

- 可选的父消息回复预览：强调竖线、一行摘要、删除占位；
- 消息正文；纯标注显示低权重的“仅标注原文”，不伪造空消息；
- 可选的“引用 N 条”入口；
- 末行时间与“已编辑”；
- 悬停或聚焦时出现的回复和更多按钮。

删除消息继续显示一条紧凑 tombstone，因为 Atha 的引用、回复和修订历史需要稳定身份。Telegram 的普通聊天删除可以让内容从当前历史消失，但这不适合 Atha 已定义的软删除与历史可追溯契约。

### 动作层级

| 位置 | 动作 | 说明 |
| --- | --- | --- |
| 消息悬停/聚焦 | 回复、更多 | 只保留一个高频动作和菜单入口 |
| 更多菜单 | 复制、编辑/添加笔记、跳回原文、引用、修订历史、删除 | 按消息状态过滤，不显示无效项 |
| 顶部栏 | 返回、跳回原文、更多 | 导出进入更多，不常驻占宽 |
| 输入上下文条 | 取消回复/编辑 | 同一输入区完成发送和修订 |
| 关联选择面板 | 搜索并多选本对话消息 | 仅在用户主动“添加引用”时出现 |

## 不应照搬的部分

- **左右收发气泡、头像和发送者分组**：Atha 是一个人的阅读思考，不存在聊天双方；复制这些会制造错误语义。
- **15 分钟分组规则**：它服务多人聊天的发言人识别，Atha 只需要稳定间距、日期和关系预览。
- **发送中、单勾双勾、已读、浏览量和转发量**：本地消息没有这些状态。
- **反应、贴纸、媒体下载、转发、置顶和群组权限菜单**：没有当前产品用例，不进入第一版。
- **C++ 自绘与完整可见区渲染架构**：Telegram 为超长、高动态聊天优化；Atha 先使用语义 DOM，以 benchmark 决定是否窗口化。
- **应用级一/二/三栏导航**：Atha 只借鉴“空间不足就切换布局，而不是继续压缩”的原则。
- **默认右键菜单**：Atha 已禁止 WebView 默认菜单；使用同一套显式应用菜单覆盖鼠标和键盘入口。
- **源码或视觉资源直接移植**：Telegram Desktop 为 GPLv3；本研究只提炼交互模式，不复制实现、图标或专有视觉资产。

## 当前实现到目标实现的最小改动面

当前简陋感主要来自三个已有位置，不需要重做消息后端：

- `reader/app/src/components/ConversationOverlay.svelte`：从底部可 resize 浮层改为窄屏整页、宽屏侧栏的三段骨架；
- `reader/web/conversations.mjs`：把“原文 + 正文 + 全套按钮”卡片投影改成线性消息、回复预览、末行元信息和状态菜单；保留现有 create/reply/revise/delete/relationships/snapshots 调用；
- `reader/atha-reader.css`：替换 `.message-card-actions` 常驻按钮和全宽卡片视觉，增加消息列宽、回复预览、菜单、焦点、高亮及窄/宽容器规则。

数据契约只需要一个经实际界面证明的补充：`MessageView` 返回当前修订的 `createdAt`/`updatedAt`，从而一次查询绘制时间与编辑状态。不要在这次界面改造中引入头像、同步状态、反应、虚拟列表或通用聊天组件。

## 实施顺序与验收

1. 先完成静态时间线、顶部原文和固定输入区，删除每条消息下的常驻动作行；
2. 接入回复预览、点击定位和短暂高亮；
3. 把现有动作接入同一个“更多”菜单，并验证删除/已删除的动作过滤；
4. 补当前修订时间和“已编辑”末行；
5. 完成窄屏整页与宽屏侧栏，不再覆盖正文；
6. 用键盘和读屏验证菜单、定位、详情与焦点返回；
7. 最后才做颜色、圆角、阴影和动画的视觉微调。

验收至少覆盖：根标注、带笔记根消息、回复根消息、回复普通消息、额外引用、长正文、多次修订、软删除、目标不在当前视口、窄窗口和宽窗口。每个目标跳转都必须同时有可见高亮和程序焦点；主题切换后不得出现写死颜色；列表滚动、输入和展开引用不得遮住正文或丢失当前位置。

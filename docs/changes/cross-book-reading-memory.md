---
description: 跨书阅读记忆中心的范围、验收、实现与关闭证据。
---

# 跨书阅读记忆中心

Status: implemented

## Problem

当前书架只能按书名和作者查找书籍，Message 搜索与历史 Snapshot 只能进入某本书后使用。已有阅读统计、MessageStore、SourceAnchor、SourceSnapshot 和书架 DTO 已包含形成跨书入口所需的事实，但产品还不能回答“最近读过什么”或“这条笔记在哪本书里”，书籍移出后也没有安全查看历史引用的入口。

## Scope

- 在资料库主窗口提供“书架 / 阅读记忆”两个顶层入口，移动与桌面使用同一组件和状态；
- 从严格校验后的 schema 1 阅读统计与当前书架投影最近阅读，按 `lastReadDate` 降序稳定排列，并显示本书累计时长；
- 在 MessageStore 现有 FTS5 投影上增加全局消息搜索，不建立新表或新索引；查询最多 256 个字符、结果最多 200 条，三字符以上按相关性再按更新时间排序，短查询按更新时间稳定排序；
- 搜索结果包含书籍、当前根 Anchor、命中消息和更新时间。当前书架存在相同完整内容身份时才提供“跳回原书”；打开后由阅读器再次验证 Edition、当前 Locator 与正文重锚结果，再定位并打开命中对话；
- 书籍缺失或当前 Locator 无法使用时不伪造跳转。每条结果都可读取根 Message 的当前与历史 SourceSnapshot；Snapshot 继续使用现有 HTML、CSS、资源和 Shadow Root 安全边界；
- 搜索排除已墓碑的命中消息以及根 Message 已墓碑的 Conversation，不物理删除或改写历史事实。

## Non-goals

- 不新增数据库、全文索引、统计 schema、同步模型、知识库对象或搜索历史；
- 不做模糊搜索、语义搜索、筛选器、趋势图、消息编辑或跨书关系图；
- 不恢复已移出或已删除的书籍，不从 Snapshot 反推可导航书籍；
- 不把 Linux WebKitGTK 结果称为 Windows、Android 或 PCT-AL10 验收。

## Acceptance

- `MEM-RECENT-01`：合法统计包含多本当前书架书籍；打开阅读记忆；最近阅读只显示精确身份匹配的当前书籍，按日期降序并以内容身份稳定打破同日并列；损坏或不可访问统计显示明确降级状态，不伪造空记录。
- `MEM-SEARCH-01`：多个 Edition 含相同查询词；执行全局搜索；结果跨书返回并按已定义顺序稳定排列，书名、作者、原文与消息正文来自既有事实，查询与结果内容不进入产品日志。
- `MEM-SEARCH-02`：命中消息或根 Message 已写删除墓碑；执行同一查询；对应命中不出现，其他未删除结果保持不变。
- `MEM-JUMP-01`：结果 Edition 的完整内容身份仍在书架；选择跳回；应用打开该书，阅读器验证当前 Locator、必要时唯一重锚，并定位根引用后打开命中消息所在对话。
- `MEM-JUMP-02`：结果 Edition 不在书架，或当前 Locator / 章节失效；界面不声明可跳回；用户仍可打开已存当前或历史 Snapshot，失败只显示固定错误。
- `MEM-SNAPSHOT-01`：根 Message 有一次主动重选；打开历史引用；当前与历史两个版本可切换，外部 URL、活动内容、未知资源和不安全 CSS 继续被拒绝。
- `MEM-UI-01`：Linux Tauri 真壳在 360 和 1000 宽度可进入阅读记忆、搜索、查看有书 / 缺书结果和 Snapshot；无横向溢出、遮挡、裁切、控制台错误或包含用户内容的 AppLog。

## Files And Steps

1. 扩展 MessageStore 查询 DTO 与 focused tests，复用现有 `message_search`、Edition、Conversation、当前根 Anchor 和墓碑语义。
2. 增加只允许资料库主路由调用的只读 Tauri commands 与 capability；不放宽现有阅读页写命令。
3. 从现有 Snapshot 渲染代码提取共享的安全元素构造；阅读页与阅读记忆继续走同一实现。
4. 增加严格最近阅读投影、阅读记忆组件和书架顶层入口；加入有书深链验证与缺书 Snapshot 降级。
5. 增加单一检查入口，扩充 Linux 真壳场景，更新事实所有者并完成独立 review。

## Checks

- `bash scripts/check-memory-center.sh`；
- `bash scripts/check-reader-linux.sh`；
- `cargo clippy --locked -p atha-backend -p atha-reader-app --all-targets -- -D warnings`；
- `autocorrect --fix/--lint` 仅针对本次中文 Markdown；
- `project_workflow.py station <task> --activity verification --gate docs`。

## Result

- 资料库新增“书架 / 阅读记忆”同层导航，从严格 schema 1 统计记录投影当前书架最近阅读；损坏或不可访问统计明确降级，不把旧书或缺书伪造成可继续阅读；
- MessageStore 在既有 `message_search` 上增加跨 Edition 只读查询，连接当前修订、Edition、未删除根 Message 与当前 Anchor；短查询与 FTS 查询都有稳定排序和 200 条上限，不增加数据库或索引；
- 有书结果按完整内容身份打开当前书籍，并在阅读页复核 Edition、根 Message 和 Locator / 唯一重锚后打开命中对话；缺书结果不显示跳回动作；
- 资料库与阅读页复用同一 Snapshot HTML、CSS、资源和 Shadow Root 安全渲染函数，可切换当前与历史 SourceCapture；
- 三个阅读记忆 command 只允许资料库根路由，现有消息写入和书内查询仍只允许阅读路由。

## Review

- Spec 独立审查对照本 change 的 Scope、Acceptance 与 Non-goals 逐项复核，zero findings；
- Standards 独立审查复核 FTS / 墓碑、command ACL、Snapshot 不可信内容、深链校验与隐私日志，zero findings；
- 两轴审查未发现需修改的 blocking finding。

## Evidence And Residual Risks

- `bash scripts/check-memory-center.sh` 通过：MessageStore 22 项、书架 / 最近阅读 Node 10 项、Snapshot / 深链 Node 检查、Svelte check / build、Tauri 12 项和 command ACL；
- `cargo clippy --locked -p atha-backend -p atha-reader-app --all-targets -- -D warnings` 通过；
- `bash scripts/check-reader-linux.sh` 通过：WebKitGTK 0.55.1，在 360 / 1000 宽度验证最近阅读、有书 / 缺书搜索、当前 / 历史 Snapshot 与安全跳回；既有 13 个手势场景、220 个有效测量和 AppLog 隐私继续通过；
- 上述最高证据为隔离数据下的 Linux Tauri 真壳，不是 Windows WebView2、Android 或 PCT-AL10 移动专项；自动化请求 touch Actions，但 WebKitGTK 实际报告 `mouse`；
- 搜索直接复用当前 FTS5 并限制 200 条；只有真实大库证明查询或渲染性能不足时才考虑分页、索引或虚拟化；
- 最近阅读沿用按本地日期记录的 `lastReadDate`，不能在同一天内表达精确打开顺序；当前产品没有耐久 `lastOpenedAt`，本阶段不为排序增加新事实。

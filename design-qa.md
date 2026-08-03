# Design QA

final result: passed

## 对比对象

- 视觉源：`C:\Users\nick\.codex\generated_images\019fc182-fc50-7972-996f-ae35e09b3467\exec-666abc27-e903-4bb3-9642-144dfcad147c.png`
- 实现视口：390 × 840 CSS 像素，`deviceScaleFactor = 2`
- 实现截图：`artifacts/local/screenshots/math-history-mobile-*.png`
- 同图对比：`artifacts/local/screenshots/design-qa-controls-comparison.png`、`artifacts/local/screenshots/design-qa-directory-comparison.png`

## 结果

| 等级 | 数量 | 结论 |
| --- | ---: | --- |
| P0 | 0 | 没有阻断阅读或操作的问题 |
| P1 | 0 | 没有核心流程失效或严重布局错误 |
| P2 | 0 | 顶部、底部、目录、搜索和设置在目标视口内均可见且可操作 |
| P3 | 2 | 纸张纹理和控件细节仍可继续做视觉精修 |

## 已核对

- 默认阅读态不显示工具层；章节和进度仍属于固定书页。
- 工具态使用覆盖层，不改变 `.reader`、`#page`、`#chapter-label` 或 `#position` 的几何尺寸。
- 顶部为左侧返回、右侧书签和更多菜单，没有搜索入口。
- 底部依次只有目录、搜索、笔记、进度和听书五个图标。
- 目录使用同一个列表，已有书签紧随对应章节；右上角书签是唯一添加或取消入口。
- 搜索、目录和设置面板互斥；亮度只改变书页，不改变工具层。

## 有意不同于视觉源

- 按当前产品要求移除了顶部搜索和视觉源中的其他顶部动作。
- 底部将亮度、字体替换为搜索、听书；文字标签全部移除。
- 目录不再提供搜索分段或书签编辑控件，只显示章节和书签。

## P3

- 当前使用纯色暗色背景，没有加入视觉源的纸张纹理；等阅读主题确定后再引入正式纹理资源。
- 表单仍使用系统原生控件和第一版间距；后续可按 `docs/codebase/READER-MOBILE-UI.md` 手工调整。

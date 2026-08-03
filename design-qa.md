# Reader Shell Design QA

## Source Visual Truth

- 控制层：`E:/Code/Atha/fixtures/local/weread/Screenshot_2026-08-02-23-01-44-62_1464dca687853f69c65d9bfc8cfd589c.jpg`
- 目录：`E:/Code/Atha/fixtures/local/weread/Screenshot_2026-08-02-23-01-57-39_1464dca687853f69c65d9bfc8cfd589c.jpg`
- 进度：`E:/Code/Atha/fixtures/local/weread/Screenshot_2026-08-02-23-02-07-31_1464dca687853f69c65d9bfc8cfd589c.jpg`
- 产品约束：用户已明确目录改为全屏、搜索独立、书签只从右上角添加、底栏改为目录/搜索/笔记/进度/听书，并删除低价值统计和自动翻页。

## Implementation Evidence

- 控制层：`E:/Code/Atha/artifacts/local/screenshots/reader-shell-02-tools.png`
- 全屏目录：`E:/Code/Atha/artifacts/local/screenshots/reader-shell-03-directory.png`
- 阅读进度：`E:/Code/Atha/artifacts/local/screenshots/reader-shell-04-progress.png`
- 分层设置：`E:/Code/Atha/artifacts/local/screenshots/reader-shell-05-settings-menu.png`
- 同图对照：
  - `E:/Code/Atha/artifacts/local/audits/reader-shell-usability/comparison-tools.png`
  - `E:/Code/Atha/artifacts/local/audits/reader-shell-usability/comparison-directory.png`
  - `E:/Code/Atha/artifacts/local/audits/reader-shell-usability/comparison-progress.png`

## Viewport And Normalization

- 实现视口：390 × 840 CSS px，devicePixelRatio 2，截图为 780 × 1680 px；
- 源截图：1440 × 3168 px；保持宽高比缩放到 764 × 1680 px，不裁切；
- 对照图：源图在左，实现图在右，1544 × 1680 px；两者高度和主题一致。源设备长宽比与 Atha 固定页面相差约 2.3%，因此只比较内容区域、工具层比例和节奏，不把该差异当成实现问题。

## State And Interaction Coverage

- 深色控制层、深色全屏目录、深色进度面板和分层设置菜单均由 production Svelte build 在 Agent Browser 中真实渲染；
- 实测目录 173 项，点击第 2 项后跳到“1.2 勾股数组”并关闭目录与工具层；
- 实测全书滑块从第 2/173 节跳到第 87/173 节，再回到书首；
- 实测书签投影到对应目录位置，点击书签后关闭目录并返回阅读；
- 实测浅色、纸张和深色主题的面板、输入框、目录项、边框与正文页同步换色；
- 控制台错误检查为空，横向溢出为 0；书页 390 × 840 CSS px，正文区域为 y=72–768，工具栏为 y=0–48 与 y=792–840。

## Required Fidelity Surfaces

- 字体与排版：壳层使用系统中文字体和 Lucide 图标；标题、章节、百分比、辅助文字层级清楚。正文继续尊重书源字体和绝对设备像素，这是产品契约，不按参考书籍截图强制替换；
- 间距与布局：48 CSS px 工具栏只占页眉页脚；目录完整覆盖视口；进度面板比例接近参考图，且删除统计后没有留下空洞；
- 颜色与令牌：所有壳层颜色只从语义令牌读取；交互强调色恢复为微信读书式蓝色，明暗主题对比清楚；
- 图像与资产：正文普通图片保持原色，公式只在深色主题反色；所有壳层图标来自 Lucide 或既有 Fluent 图标，没有占位图、手绘 SVG 或 CSS 图标；
- 文案：目录、阅读进度、全书百分比、本节页码和设置分类使用短中文；没有把内部路径、诊断字段或未实现功能混入普通面板。

全图对照已足以读清图标、标题、列表、滑块和辅助文字，因此不再裁切局部对照；设置菜单没有一张与用户批准后信息架构完全相同的源图，只按已批准的“列表后下钻”结构检查真实渲染和交互。

## Findings

没有剩余 P0、P1 或 P2 问题。

- P3：Atha 不复制微信读书的纸张纹理、阅读时长、预计读完、自动翻页和目录页码。前四项是已批准的性能与范围取舍；目录页码属于当前布局投影，不适合作为稳定目录数据。
- P3：设置菜单采用右上角紧凑列表，而不是微信读书底部排版台；这是用户已批准的入口和分层差异。

## Comparison History

1. P1：首轮真实浏览器检查发现全屏目录的 fixed 面板被定位到 y=793，而不是 y=0。原因是工具栏的 `backdrop-filter` 创建了 fixed containing block；删除该效果后，目录矩形为 x=0、y=0、390 × 840，并减少一项合成开销。
2. P2：全屏目录仍继承底部面板圆角，四角露出阅读背景；为 `.directory-panel` 明确设置 `border-radius: 0`，最终截图四边完整覆盖。
3. P2：首轮令牌整理把既有蓝色强调改成了绿色，与参考视觉和既有产品方向漂移；恢复 `#1497e8` 后，当前目录、进度和工具入口与参考的蓝色交互语义一致。
4. 修复后重新以相同 390 × 840、DPR 2、深色状态捕获三张实现图并重建同图对照；复查未发现新的 P0、P1 或 P2。

## Implementation Checklist

- [x] 主题颜色集中到语义令牌；
- [x] 工具栏与正文安全区几何稳定；
- [x] 目录全屏、书签同列、导航后返回阅读；
- [x] 全书进度、当前章节和本节页码可用；
- [x] 更多菜单按字体、布局、主题、行为和本书样式下钻；
- [x] Agent Browser 交互、控制台和同图视觉对照完成。

final result: passed

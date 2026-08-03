---
description: 评估 Atha 从 wry/tao 迁移到 Tauri 2 并引入前端框架的技术选型、边界与验证门槛。
---

# Tauri 与前端框架技术选型

## 结论

Atha 的默认技术组合建议定为：

> **Tauri 2 + Vite + Svelte 5 + TypeScript**

这个选择有一个重要边界：**Svelte 只管理应用壳与产品界面，不接管书籍 XHTML、分页、选区、定位和翻页热路径。** 现有阅读内核继续保持 framework-agnostic，由稳定的 TypeScript/JavaScript 模块直接控制书籍 DOM；Svelte 通过窄接口订阅阅读状态、发出用户命令。

选择 Svelte 5，是因为它与 Solid 同属本轮外部基准的性能第一梯队，而 Atha 最敏感的排版热路径本来就不交给框架；两者差异不足以在 Atha 实测前当成产品性能结论。此时生态成为有效的第二排序：Svelte 官方包目录已收录 Bits UI、shadcn-svelte、Ark UI、Testing Library、Storybook、TanStack 等持续维护的组件和工具，[Svelte 官方包目录](https://svelte.dev/packages)能够直接覆盖应用壳与无障碍控件需要。Svelte 5 的 runes 是编译器识别的响应式能力，可在 `.svelte` 和 `.svelte.js/.ts` 中使用。[Svelte 5 runes 官方说明](https://svelte.dev/docs/svelte/what-are-runes)

性能对照方案是 **Tauri 2 + Vite + SolidJS 1 + TypeScript**。Solid 的细粒度响应式直接更新依赖状态的 DOM，适合“少量状态高频变化、主体内容保持稳定”的壳层界面。[Solid 官方说明](https://docs.solidjs.com/advanced-concepts/fine-grained-reactivity)描述了 signal、effect 与精确订阅关系。若 Svelte 原型未通过 Atha 的真实门槛，再用同一接口做 Solid 1 对照；当前 Solid 2 文档和工具链仍明确标记为 beta，不纳入稳定产品选型。[Solid 2 状态](https://docs.solidjs.com/v2)

如果团队规模扩大，招聘、成熟管理后台组件和长期人员流动成为首要矛盾，则以 **Vue 3** 作为生态优先的保守选择。Vue 的运行时响应式、编译器优化和虚拟 DOM 模型成熟、可解释，但其性能模型和运行时体量并不是本次“性能第一”的最优点。[Vue 渲染机制](https://vuejs.org/guide/extras/rendering-mechanism)

不建议现在把 React 19、Preact 或 Lit 设为默认方案；原因见后文。

## Atha 的真实边界

Atha 当前不是从零开始的前端应用。它已经有以下经过实现和测试约束的基础：

- Windows-first、local-first，阅读渲染只使用 WebView2；
- Rust 主机基于 `wry/tao`，负责窗口、协议、状态与诊断；
- 阅读页由原生 HTML/CSS/JavaScript 模块组成，书籍 XHTML 在受控边界内渲染；
- 不可信书籍不得执行脚本、访问网络、越过书籍资源根目录或获得主机能力；
- 已有启动、打开、翻页、重排、内存和真实 EPUB 行为门槛。

因此，这次选型不应被定义为“用框架重写阅读器”，而应被定义为两个可以独立撤销的变化：

1. 将主机与打包层从直接使用 `wry/tao` 迁移到 Tauri 2；
2. 在现有阅读内核之外增加框架化应用壳。

两步必须分开做 A/B 验证。否则一旦启动、内存或交互性能变化，无法判断来自 Tauri、框架、构建产物还是顺手重写的阅读逻辑。

## Tauri 2 能带来什么

Tauri 不是另一个渲染引擎。其桌面应用仍由 Rust 核心、系统 WebView 和 IPC 组成；Windows 继续使用 WebView2，而且 WebView 库不随应用重复打包。[Tauri 进程模型](https://v2.tauri.app/concept/process-model/) Tauri 自身正是对 TAO 与 WRY 的编排，直接使用 `wry/tao` 仍然是官方支持的更深层集成路线。[Tauri 架构说明](https://v2.tauri.app/concept/architecture/)

所以迁移到 Tauri **不会天然降低 WebView2 的渲染成本，也不会消除 WebView2 进程的内存**。性能应预期为与当前方案同一量级，再由实测决定是否接受；不能把 Tauri 宣传中的安装包大小等同于 Atha 的运行时性能。

真正收益来自应用工程生态：

- 统一的配置、开发、构建、签名和安装包入口；
- 官方或官方维护范围内的对话框、文件系统、日志、更新、单实例、窗口状态、持久化存储、SQL 等插件；
- `command`、event 和 channel 等明确的 Rust/前端通信形式；
- capability、permission、scope 与 CSP 组成的主机能力边界；
- 前端无关，允许现有页面以 brownfield 方式渐进迁移，而不是一次重写。

[Tauri 插件目录](https://v2.tauri.app/plugin/)显示了它相对直接使用 `wry/tao` 最有价值的成熟能力；[brownfield 模式](https://v2.tauri.app/concept/inter-process-communication/brownfield/)也是默认模型，适合承接已有前端。

迁移成本和风险同样明确：

- 当前直接控制窗口、WebView2、定制协议和资源安全边界的代码，需要映射到 Tauri 配置、插件或扩展点；
- 新增 Node/Vite 前端构建链和 Tauri 配置链，发布问题的排查层数增加；
- `invoke` 默认涉及序列化，不应把大段 XHTML、图片或连续流量经 JSON 往返；Tauri 官方也提示大型返回值的 JSON 序列化会拖慢通信，并提供原始响应和 channel 作为替代。[从前端调用 Rust](https://v2.tauri.app/develop/calling-rust/)
- Tauri 的能力系统有助于缩小权限，但不会自动替 Atha 验证路径、来源和书籍资源；自定义 Rust command 仍然是信任边界；
- 为使用少数插件而迁移整个主机是否值得，必须由主机 A/B 原型回答。

## 推荐架构

建议形成三个清晰层次：

```text
Tauri 2 / Rust host
  ├─ 窗口、生命周期、安装与更新
  ├─ 文件选择、受控书库访问、持久化、日志
  └─ 窄 command / event / channel 接口

Svelte application shell
  ├─ 书架、导航、菜单、搜索、目录、笔记、设置
  ├─ 窗口和阅读状态的展示
  └─ 向 reader kernel 发出语义化命令

Framework-agnostic reader kernel
  ├─ XHTML 资源装载与隔离
  ├─ 分页、排版、定位、选区与翻页
  └─ 直接控制书籍 DOM，不挂载 Svelte 组件树
```

首轮迁移继续使用单 WebView，不为了隔离额外增加 WebView。书籍 XHTML 仍由现有受控协议装载到 closed Shadow DOM，导入时移除脚本；应用壳所在 WebView 只得到显式 capability allowlist，不暴露通用文件系统能力。Tauri 的 capability 按窗口或 WebView 授权，**不能**把同一 WebView 里的 Shadow DOM 当作独立权限域。[Capability 参考](https://v2.tauri.app/reference/acl/capability/) 只有未来经实测证明必须拆分书籍 WebView 时，才让该独立 WebView 不匹配任何 capability。

首批前端依赖只包含 Svelte、Vite 和 TypeScript。先用 Svelte 自带状态能力与普通 CSS；遇到对话框、菜单、Popover 等有真实无障碍要求的控件时，再按组件引入 Bits UI。暂不引入 SvelteKit、路由库、全局状态库、Tailwind 或整套设计系统。

第一阶段采用 brownfield 模式，并配置严格 CSP：不加载 CDN 或远程脚本，脚本和样式随应用构建。Tauri 不会自动生成安全的 CSP，仍需项目显式配置。[Tauri CSP 说明](https://v2.tauri.app/security/csp/)

Isolation 模式不作为首轮迁移的硬要求。它会让 IPC 先经过沙箱 iframe 并增加加解密步骤；Windows 下 isolation 应用还不支持直接加载 ES modules。可以在主机和框架性能稳定后，针对真实威胁模型单独评估。[Tauri isolation 模式](https://v2.tauri.app/concept/inter-process-communication/isolation/)

## 框架比较

### Vanilla

原生 DOM 仍是阅读内核的性能和控制基线，也是当前代码最小迁移成本的方案。它的问题不是不能做产品，而是随着书架、搜索、笔记、设置、同步状态和多窗口增加，组件复用、局部状态、可测试性及现成 UI 能力都要由项目自行建立，未解决本次引入成熟生态的目标。

结论：**保留在 reader kernel，不再承担整个应用壳。**

### SolidJS

优势：

- 细粒度订阅，状态变化直接触发相关计算和 DOM 更新，不需要应用级虚拟 DOM diff；
- 运行时较小，官方主页当前给出约 7 KB min+gzip 的量级；该数字只反映框架本身，不能代表 Atha 最终包或内存。[Solid 官网](https://www.solidjs.com/)
- JSX、TypeScript、Vite、Vitest 与 Testing Library 路径清楚，[官方测试指南](https://docs.solidjs.com/guides/testing)已有直接支持；
- 与现有命令式 reader kernel 可以通过 ref、adapter 和 store 边界组合，不要求改写书籍 DOM。

代价：生态和人员池明显小于 React/Vue；部分 React 组件不能直接复用；团队要理解 signal 的所有权、清理和异步边界。

Solid 2 仍处于 beta；如需对照，只使用稳定的 Solid 1。

结论：**性能对照方案，不是默认。** 只有 Svelte 未通过 Atha 实测门槛时才引入对照原型。

### Svelte 5

优势是编译器生成更新逻辑、模板简洁、应用开发体验成熟，性能通常接近 Solid。其官方包目录提供多套活跃的 headless UI、完整组件库、测试和可访问性工具，适合 Atha 自定义阅读界面而不再手写所有控件。缺点是 runes 和 `.svelte` 编译语义会让壳层代码与框架耦合，因此 reader kernel 必须留在普通 TypeScript/JavaScript 模块中。

结论：**默认应用壳。** 使用 Vite SPA，不引入 SvelteKit；Tauri 不需要服务端渲染和服务器路由。

### Vue 3

Vue 3 采用运行时 Proxy/ref 响应式，并由编译器向虚拟 DOM 提供优化信息。[Vue 响应式原理](https://vuejs.org/guide/extras/reactivity-in-depth.html) 大量列表仍需 shallow API、虚拟化等手段控制开销，[Vue 性能指南](https://vuejs.org/guide/best-practices/performance)也将这些列为优化路径。

它的组件生态、中文资料、人员供给和长期维护风险优于 Solid/Svelte，但 Atha 的主体不是通用管理后台，最关键的书籍 DOM 又不应交给组件库。因此生态优势不足以覆盖本轮的性能优先级。

结论：**生态优先的保守选项，不是默认。**

### React 19

React 拥有最大的组件和人员生态，但普通状态更新会重新执行相关组件及其嵌套组件，再在 commit 阶段最小化真实 DOM 修改。[React render/commit 说明](https://react.dev/learn/render-and-commit) React Compiler 已稳定并可以自动 memoize，但它仍是一层需要配置、诊断和维护的优化工具，而不是 Atha 当前必须承担的复杂度。[React Compiler 介绍](https://react.dev/learn/react-compiler/introduction)

结论：**暂不选择。** 只有未来出现不可替代的 React 专属组件、团队能力或跨端共享需求时再重新评估。

### Preact

Preact 本体小，官方将其描述为约 3 KB 的 React 替代方案，并通过 `preact/compat` 承接 React 生态。[Preact 官网](https://preactjs.com/) 但兼容层仍存在语义差异，[官方差异说明](https://preactjs.com/guide/v11/differences-to-react/)也明确列出事件等不同点。Atha 若为了生态使用兼容层，会同时承担 React 范式和兼容排障，而没有获得 Solid 那样明确的细粒度模型。

结论：**不作为默认。** 如果未来必须复用特定 React 组件且完整 React 成本不可接受，再做定点验证。

### Lit

Lit 体积小、基于 Web Components、只更新模板的动态部分，适合可嵌入的独立组件。[Lit 文档](https://lit.dev/docs/v3/) 但 Lit 默认使用 Shadow DOM，[Shadow DOM 说明](https://lit.dev/docs/components/shadow-dom/)会增加全局主题、焦点、选区、查询和测试边界。Atha 已经需要严格管理书籍内容隔离，再用 Lit 组织整个应用壳会叠加两套 Shadow DOM 语义，而其整套 UI 生态也弱于 Vue/React。

结论：**可用于未来独立可嵌入控件，不用于主应用框架。**

## 外部基准应如何使用

[js-framework-benchmark](https://github.com/krausest/js-framework-benchmark)测量创建、替换、局部更新、选择、交换、删除、追加和清空大表格等合成工作负载。其 2026 年 Chrome 148 结果中，候选 keyed 实现的汇总方向是：Solid 与 Svelte 的 CPU 因子最接近 Vanilla，Vue/Lit 随后，Preact signals、React 和 Preact hooks 更靠后；Solid 在该轮候选中也表现出较好的内存和加载因子。[Chrome 148 结果页](https://krausest.github.io/js-framework-benchmark/2026/chrome148.html)

这些结果只能用于筛选，不能作为 Atha 的验收结论：测试机器是 macOS/Chrome，而不是 Windows/WebView2；框架版本不完全同代，其中 Vue 结果还是 alpha；表格 DOM 与 EPUB 分页、选区、WebView IPC 完全不同；Vanilla 实现也被基准项目标注为手工 DOM 的性能基线而非推荐写法。故本文只采用“候选分组”，不把具体小数差异包装成 Atha 的性能预测。

## 最小迁移验证门槛

先做短命原型分支，不进行全量重写。验证分两关：

### 第一关：只迁移主机

- 使用 Tauri 2 承载**完全相同**的现有 reader HTML/CSS/JavaScript 产物；
- 复刻现有定制协议、资源根目录、窗口尺寸/DPI、诊断和状态接口；
- 不引入 Svelte，不改分页和排版代码；
- 运行现有 M2/M3 测试、真实 EPUB 样本与 benchmark；
- 验证构建、Windows 安装、首次启动、升级路径以及 WebView2 runtime 前提。

只有这一关通过，才说明 Tauri 的工程收益没有破坏当前阅读器。

### 第二关：增加 Svelte 应用壳

- 只迁移导航、菜单、目录、搜索、笔记、进度和设置等壳层；
- reader kernel 通过窄 adapter 暴露语义状态和命令，不把每页 DOM 节点放入 Svelte 状态；
- 不通过 JSON `invoke` 传输整章 XHTML 或大图片；资源继续走受控协议/原始响应，连续进度或诊断使用 channel；
- 在 4K 与系统缩放场景验证控件遵循系统缩放、书页使用绝对尺寸的现有契约；
- 对键盘、焦点、文本选择、窗口 resize、目录跳转和退出恢复做真实 WebView2 验收。

### 接受条件

现有硬门槛保持不变，不因迁移而放宽：

- 冷启动 P95 不高于 2000 ms；
- 首次稳定显示不高于 750 ms；
- 热打开不高于 120 ms；
- 翻页不高于 50 ms；
- 重排不高于 150 ms；
- 峰值工作集不高于 1024 MiB；
- 书籍脚本、网络、路径越界和未知资源继续被拒绝。

另加一条迁移比较门槛：在同一台 Windows 机器、同一 release 构建、同一本真实 EPUB 上，当前版本与候选版本交错运行至少 10 轮；比较中位数、P95 和**整个进程树**峰值内存。首次稳定、翻页、重排出现超过 5% 且可重复的退化，或进程树峰值增加超过 50 MiB，默认判定迁移失败；若要接受，必须指出具体插件或维护收益并由产品明确换取。这里的 5%/50 MiB 是迁移阶段的建议守门线，不是对所有未来功能的永久指标。

安全验收至少证明：

- 单 WebView 方案中，书籍脚本被移除，closed Shadow DOM 不暴露主壳接口；若未来拆出独立书籍 WebView，该 WebView 没有 capability 与 IPC；
- 主壳 capability 只列出必要 command，不开放通用 shell 或任意路径文件系统访问；
- CSP 不允许远程脚本和未批准来源；
- 既有脚本、网络、目录穿越和未知资源拒绝测试全部通过；
- Rust command 对路径、书籍身份和调用状态重新验证，不信任前端传参。

## 最终决策规则

1. 如果“仅 Tauri 主机”不能通过第一关，保留直接 `wry/tao`，仍可单独在现有页面引入 Svelte 应用壳；成熟前端生态并不以 Tauri 为前提。
2. 如果 Tauri 通过、Svelte 通过，则采用默认组合，并持续保持 reader kernel 无框架依赖。
3. 如果 Svelte 未通过真实性能门槛，用同一接口对照稳定的 Solid 1；不得连同 reader kernel 一起重写。
4. 如果团队能力与招聘成为比性能更强的长期约束，再选择 Vue 3。
5. React、Preact、Lit 只有出现本文列出的特定触发条件时才重开评估，避免为了“生态最大”提前支付运行时和工程复杂度。

这个决策让 Atha 获得 Tauri 的桌面工程生态和 Svelte 的成熟组件能力，同时把最敏感、最难重构的阅读排版内核留在已经可测、可控的原生 DOM 边界中。

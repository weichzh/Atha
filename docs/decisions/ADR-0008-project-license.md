# ADR-0008：项目采用 AGPL-3.0-or-later

## 状态

accepted

## 日期

2026-08-08

## 背景

Atha 即将进入 Android 分发、多格式解析、词典和 CSS 社区阶段，但仓库尚无许可证正文或 package 元数据。项目会使用不同许可证的解析库，也会处理不属于项目代码许可范围的字体、书籍、词典和测试资产。必须先明确第一方代码的授权，再逐项判断组合与分发义务。

## 驱动因素与场景

`A-LIC-01` 的刺激源、刺激、环境、制品、响应与度量记录在活动 change；最高优先目标是所有第一方许可投影唯一一致，同时不把项目许可误当作第三方内容授权。Android 分发、依赖接入和 CSS 社区贡献规则都消费这一决定。

## 决策

1. Atha 第一方代码采用 GNU Affero General Public License v3.0 or later，精确 SPDX 表达式为 `AGPL-3.0-or-later`。
2. 仓库根 `LICENSE` 保存未经修改的 GNU AGPL v3 官方英文全文；不维护内容重复的 `COPYING`。
3. Cargo workspace、独立 P0 crate 与 npm package 显式声明同一 SPDX。README 面向用户说明，`CONTEXT.md` 拥有长期法律 / 分发约束。
4. 第三方代码、生成物、字体、书籍、词典、fixture 和用户内容保留各自许可或权利状态；根许可证不重新许可这些对象。仓库已复制第三方资产的版权与许可文本集中保存在 `THIRD_PARTY_NOTICES.md`。
5. 引入依赖时核对精确 `-only` / `-or-later`、与实际链接和分发方式的兼容性，并履行版权、修改说明、源码、NOTICE 或重新链接义务。仅仅“也是 GPL / AGPL”不构成批准。

## 候选与权衡

- 继续不声明许可证：拒绝。公开可见不等于获准复制、修改或分发，也会阻塞 Android 与第三方组件决策。
- 宽松许可证：不采用。它会允许闭源再分发，不符合用户本次决定。
- 专有许可：不采用。它与公开协作、后续 CSS 社区和用户明确选择冲突。
- `AGPL-3.0-or-later`：采用。它为第一方代码提供明确的强 copyleft 与网络交互源码义务，同时保留采用未来版本的选择。

## 后果

- 正面：源码、manifest、公开说明与依赖决策有单一可机读事实；
- 正面：Android 与网络相关扩展不能绕过对应源码义务；
- 负面：分发必须履行 AGPL，商业闭源组合需要另行决定，贡献与社区仓库也需明确同许可或兼容条款；
- 负面：第三方 `-only`、LGPL、字体和数据许可仍需逐项审查，项目许可不会消除这些成本。

## 假设

- 本决定只覆盖 Atha 第一方代码；当前本机书籍和词典不进入 Git 或公共安装包。
- 首次正式分发前会生成完整依赖 / 资产清单并复核 notices、源码提供和 LGPL 重新链接材料。
- 未签署单独协议的第一方贡献将由对应仓库的明确贡献条款处理；本切片不推断尚未创建的 CSS 社区仓库许可。

## 风险与缓解

- `mdict-rs` 为 `AGPL-3.0-only OR 商业许可`：`-only` 与项目整体的 `-or-later` 表达需要专门兼容性判断，未解决前不进入产品。
- `libmobi` 的 LGPL-3.0-or-later 义务不会被 AGPL 吸收：Android 静态链接前必须设计可重新链接材料或采用可合规的链接 / 替代方案。
- 第三方资产可能缺少再分发权：书籍、词典、字体和 fixture 默认只允许本机 opt-in 测试，除非逐文件证明来源与授权。
- 未来改为双许可或其他许可证可能受既有贡献与已分发版本约束：任何变化都需要新的批准、法律审查与取代 ADR，不能只改一个 manifest 字段。

## 回滚与复查

首次分发前可通过新决定替换仓库当前许可；已分发版本的既有授权不可撤销。出现双许可、CLA、商业分发、CSS 社区独立仓库、`AGPL-3.0-only` 依赖或 LGPL Android 链接时必须复查。

## 取代关系

本 ADR 不取代既有 ADR，也未被其他 ADR 取代。

## 实施与检查位置

- 根许可：`LICENSE`；
- 已复制第三方资产：`THIRD_PARTY_NOTICES.md`；
- package 元数据：根 / member Cargo manifest、P0 manifest 与 `reader/app/package.json`；
- 稳定事实：`README.md`、`CONTEXT.md`；
- 依赖规则：`docs/architecture/DESIGN-GUIDE.md`；
- 当前证据与 review：`docs/changes/project-agpl-license.md`。

## 相关资料

- GNU AGPL：<https://www.gnu.org/licenses/agpl-3.0.html>
- GNU AGPL 纯文本：<https://www.gnu.org/licenses/agpl-3.0.txt>
- SPDX：<https://spdx.org/licenses/AGPL-3.0-or-later.html>
- Cargo manifest：<https://doc.rust-lang.org/cargo/reference/manifest.html#the-license-and-license-file-fields>
- npm package.json：<https://docs.npmjs.com/cli/configuring-npm/package-json/#license>

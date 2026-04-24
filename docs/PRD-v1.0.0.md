# EasyFlowHub PRD v1.0.0

> status: active
> owner: EasyFlowHub maintainers
> last_updated: 2026-04-24
> paired_execution_doc: `docs/roadmap.md`

## 1. 产品定义

- 产品名称：`EasyFlowHub`
- 产品形态：`Windows-first hybrid desktop app`
- 产品定位：`在单机桌面环境中统一快速笔记、待办管理、本地脚本执行，以及面向 AI 工作流的受控集成能力`
- 版本目标：`v1.0.0 先交付稳定、可回归、可交接的核心闭环，再以受控方式纳入 relay / MCP / extensions 等高级能力`

## 2. 问题与机会

当前个人效率与本地自动化流程常被拆散在多个工具中：

- 快速记录和后续整理割裂，临时笔记容易沉没。
- 待办常埋在 Markdown 文本里，缺少统一聚合和回跳入口。
- 脚本能力存在，但缺少稳定的发现、执行、记录与管理面板。
- AI 相关能力正在形成，但如果没有边界约束，容易变成“能演示、难发布、不可维护”的附加复杂度。

EasyFlowHub v1.0.0 要解决的不是“做一个尽可能大的平台”，而是先做成一个稳定的个人桌面工作台。

## 3. 产品目标

### 3.1 必须达成

- 让用户能用全局快捷键快速捕获笔记，并稳定保存。
- 让笔记、待办、脚本、设置都能在同一个 manager 工作台中闭环操作。
- 让桌面关键行为在 Windows 打包产物中也稳定，而不是只在开发态可用。
- 让 relay / MCP / extensions 有清晰边界，能作为 v1 的受控能力存在，而不是变成发布阻塞源。

### 3.2 明确不做

- 不做云同步、多端协作、账号体系。
- 不做扩展中任意第三方代码执行。
- 不做完整的 provider 平台化治理与复杂配额系统。
- 不做独立搜索页、Everything 深度集成、beautification 等非核心生产力能力。

## 4. 目标用户与核心场景

| 用户类型 | 核心诉求 | 典型场景 |
|---|---|---|
| 个人效率用户 | 快速记录、整理笔记、聚合待办 | 用快捷键记下一条想法，稍后在 manager 中整理和继续处理 |
| 本地自动化用户 | 稳定发现和运行脚本，保留执行痕迹 | 在 manager 中查看脚本、执行任务、回看历史 |
| 仓库维护者 / AI 工作流用户 | 在同一壳层里查看 relay / MCP / extension 状态 | 调试本地 AI 基础设施、查看贡献、做受控配置 |

## 5. v1.0.0 范围定义

### 5.1 核心体验范围

| 能力 | v1.0.0 要求 | 发布验收口径 |
|---|---|---|
| Quick note capture | 支持全局快捷键唤起；Markdown 编辑；自动保存；图片插入、删除、展示；历史笔记兼容；关闭当前窗口不误伤其他窗口 | 在开发态与打包产物下均能完成“新建 -> 编辑 -> 保存 -> 关闭 -> 重开确认” |
| Notes manager | 支持查看、编辑、回收站、恢复；窗口切换或重开后数据不明显损坏 | 能完成 CRUD、删除、恢复、再次打开校验 |
| Todo closed loop | 从 Markdown checkbox 解析待办；在 todo 卡片和 manager 中聚合展示；支持完成状态、内联编辑、来源回跳 | 能完成“记笔记 -> 聚合 -> 勾选/编辑 -> 回到原笔记” |
| Manager workspace | manager 作为统一入口稳定可用；核心面板具备一致导航与切换体验 | 主要面板能切换、状态不明显丢失、不会出现入口漂移 |
| Settings baseline | 快捷键、自启动、模块启停等关键设置可配置且可持久化 | 设置修改后可重开验证，核心模块不可被错误禁用 |
| Windows desktop behavior | 自启动、托盘、多窗口、关闭/隐藏行为在打包产物中可用 | 不出现明显“开发态正常、打包后异常” |

### 5.2 自动化工作台范围

| 能力 | v1.0.0 要求 | 发布验收口径 |
|---|---|---|
| Script discovery and execution | 可发现本地脚本、查看详情、触发执行 | 至少有一条标准样例脚本主路径可重复回归 |
| Tasks / history | 能查看执行记录和任务状态 | 执行后可在 manager 中追踪结果 |
| Script validation path | 仓库内保留默认 smoke / 手工验证入口 | 团队成员知道“脚本功能该怎么验” |

### 5.3 高级能力的受控交付范围

| 能力 | v1.0.0 策略 | 不作为 v1 硬门槛的部分 |
|---|---|---|
| Relay | 保留 OpenAI 兼容基础配置、健康状态、失败提示与最小 failover；manager 可查看和保存配置 | 更复杂的 provider 管理、独立密钥托管层、完整平台化治理 |
| MCP | 保留现有集成、类别展示、external server catalog 的只读视图 | 更完整的外部 server 生命周期管理 |
| Extensions | 只承诺 manifest 扫描、贡献聚合、只读审计与有限导入 | 安装/卸载、热重载、启停状态持久化、签名验证 |

## 6. 功能性需求

### 6.1 Quick note

- 必须支持全局快捷键快速唤起。
- 必须支持 Markdown 编辑和自动保存。
- 必须支持图片插入、删除和预览展示。
- 必须保证 `Esc`、`Alt+F4`、窗口按钮等关闭动作只影响当前窗口。
- 必须兼容旧笔记数据，不因为升级导致常见内容异常。

### 6.2 Notes manager

- 必须提供笔记浏览、编辑、删除、回收站、恢复能力。
- 必须允许从 manager 继续整理 quick note 内容。
- 必须以 `notes.rs` 为持久化 owner，避免多处重复写入逻辑。

### 6.3 Todo closed loop

- 必须从笔记 Markdown 中解析 checkbox 待办。
- 必须在 todo 卡片和 manager 两处形成一致的聚合视图。
- 必须支持完成状态切换、必要的内联编辑和来源回跳。

### 6.4 Scripts / tasks

- 必须发现本地脚本并展示脚本元信息。
- 必须支持执行脚本并保留基础记录、状态或时间线。
- 必须固定一条最小可重复的脚本验证路径，避免“功能在但不会验”。

### 6.5 Manager / settings

- manager 必须作为统一入口，而不是继续在多个页面硬分叉。
- `src/modules` 必须继续作为 manager 模块注册和启停配置 owner。
- 核心模块 `notes` 与 `settings` 不可被错误禁用。
- 设置页必须能管理模块启停、关键快捷键与自启动等基础配置。

### 6.6 Relay / MCP / extensions

- relay 的真实选路与 failover 必须继续集中在 `scriptmgr-go/internal/relay`。
- extension 机制在 v1 只能是声明式 manifest，不执行任意第三方代码。
- manager 中的 relay / MCP / extensions 面板以运营、审计、导入辅助为主，不复制后端 owner 逻辑。

## 7. 非功能性需求

| 维度 | 要求 |
|---|---|
| 数据安全 | 笔记和关键配置在窗口切换、关闭、重开后不出现明显损坏或丢失 |
| 可回归 | 核心链路必须存在 smoke、checklist 或测试入口，能被重复执行 |
| 架构边界 | TS / Rust / Go 各层 owner 清晰，不新增重复实现 |
| Windows 适配 | 多窗口、托盘、自启动、打包行为必须优先以 Windows 实际表现为准 |
| 可维护性 | 文档、roadmap、README 与当前交付面保持一致，不让历史方案继续冒充事实来源 |
| 错误反馈 | 配置非法、服务未启动、运行失败等场景必须有可理解反馈，而不是静默失败 |

## 8. 发布判定

### 8.1 发布阻塞项

- Quick note、notes manager、todo、scripts、manager/settings 存在至少一条稳定主路径。
- Windows 打包产物完成基础 smoke，验证自启动、托盘、多窗口、关闭行为。
- 默认 smoke / checklist / 文档入口已同步，维护者知道如何回归。
- `README`、`PRD`、`roadmap` 对外描述不互相冲突。

### 8.2 可带着限制发布

- relay 仍采用 raw JSON 配置，但保存、失败、来源展示清晰。
- MCP 仍以只读运营视角为主，不提供完整生命周期管理。
- extensions 仍停留在声明式贡献聚合，不承诺安装器和动态执行能力。

## 9. 开发阶段划分设计

| 阶段 | 目标 | 核心交付 | 退出条件 | 当前状态 |
|---|---|---|---|---|
| Phase 1. 核心体验稳定化 | 先把用户最直接感知的主链路做稳 | quick note、notes manager、todo、窗口关闭、自启动、托盘 | 通过核心体验 smoke，尤其是打包产物回归 | `[in_progress]` |
| Phase 2. 自动化工作台收口 | 让 scripts / tasks / manager / settings 形成明确可验证主路径 | 脚本样例、执行记录、模块启停、关键设置持久化 | 团队可重复完成脚本执行与 manager 设置回归 | `[in_progress]` |
| Phase 3. 高级能力受控纳入 | 把 relay / MCP / extensions 纳入 v1，但不扩大成平台级工程 | relay 基础回归、MCP catalog、extension contribution aggregation | 高级能力具备最小可用和失败预案，不反向拖垮核心发布 | `[in_progress]` |
| Phase 4. 发版治理闭环 | 把“可跑”收敛为“可发布、可交接、可持续维护” | smoke/checklist、CI 对齐、README 与 docs 同步 | 核心验收结果回填完成，文档与代码口径一致 | `[in_progress]` |

### 阶段依赖

- Phase 1 是发布基础，不应被 Phase 3 的高级能力反向阻塞。
- Phase 2 依赖 Phase 1 的 manager 壳层稳定，但可并行推进脚本样例和设置持久化。
- Phase 3 必须受 PRD 边界约束，只做“受控增强”，不引入新的平台承诺。
- Phase 4 从第一阶段开始同步推进，但必须在最终发版前完成收口。

## 10. 主要风险

| 风险 | 影响 | 应对策略 |
|---|---|---|
| TS / Rust / Go 三层漂移 | 行为和类型定义不一致，容易出现“看起来能用，实际回归失败” | 继续坚持 canonical owner，补齐 typed wrapper、测试和 smoke |
| Windows 行为仅开发态验证 | 打包后出现自启动、托盘、关闭行为异常 | 把打包产物 smoke 作为 v1 发布硬要求 |
| 高级能力范围膨胀 | relay / extensions 变成新的无限工程 | 以“受控交付”而非“完整平台”定义 v1 |
| 文档双源 | README、roadmap、历史 plan 相互冲突 | 让 PRD 负责需求基线，roadmap 负责执行状态，旧 plan 继续降级为参考 |

## 11. 文档分工

- `docs/PRD-v1.0.0.md`：v1.0.0 的正式需求基线与阶段设计。
- `docs/roadmap.md`：当前执行状态、验证债务与下一队列。
- `docs/PROJECT_PROFILE.md`：项目画像、技术栈、验证命令与 owner 拓扑。
- `docs/plans/*.md`：具体实施方案与专题设计，不直接替代 PRD 和 roadmap。

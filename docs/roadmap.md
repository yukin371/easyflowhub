# EasyFlowHub Roadmap

## v1.0.0

### Current Goal

- 在不扩大架构漂移的前提下，完成 manager、quick note、todo、relay、extension 的 v1 闭环。
- 把 Windows 桌面关键路径、Go relay 路径和仓库级 AI 工作流都收敛到可验证、可交接、可回归的状态。
- 用 checkpoint 驱动多人协作，避免多人同时改同一层却没有统一退出条件。

### Active Tracks

#### Track A. Product Stability

- owner scope: `easyflowhub-app/src`, `easyflowhub-app/src-tauri/src`
- 目标：稳定 quick note、notes manager、todo card 的核心使用路径，优先解决用户能直接感知的行为不一致。
- 可并行任务包：
  - `V1-A1` quick note Markdown / 图片 / 历史兼容回归
  - `V1-A2` todo 卡片与管理中心回跳、聚合入口迭代
  - `V1-A3` 多窗口、关闭行为、autostart 与托盘路径稳定化

#### Track B. Relay And Extensions

- owner scope: `scriptmgr-go/internal/relay`, `scriptmgr-go/internal/extensions`, `easyflowhub-app/src/components/manager/relay`
- 目标：把 relay 与扩展系统从“已能演示”推进到“可以持续迭代并有真实回归依据”的状态。
- 可并行任务包：
  - `V1-B1` `[in_progress]` 真实上游回归：流式响应、429、timeout、不同鉴权方式
  - `V1-B2` `[in_progress]` provider 密钥迁移：从明文 JSON 收敛到环境变量或独立密钥层
  - `V1-B3` `[done]` extension 生命周期设计：安装、启停、签名、目录约束
  - `V1-B4` `[done]` extension relay preset 导入：让 manifest 中的 provider / route 贡献可直接进入当前 relay 配置

#### Track C. Quality And Release Readiness

- owner scope: `easyflowhub-app`, `scriptmgr-go`, `.github/workflows`, `docs`
- 目标：把当前“开发态可跑”提升到“发布前有明确验证出口”的状态。
- 可并行任务包：
  - `V1-C1` `[done]` Windows 打包 smoke checklist 与人工验收脚本
  - `V1-C2` `[done]` CI toolchain 对齐，重点修复 Go 版本漂移
  - `V1-C3` `[done]` 建立最小可重复的端到端或 scripted smoke 路径

#### Track D. Docs And AI Workflow Governance

- owner scope: `docs`, root `AGENTS.md`, nearest `MODULE.md`
- 目标：让 roadmap、guardrails、MODULE 文档真正承担协作约束，而不是只作为设计归档。
- 可并行任务包：
  - `V1-D1` `[done]` 补 review checklist、import / ownership checklist
  - `V1-D2` `[done]` 清点并收敛过期 plan / 架构文档，避免双源
  - `V1-D3` `[done]` 评估 `scriptmgr` 是否接管常用验证、截图、文档同步动作

#### Track E. Extension Enhancement（新增）

- owner scope: `scriptmgr-go/internal/extensions`, `scriptmgr-go/internal/relay`, `easyflowhub-app`
- 目标：让应用具备 VSCode 风格的声明式扩展机制，支持贡献聚合、启用/停用、目录监控热重载。
- 参考设计：`docs/plans/2026-04-08-extension-enhancement-plan.md`
- 可并行任务包：
  - `V1-E1` `[in_progress]` **Phase 1** — Contribution Aggregation：已落地 `ContributionAggregator`、`EffectiveConfig()`、`/api/extensions/contributions` 后端主链路，剩余前端真实消费与更细粒度校验待继续推进
  - `V1-E2` `[planned]` **Phase 2** — Activation Model：实现 `state.json` 读写，enable/disable CLI 和 API，manager 面板状态展示
  - `V1-E3` `[planned]` **Phase 3** — File Watching + Hot Reload：引入 fsnotify 监视扩展目录，变更自动触发 registry 刷新和 contributions 重新聚合
  - `V1-E4` `[planned]` **Phase 4** — Install / Uninstall：本地目录安装流程，staging + atomic move，manager 安装/卸载 UI
  - `V1-E5` `[future]` **Phase 5** — Signature Verification：`plugin.sig` detached signature + trust key 管理

### Release Checkpoints

#### CP1. Experience Stability Gate `[active]`

- 目标：桌面核心使用路径达到“日常可用且不频繁破坏已有笔记”的状态。
- 退出条件：
  - quick note 在 `Esc`、`Alt+F4`、工具按钮下都只关闭当前窗口
  - quick note 图片插入、删除、大图展示与旧笔记兼容性完成回归
  - autostart 在 Windows 打包产物下验证通过，不弹终端、不阻塞启动
  - todo 卡片回跳与管理中心主路径没有明显断裂
- 建议分工：
  - 前端 owner 负责编辑体验与 UI 回归
  - Rust / Tauri owner 负责窗口生命周期、自启动、托盘行为
  - 验证 owner 负责 smoke checklist 与结果沉淀

#### CP2. Relay Closed Loop Gate `[active]`

- 目标：relay 与 extension 基础设施达到“可被团队反复使用并有退化预案”的状态。
- 退出条件：
  - relay 完成至少一轮真实上游回归，覆盖流式响应、`429`、`5xx`、timeout
  - manager Relay 面板在服务未启动、runtime 未启动、非法 JSON 下有可接受交互
  - provider API key 不再依赖明文用户配置
  - 扩展系统明确 v1 边界：继续只读 manifest，或升级为受控安装 / 启停
- 建议分工：
  - Go owner 负责 relay 运行时、health、failover 和测试
  - Frontend owner 负责 manager Relay 面板体验
  - Docs / architecture owner 负责 extension 生命周期边界文档

#### CP3. Release Governance Gate `[queued]`

- 目标：让多人并行推进时仍保持发布质量和文档一致性。
- 退出条件：
  - GitHub Actions Go 版本与 `scriptmgr-go/go.mod` 对齐
  - 至少一条标准验证链可被团队重复执行
  - roadmap、guardrails、相关 `MODULE.md` 与当前代码 owner 不冲突
  - 旧设计文档完成收敛，不再与 active 设计并列为事实来源
- 建议分工：
  - CI owner 负责 workflow / toolchain
  - 文档 owner 负责 source-of-truth 清理
  - 主执行 owner 负责把验证结果回填 roadmap

### Recent Progress

- `2026-04-14` 已继续推进 manager 模块化收敛：`SettingsPanel` 不再手写订阅 `moduleRegistry`，改为消费共享 `useToggleableModules()` hook；并新增 `SettingsPanel.test.tsx`，把设置页模块列表渲染与开关仍通过 `moduleRegistry.toggleModule()` 生效的路径固定为回归约束。
- `2026-04-14` 已为 `manager_modules` 单一宿主入口补齐前端自动化回归：`ManagerExtensionEntries.test.tsx` 现覆盖 builtin 命中、`extensions` 回退、详情深链与非 builtin 来源保护；新增 `ExtensionsPanel.test.tsx` 覆盖只读审计目录语义、空态、深链高亮以及扩展列表 / contribution 接口失败时的降级路径；新增 `ManagerPage.test.tsx` 覆盖侧边栏 extension entry 到页面级 `extensions` 面板切换与 `MANAGER_OPEN_EXTENSION_EVENT` 派发，避免 `Extensions` 面板再次被演化成第二个可操作入口。
- `2026-04-11` 已把 Stage B3 再向前推进一段：新增 builtin `Extensions` 面板，集中展示 manifest 扩展、effective `script_roots`、MCP server catalog 和 `manager_modules` 只读审计目录；唯一可操作宿主入口现已收口到 `ManagerSidebar -> ManagerExtensionEntries`，并继续通过统一深链在 `Extensions` 面板中高亮对应扩展卡片。
- `2026-04-11` 已推进 Stage B3 的第一段：manager 开始把 extension `manager_modules` 渲染为只读 extension entry host，显示来源扩展并仅允许跳转到已存在的 builtin panel，不再把它们注入 `src/modules` registry 或 sidebar workspace 列表。
- `2026-04-11` 已推进 Stage B2 的第一段：`internal/mcpcli` 新增 persisted + extension `mcp_servers` 的 effective catalog，`GET /api/mcp/servers` 已可返回 `persisted` / `extension` / `conflicted` 三类条目，manager MCP 面板开始只读展示来源与冲突状态。
- `2026-04-11` 已推进 Stage B1：`internal/discovery` 开始真实消费 extension `script_roots`，脚本发现链路现已合并 repo roots、`roots.json` 和 effective extension roots，不再要求手工把扩展脚本目录导入到 `roots.json`。
- `2026-04-11` 已推进 Track E Phase 1 后端主链路：在 `scriptmgr-go/internal/extensions` 新增 `ContributionAggregator` 与 `EffectiveContributions`，relay runtime 新增 `EffectiveConfig()` merged view，并暴露 `GET /api/extensions/contributions`；前端已补最小 TS 类型与 API wrapper，供后续 manager 消费。
- `2026-04-11` 已新增 `docs/plans/2026-04-11-stage-b-contribution-consumption-plan.md`，把 `script_roots`、`mcp_servers`、`manager_modules` 三类 contribution 的真实消费链、owner 边界与分阶段落地顺序写清，作为扩展平台 Stage B 的 DDD 输入。
- `2026-04-11` 已新增 `docs/plans/README.md` 和 docs 快速开发入口，按“扩展平台 / manager 模块化 / 打包验收”三条路径整理最短阅读链，减少在 `docs/plans` 整目录中翻找上下文的成本。
- `2026-04-11` 已新增 `docs/plans/2026-04-11-vscode-style-extension-platform-roadmap.md`，把当前 Track E 的声明式扩展阶段与后续 SDK / 权限模型 / Isolated Extension Host 的长期演进关系写清，避免“像 VSCode 一样”只停留在口号层。
- `2026-04-10` 已继续推进 manager 模块化：为 `easyflowhub-app/src/modules` 补 `MODULE.md`，新增统一的 registry React hooks，并收敛 builtin module 定义骨架，减少 `ManagerPage` / `SettingsPanel` 对 registry 订阅细节的重复依赖。
- `2026-04-08` 已完成扩展性增强方案设计，新增 `docs/plans/2026-04-08-extension-enhancement-plan.md`，规划 Phase 1-5 实施路径，覆盖贡献聚合、启停状态、热重载、安装卸载和签名验证。
- `2026-04-08` 已在 roadmap 中新增 Track E（Extension Enhancement），对齐 VSCode 风格的 A+B+C 扩展模型（Contribution Points + Activation + File Watching）。
- `2026-04-05` 已修复正文首行被误持久化为标题的问题，并统一管理中心与快速笔记的 Markdown 自动补全逻辑。
- `2026-04-05` 已扩展全局 todo 解析与分组显示，并补上 todo 卡片的来源回跳与内联编辑能力。
- `2026-04-05` 已修正图片落盘、拖拽和窗口关闭路径，优先恢复 quick note 的可见性、可编辑性和可关闭性。
- `2026-04-05` 已切换 autostart 为显式 `--autostart` 后台参数，并让管理窗口在自启动阶段保持隐藏。
- `2026-04-07` 已完成通用全栈 AI 工作流设计、模板包和单文档版 bootstrap 手册。
- `2026-04-07` 已为当前仓库实例化 `PROJECT_PROFILE`、`ARCHITECTURE_GUARDRAILS`、`docs/README`、关键 `MODULE.md` 与增强版 `AGENTS.md`。
- `2026-04-07` 已通过 `ADR-0001` 固化 AI 权威文档顺序、旧大文档退役策略与文档同步规则。
- `2026-04-07` 已落地 `scriptmgr relay` 第一阶段后端骨架，包含 OpenAI 兼容 proxy、加权选路、基础 failover、provider health 与 manifest 扩展扫描。
- `2026-04-07` 已让 `scriptmgr serve` 暴露 relay / extension 管理接口，并在 manager 中新增 Relay 面板，可编辑 `relay.json`、查看 provider health 与扩展清单。
- `2026-04-07` 已完成 `V1-C1`，新增 Windows 打包 smoke checklist 与 `scripts/check-release-artifacts.ps1`，把产物校验和人工验收入口固定下来。
- `2026-04-07` 已完成 `V1-C2`，让 GitHub Actions 的 Go setup 直接读取 `scriptmgr-go/go.mod`，消除 workflow 中的版本硬编码漂移。
- `2026-04-07` 已完成 `V1-C3`，新增 `scripts/run-smoke.ps1` 与 `docs/checklists/scripted-smoke.md`，把默认验证链固定为单入口命令。
- `2026-04-07` 已完成 `V1-B3`，新增扩展生命周期设计文档，明确 managed install、enable/disable、signature 与目录约束的下一阶段边界。
- `2026-04-07` 已完成 `V1-D1`，新增 `docs/checklists/review-checklist.md`，并把 docs 入口与 guardrails 引到这份 review / import / ownership checklist。
- `2026-04-07` 已完成 `V1-D2`，将旧 `scriptmgr` roadmap 收敛为 superseded 指针文档，并把 README / 设计文档入口统一回 `docs/roadmap.md`。
- `2026-04-07` 已完成 `V1-D3`，通过 `ADR-0002` 固化 `scriptmgr-go` 不接管仓库级截图与文档同步，仓库自动化继续由 `scripts/` + `docs/checklists/` 负责。
- `2026-04-07` 已完成 `V1-B4`，manager 可将扩展 manifest 中的 relay provider / route 贡献导入当前编辑器；同名 provider / route 会中止导入并显式报错，新增项会写入 `source=extension:<id>`，且 provider 默认禁用。
- `2026-04-07` 已完成 `V1-B1` 的自动化子步骤：`internal/relay/service_test.go` 已覆盖流式响应、`429` 限流、timeout failover 与 provider 鉴权头回归，并通过 `scripts/run-smoke.ps1`。
- `2026-04-07` 已完成 `V1-B2` 的第一子步骤：relay provider 新增 `api_key_env`，运行时可从环境变量解析密钥并在缺失时 failover；`api_key` 继续仅作兼容字段。
- `2026-04-07` 当前阶段进行中：`V1-B1`，先补 relay 对流式响应、`429`、timeout 与不同鉴权头的自动化回归，再决定是否引入外部真实 provider smoke。
- `2026-04-07` 当前阶段进行中：`V1-B2`，继续把 manager 提示、README 和后续配置迁移收敛到 `api_key_env` 路径。

### Validation Debt

- `CP1 / V1-A3` 需要在 Windows 打包产物中回归验证 autostart，不出现终端窗口或卡住。
- `CP1 / V1-A3` 需要继续回归 quick note 在 `Esc`、`Alt+F4` 与工具按钮下只关闭当前窗口。
- `CP1 / V1-A1` 需要验证 quick note 图片插入、删除、大图展示与旧笔记兼容性。
- `CP1 / V1-A2` 需要继续评估 todo 新建入口是否应进一步下放到管理中心聚合面板。
- `CP2 / V1-B1` 需要在 Codex 与 Claude 侧分别验证新增 MCP 的真实可用性，而不是只完成配置。
- `CP2 / V1-B2` 需要继续把存量配置从明文 `api_key` 迁移到 `api_key_env` 或独立密钥管理层，并决定何时移除兼容字段。
- `CP2 / V1-B1` 需要补 relay 的真实上游回归，包括流式响应、不同鉴权方式、`429` 限流与 timeout 行为。
- `CP2 / V1-B1` 需要验证 manager 中的 Relay 面板在 `scriptmgr serve` 未启动、relay runtime 未启动、配置 JSON 非法三类场景下的人工交互体验。
- `CP2 / V1-B4` 需要补一轮 manager 人工验证，覆盖非法 JSON、重复导入、保存前刷新与保存后回读场景。
- `CP2 / Stage B3` 需要补一轮 manager 人工回归，确认 `ManagerSidebar -> ManagerExtensionEntries` 是 `manager_modules` 唯一可操作宿主入口，而 `Extensions` 面板仅保留只读审计目录。

### Next Queue

- 在 Stage A 接近可用后，按 `docs/plans/2026-04-11-stage-b-contribution-consumption-plan.md` 依次落地 `script_roots`、`mcp_servers`、`manager_modules` 的真实消费链。
- 继续把 manager 面板新增流程收敛到 `src/modules` 标准骨架，评估是否需要为模块元数据、排序和分组补统一约束。
- `V1-A1` 先形成 quick note 图片与旧笔记兼容的回归清单，再决定是否继续补编辑器能力。
- `V1-A3` 先做 Windows 打包 smoke 与 autostart 验证，把“开发态正常”与“产物正常”区分开。
- `V1-B1` 为 relay 增加真实上游回归样例和失败场景记录，避免后续只靠本地 mock。
- `V1-B2` 先定 provider 密钥迁移方案，再拆成 Go runtime、manager UI、文档三个子任务。

## Candidate Next Version

### v1.1.0

- 完成 Track E（Extension Enhancement）Phase 1-4，让扩展系统从"声明式扫描"进化为"可安装、可启停、可热重载"。
- 引入更完整的 Markdown 工具栏和块级编辑增强。
- 补齐 todo 的跨笔记批量操作、排序和过滤能力。
- 持续推进 manager 面板组件化，形成新增模块的标准骨架。
- 将通用 AI 工作流中的验证、回归和文档同步动作收敛为项目内标准命令。
- 在 relay / extension 基础稳定后，再评估更强的 provider 管理与受控扩展生态。

## Collaboration Rules

- 一个任务包只指定一个主 owner；跨 TS / Rust / Go 的工作要拆成配套子任务，不在一个任务里混改所有层。
- 每个任务包在开始前都要明确：影响模块、验证方式、需要同步的文档。
- 每完成一个开发阶段或 checkpoint 子步骤，都先更新本文件，再进入下一阶段。
- checkpoint 未关闭前，不新增同轨道的大范围重构，优先清验证债和退出条件。
- 完成的事项进入 `Recent Progress`；长期有效的边界进入 `docs/decisions`；实现细节进入 `docs/plans`。
- roadmap 只保留 active 与 near-future 内容；如果某项长期停留不动，要么拆小，要么降级到 candidate version。

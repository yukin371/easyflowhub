# EasyFlowHub Roadmap

## v1.0.0

### Current Goal

- 稳定管理中心与快速笔记的 Markdown 编辑体验，减少标题、todo、图片和快捷键行为分叉。
- 修复多窗口、todo 卡片与开机自启动的关键行为问题。
- 建立仓库级 AI 工作流、架构边界和模块上下文基线，降低后续漂移成本。
- 为 API relay 与扩展系统建立可迭代的 owner，避免再次把路由、provider 和插件逻辑散落在多个层。

### Active Tracks

#### Product / Feature

- 持续验证快速笔记图片、大图展示、跨窗口切换与历史兼容性。
- 继续迭代 todo 悬浮卡片与管理中心回跳路径。

#### Architecture / Quality

- 收敛管理中心公共骨架，减少重复 UI 模板。
- 保持前端 wrapper、Tauri 命令与 `scriptmgr-go` 业务 owner 明确分层。
- 用 `PROJECT_PROFILE`、`ARCHITECTURE_GUARDRAILS`、`MODULE.md` 取代分叉文档。
- 让 relay 选路 / failover 固定收敛到 `internal/relay`，manifest 扩展固定收敛到 `internal/extensions`。

#### AI Workflow / Tooling

- 让 `roadmap + guards + module docs` 成为默认上下文入口。
- 对齐 Codex / Claude 的工具栈，并评估 `scriptmgr` 作为统一自动化入口。
- 让 `scriptmgr relay` 与 `extensions list` 成为后续 UI 和自动化可复用的底层入口。

### Recent Progress

- `2026-04-05` 已修复正文首行被误持久化为标题的问题，并统一管理中心与快速笔记的 Markdown 自动补全逻辑。
- `2026-04-05` 已扩展全局 todo 解析与分组显示，并补上 todo 卡片的来源回跳与内联编辑能力。
- `2026-04-05` 已修正图片落盘、拖拽和窗口关闭路径，优先恢复 quick note 的可见性、可编辑性和可关闭性。
- `2026-04-05` 已切换 autostart 为显式 `--autostart` 后台参数，并让管理窗口在自启动阶段保持隐藏。
- `2026-04-07` 已完成通用全栈 AI 工作流设计、模板包和单文档版 bootstrap 手册。
- `2026-04-07` 已为当前仓库实例化 `PROJECT_PROFILE`、`ARCHITECTURE_GUARDRAILS`、`docs/README`、关键 `MODULE.md` 与增强版 `AGENTS.md`。
- `2026-04-07` 已通过 `ADR-0001` 固化 AI 权威文档顺序、旧大文档退役策略与文档同步规则。
- `2026-04-07` 已落地 `scriptmgr relay` 第一阶段后端骨架，包含 OpenAI 兼容 proxy、加权选路、基础 failover、provider health 与 manifest 扩展扫描。
- `2026-04-07` 已让 `scriptmgr serve` 暴露 relay / extension 管理接口，并在 manager 中新增 Relay 面板，可编辑 `relay.json`、查看 provider health 与扩展清单。

### Validation Debt

- 需要在 Windows 打包产物中回归验证 autostart，不出现终端窗口或卡住。
- 需要继续回归 quick note 在 `Esc`、`Alt+F4` 与工具按钮下只关闭当前窗口。
- 需要验证 quick note 图片插入、删除、大图展示与旧笔记兼容性。
- 需要继续评估 todo 新建入口是否应进一步下放到管理中心聚合面板。
- 需要在 Codex 与 Claude 侧分别验证新增 MCP 的真实可用性，而不是只完成配置。
- 需要把敏感密钥从用户配置中的明文形式迁移到环境变量或独立密钥管理层。
- 需要对齐 GitHub Actions 的 Go 版本与 `scriptmgr-go/go.mod`，避免 CI toolchain 漂移。
- 需要补 `relay` 的真实上游回归，包括流式响应、不同鉴权方式、429 限流与 timeout 行为。
- 需要决定扩展清单何时从只读 manifest 升级到受控安装/启停流程。
- 需要验证 manager 中的 Relay 面板在 `scriptmgr serve` 未启动、relay runtime 未启动、配置 JSON 非法三类场景下的人工交互体验。

### Next Queue

- 基于新 guardrails 补一轮 import / ownership / review checklist，减少重复实现进入主干。
- 清点并收敛其他可能过期的设计文档，避免再次形成双源。
- 评估 `scriptmgr` 是否应接管常用验证、截图和文档同步动作。
- 为 relay 增加配置编辑 UI、provider 统计与熔断/恢复策略。
- 为扩展系统补签名、安装目录约束和 manager 面板接入。
- 把 relay provider 的 API key 从明文 JSON 迁移到环境变量映射或独立密钥层。

## Candidate Next Version

### v1.1.0

- 引入更完整的 Markdown 工具栏和块级编辑增强。
- 补齐 todo 的跨笔记批量操作、排序和过滤能力。
- 持续推进面板组件化，形成新增模块的标准骨架。
- 将通用 AI 工作流中的验证、回归和文档同步动作收敛为项目内标准命令。

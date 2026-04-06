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
  - `V1-B1` 真实上游回归：流式响应、429、timeout、不同鉴权方式
  - `V1-B2` provider 密钥迁移：从明文 JSON 收敛到环境变量或独立密钥层
  - `V1-B3` extension 生命周期设计：安装、启停、签名、目录约束
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
- `2026-04-07` 已完成 `V1-D1`，新增 `docs/checklists/review-checklist.md`，并把 docs 入口与 guardrails 引到这份 review / import / ownership checklist。
- `2026-04-07` 已完成 `V1-D2`，将旧 `scriptmgr` roadmap 收敛为 superseded 指针文档，并把 README / 设计文档入口统一回 `docs/roadmap.md`。
- `2026-04-07` 已完成 `V1-D3`，通过 `ADR-0002` 固化 `scriptmgr-go` 不接管仓库级截图与文档同步，仓库自动化继续由 `scripts/` + `docs/checklists/` 负责。
- `2026-04-07` 已完成 `V1-B4`，manager 可将扩展 manifest 中的 relay provider / route 贡献导入当前编辑器；同名 provider / route 会中止导入并显式报错，新增项会写入 `source=extension:<id>`，且 provider 默认禁用。

### Validation Debt

- `CP1 / V1-A3` 需要在 Windows 打包产物中回归验证 autostart，不出现终端窗口或卡住。
- `CP1 / V1-A3` 需要继续回归 quick note 在 `Esc`、`Alt+F4` 与工具按钮下只关闭当前窗口。
- `CP1 / V1-A1` 需要验证 quick note 图片插入、删除、大图展示与旧笔记兼容性。
- `CP1 / V1-A2` 需要继续评估 todo 新建入口是否应进一步下放到管理中心聚合面板。
- `CP2 / V1-B1` 需要在 Codex 与 Claude 侧分别验证新增 MCP 的真实可用性，而不是只完成配置。
- `CP2 / V1-B2` 需要把敏感密钥从用户配置中的明文形式迁移到环境变量或独立密钥管理层。
- `CP2 / V1-B1` 需要补 relay 的真实上游回归，包括流式响应、不同鉴权方式、`429` 限流与 timeout 行为。
- `CP2 / V1-B3` 需要决定扩展清单何时从只读 manifest 升级到受控安装 / 启停流程。
- `CP2 / V1-B1` 需要验证 manager 中的 Relay 面板在 `scriptmgr serve` 未启动、relay runtime 未启动、配置 JSON 非法三类场景下的人工交互体验。
- `CP2 / V1-B4` 需要补一轮 manager 人工验证，覆盖非法 JSON、重复导入、保存前刷新与保存后回读场景。

### Next Queue

- `V1-A1` 先形成 quick note 图片与旧笔记兼容的回归清单，再决定是否继续补编辑器能力。
- `V1-A3` 先做 Windows 打包 smoke 与 autostart 验证，把“开发态正常”与“产物正常”区分开。
- `V1-B1` 为 relay 增加真实上游回归样例和失败场景记录，避免后续只靠本地 mock。
- `V1-B2` 先定 provider 密钥迁移方案，再拆成 Go runtime、manager UI、文档三个子任务。

## Candidate Next Version

### v1.1.0

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

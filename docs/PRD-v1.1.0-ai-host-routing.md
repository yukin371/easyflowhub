# EasyFlowHub PRD v1.1.0 AI Host Routing And Relay

> status: draft
> owner: EasyFlowHub maintainers
> last_updated: 2026-04-25
> paired_execution_doc: `docs/plans/2026-04-25-ai-host-routing-implementation-plan.md`
> supersedes_in_scope: `docs/plans/2026-04-25-claude-codex-relay-switch-design.md`

## 1. 产品定义

- 版本名称：`v1.1.0 AI Host Routing And Relay`
- 能力定位：`为 Claude / Codex 等 AI 宿主提供受控的本地网关切换，并为 OpenAI 兼容流量提供正式的 relay / 负载均衡运行时能力`
- 版本目标：`把宿主配置切换与 relay 运行时都纳入正式需求边界，再决定现有最小实现保留、补强或继续扩展的顺序`

## 2. 问题定义

当前仓库已有三类相关能力，但它们没有形成同一套需求闭环：

- `scriptmgr-go/internal/relay` 已提供 OpenAI 兼容 relay 与基础负载能力。
- Claude / Codex 在用户机器上各自维护独立宿主配置。
- 仓库文档中已经提到 `cc-switch` 思路，但没有正式 PRD 说明到底要“借鉴到什么程度”。

因此当前问题不是“完全没有代码”，而是：

- 需求目标不明确：到底是配置切换器、统一中转，还是完整 `cc-switch` 风格产品。
- 协议边界不明确：Claude 与 Codex 的协议和认证模型并不一致。
- 验收口径不明确：切换成功的判定、relay 路由与 failover 的覆盖范围、失败回滚、配置备份、用户可见反馈都还没有正式定义。

## 3. 产品目标

### 3.1 必须达成

- 让维护者能用统一入口查看 Claude / Codex 当前宿主配置。
- 让维护者能安全地把 Claude / Codex 指向指定 AI 网关或本地 relay。
- 让 OpenAI 兼容流量具备正式承诺的 relay / 负载均衡 / failover 行为，而不是停留在“已有代码但需求未收口”状态。
- 所有配置写入都具备备份和原子替换，避免把宿主配置写坏。
- 明确 OpenAI relay 与宿主配置编排的 owner 边界，避免职责漂移。

### 3.2 明确不做

- 不在 `v1.1.0` 承诺完整复刻 `cc-switch` 的 health orchestration、热切换和多 provider 编排。
- 不在 `v1.1.0` 把 Claude 协议硬塞进现有 OpenAI 兼容 relay。
- 不在 manager 前端先做复杂 UI；先以 CLI 和文档验收主路径为准。
- 不在本版本引入新的密钥托管系统。

## 4. 目标用户与核心场景

| 用户类型 | 核心诉求 | 典型场景 |
|---|---|---|
| 仓库维护者 | 快速把本机 Claude / Codex 切到指定网关 | 切换到本地 relay、企业网关或新的测试代理 |
| AI 工作流用户 | 获得稳定的 OpenAI 兼容中转与基础负载均衡 | 让 Codex 或兼容客户端稳定走本地 relay |
| 发布维护者 | 能判断当前实现是否符合边界，不继续膨胀 | 审核“最小实现”与 relay 运行时是否进入正式版本 |

## 5. 范围定义

### 5.1 v1.1.0 范围内

| 能力 | 要求 | 验收口径 |
|---|---|---|
| Host status inspect | 提供统一命令查看 Claude / Codex 当前配置状态 | 能输出配置路径、当前 base URL、关键字段是否存在 |
| Host switch | 提供统一命令切换 Claude / Codex 到指定上游 | 可按宿主单独切换，也可一次切两个宿主 |
| Relay runtime | 提供正式支持的 OpenAI 兼容 relay 入口 | 能完成 `/v1/*` 请求转发、基础 provider 选择与失败切换 |
| Load balancing | 提供基础加权路由与最小 failover | 至少覆盖 weighted round robin、`429` / `5xx` / timeout 的最小策略 |
| Safe write | 写入前自动备份，写入过程原子替换 | 配置变更后生成备份；异常时原文件不损坏 |
| Boundary clarity | 明确 relay 与 host config 的 owner 边界 | 文档与代码都体现“relay 不拥有宿主配置编排” |

### 5.2 暂不纳入

| 能力 | 暂不纳入原因 |
|---|---|
| 完整 `cc-switch` proxy orchestration | 与 `internal/relay` owner 重叠，需求未证明必须 |
| Claude 协议经由现有 OpenAI relay 透明转发 | 协议边界不成立，强行纳入会产生错误承诺 |
| 前端 manager 配置 UI | 先固定 CLI 合同和验收标准，再决定是否上 UI |
| provider health 驱动自动切换宿主 | 会把“配置编排”升级成“运行时调度”，范围过大 |

## 6. 功能性需求

### 6.1 Host status

- 必须提供一个统一入口查看 Claude / Codex 当前配置状态。
- 必须至少展示：
  - 配置文件路径
  - 当前 base URL
  - 当前 provider / model 的最小必要信息
  - 宿主配置是否存在
- 应支持结构化 JSON 输出，便于脚本和 AI 消费。

### 6.2 Host switch

- 必须支持只切 Claude。
- 必须支持只切 Codex。
- 必须支持一次同时切两个宿主。
- 命令输入必须显式区分 Claude 与 Codex 的目标 URL，不允许用一个字段模糊代替两个协议目标。

### 6.3 Safe write

- 必须在写入前保留原配置备份。
- 必须使用原子替换，避免部分写入导致配置损坏。
- 必须在输入无效、目标文件不可解析等情况下返回明确错误。

### 6.4 Relay runtime and load balancing

- 必须继续提供 OpenAI 兼容 relay 入口，并由 `scriptmgr-go/internal/relay` 作为唯一 owner。
- 必须支持 provider 配置、route 选择、基础 weighted round robin。
- 必须对至少以下失败场景提供最小 failover：
  - `429`
  - `5xx`
  - timeout
- 应暴露 provider health 快照，便于 manager 和 CLI 查看当前运行状态。
- 必须以“受控交付”定义负载均衡能力，只承诺当前实现的最小策略，不虚构更复杂的熔断和配额能力。
- 对外宣称这些能力已“正式支持”前，必须有对应测试或 smoke 证据；不能仅凭代码路径存在就视为完成。

### 6.5 Protocol boundary

- Claude 的配置编排只负责 Claude 宿主可消费的配置格式。
- Codex 的配置编排只负责 Codex 宿主可消费的配置格式。
- `scriptmgr-go/internal/relay` 继续只拥有 OpenAI 兼容 relay 行为，不新增 Claude 协议兼容承诺。

## 7. 非功能性需求

| 维度 | 要求 |
|---|---|
| 安全性 | 不覆盖用户原配置而不留备份 |
| 可回归 | 至少有 `go test` + CLI smoke 的固定验证路径 |
| 边界稳定 | 不把宿主配置编辑逻辑散到 CLI 入口和 relay owner 之外 |
| 可审计 | 文档中明确当前版本做了什么、没做什么 |
| 可扩展 | 后续增加 dry-run / diff / preset、以及更强 relay 策略时不推翻当前 owner 划分 |

## 8. 发布判定

### 8.1 发布阻塞项

- PRD、实施计划、roadmap 已明确“配置切换”和“协议中转”不是同一件事。
- CLI 主路径可验证：
  - `status`
  - 只切 Claude
  - 只切 Codex
- relay 主路径可验证：
  - `scriptmgr relay serve`
  - provider health snapshot
  - 至少一轮 `429` / `5xx` / timeout 自动化或 smoke 证据
- 备份与原子写入行为已由测试或可重复 smoke 覆盖。

### 8.2 可带着限制发布

- 暂时没有 manager UI。
- 暂时没有 dry-run / diff。
- 暂时没有多 preset 切换模型。
- 暂时不承诺复杂熔断恢复、配额调度和非 OpenAI 协议适配。

## 9. 成功指标

- 维护者不再需要手改两个宿主配置文件完成切换。
- 团队能明确判断某项需求属于“宿主配置编排”还是“relay 运行时能力”。
- OpenAI 兼容客户端可通过正式支持的 relay 主路径获得最小负载均衡与 failover。
- 新增需求不会再用“试图引入 `cc-switch`”这种模糊描述直接进入实现，而是先落到需求边界。

## 10. 风险

| 风险 | 影响 | 应对策略 |
|---|---|---|
| 继续把“配置切换”误当成“统一中转” | 方案膨胀，owner 漂移 | PRD 明确拆分两类能力 |
| relay 已有实现但未明确承诺范围 | 后续误判“都支持” | PRD 把当前正式支持的负载均衡能力写死 |
| 宿主配置文件格式变化 | 写入失败或误写 | 保持解析逻辑聚合在单一 owner，并补测试 |
| 用户本地已有自定义复杂配置 | 简单 upsert 可能不覆盖全部场景 | 先定义最小支持面，超出范围明确报错 |
| 现有最小实现与正式 PRD 不一致 | 返工成本上升 | 用 paired execution doc 做逐条审视 |

## 11. 文档分工

- `docs/PRD-v1.1.0-ai-host-routing.md`：正式需求基线。
- `docs/plans/2026-04-25-ai-host-routing-implementation-plan.md`：实施与验收计划。
- `docs/roadmap.md`：当前执行状态与后续收口队列。
- `docs/plans/2026-04-25-claude-codex-relay-switch-design.md`：降级为专题设计参考，不再充当需求基线。

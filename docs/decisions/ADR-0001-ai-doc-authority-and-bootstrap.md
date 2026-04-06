# ADR-0001 AI Doc Authority And Bootstrap

- status: accepted
- date: 2026-04-07

## Context

EasyFlowHub 同时包含 TypeScript、Rust、Go 三层，历史上已经出现过以下问题：

- 旧架构文档与当前代码分叉，误导 AI 和维护者
- 同类能力容易在不同包里重复出现，后期清理成本高
- 设计、计划、标准文档容易越积越多，且更新不同步

仓库需要一套长期有效、可被 AI 快速读取的权威文档顺序和退役机制。

## Decision

1. 仓库级 AI 启动顺序固定为：`AGENTS.md` -> `docs/README.md` -> `docs/PROJECT_PROFILE.md` -> `docs/roadmap.md` -> `docs/ARCHITECTURE_GUARDRAILS.md` -> 最近的 `MODULE.md` -> 相关 `ADR` / `plan`。
2. `PROJECT_PROFILE.md` 负责项目事实、验证命令、入口点和 canonical owner 概览。
3. `roadmap.md` 只维护当前版本目标、活跃轨道、验证债和下一队列，不保留长历史。
4. `ARCHITECTURE_GUARDRAILS.md` 负责硬边界、依赖方向、duplicate owner 禁令和例外流程。
5. `MODULE.md` 只保留模块职责、不变量和高风险陷阱，不扩写全量结构抄录。
6. `docs/plans/*.md` 仅用于短期方案，完成后必须收敛、归档或被替代。
7. `docs/decisions/ADR-*.md` 负责长期有效的 owner、边界和例外决策。
8. 已经过期的大型架构文档不再继续修补，优先改成指针文档或归档。

## Consequences

- 新 AI 会话有固定且可预测的上下文入口。
- 旧大文档失去权威地位，避免继续形成双源。
- 任何跨层或共享能力变更，都必须同步至少一个权威文档。
- 维护者需要持续把短期方案收敛进 `roadmap`、`MODULE.md` 或 `ADR`，否则文档仍会再次腐化。

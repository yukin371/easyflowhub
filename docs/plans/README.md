# Plans Index

本目录存放短中期设计和实施方案，不是长期 source of truth。

使用规则：

- 当前优先级先看 [../roadmap.md](../roadmap.md)
- 长期边界先看 `docs/decisions` 和相关 `MODULE.md`
- 进入具体实施前，再读这里最相关、最新的一两份 plan
- 如果某份 plan 已被更新方案取代，应在新 plan 或 roadmap 中显式指向它，而不是继续并行维护

## Fast Path

### 扩展平台 / VSCode 风格模块化

按这个顺序读：

1. [../roadmap.md](../roadmap.md)
2. [2026-04-11-vscode-style-extension-platform-roadmap.md](./2026-04-11-vscode-style-extension-platform-roadmap.md)
3. [2026-04-08-extension-enhancement-plan.md](./2026-04-08-extension-enhancement-plan.md)
4. [2026-04-07-extension-lifecycle-design.md](./2026-04-07-extension-lifecycle-design.md)
5. [../../scriptmgr-go/internal/extensions/MODULE.md](../../scriptmgr-go/internal/extensions/MODULE.md)
6. [../../easyflowhub-app/src/modules/MODULE.md](../../easyflowhub-app/src/modules/MODULE.md)

用途划分：

- `2026-04-11...`：长期平台蓝图，回答“最终要扩到哪一层”
- `2026-04-08...`：当前 Track E 的实施主线，回答“下一阶段先做什么”
- `2026-04-07 extension lifecycle...`：安装、启停、签名、目录边界，回答“什么不能乱做”

### Relay / Extension 当前落地主线

按这个顺序读：

1. [../roadmap.md](../roadmap.md)
2. [2026-04-07-api-relay-and-extension-phase1.md](./2026-04-07-api-relay-and-extension-phase1.md)
3. [2026-04-08-extension-enhancement-plan.md](./2026-04-08-extension-enhancement-plan.md)
4. [../../scriptmgr-go/internal/relay/MODULE.md](../../scriptmgr-go/internal/relay/MODULE.md)
5. [../../scriptmgr-go/internal/extensions/MODULE.md](../../scriptmgr-go/internal/extensions/MODULE.md)

### Manager 模块化 / 面板接入

按这个顺序读：

1. [../roadmap.md](../roadmap.md)
2. [../../easyflowhub-app/src/modules/MODULE.md](../../easyflowhub-app/src/modules/MODULE.md)
3. [2026-04-11-vscode-style-extension-platform-roadmap.md](./2026-04-11-vscode-style-extension-platform-roadmap.md)
4. 最近一次相关的 `manager` 或面板级 `MODULE.md`

## Active Plans

- [2026-04-11-stage-b-contribution-consumption-plan.md](./2026-04-11-stage-b-contribution-consumption-plan.md)
- [2026-04-11-vscode-style-extension-platform-roadmap.md](./2026-04-11-vscode-style-extension-platform-roadmap.md)
- [2026-04-08-extension-enhancement-plan.md](./2026-04-08-extension-enhancement-plan.md)
- [2026-04-07-extension-lifecycle-design.md](./2026-04-07-extension-lifecycle-design.md)
- [2026-04-07-api-relay-and-extension-phase1.md](./2026-04-07-api-relay-and-extension-phase1.md)

## Reference / Legacy Notes

- `2026-03-*` 下的大部分方案属于历史实施背景，只有在维护对应老模块时才回看。
- `2026-03-23-modular-architecture-design.md` 已 superseded，只在需要追溯早期 `Deskflow` 模块化设计时查看。
- `scriptmgr-roadmap.md`、`scriptmgr-v1.0-roadmap.md` 属于旧路线文档，不应再作为当前优先级来源。
- `UI_REQUIREMENTS.md` 属于大体量参考材料，不适合作为快速开发入口。

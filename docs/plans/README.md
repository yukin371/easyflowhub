# Plans Index

本目录存放短中期设计和实施方案，不是长期 source of truth。

使用规则：

- 当前优先级先看 [../roadmap.md](../roadmap.md)
- 长期边界先看 `docs/decisions` 和相关 `MODULE.md`
- 进入具体实施前，再读这里最相关、最新的一两份 plan
- 如果某份 plan 已被更新方案取代，应在新 plan 或 roadmap 中显式指向它，而不是继续并行维护

## Fast Path

### Windows 原生运行时（native runtime track）

按这个顺序读：

1. [../roadmap.md](../roadmap.md) 的 `Native Runtime Track`
2. [2026-11-25-g0-review.md](./2026-11-25-g0-review.md)（**已批准 2026-11-25**：G0 有条件通过，锁定 egui/eframe 0.31，退出条件 C1–C5）
3. [2026-11-25-c2-ime-acceptance.md](./2026-11-25-c2-ime-acceptance.md)（**待执行**：C2 三种输入法人机验收，Phase 1 第一周硬性 —— 执行清单 + 记录表）
4. [2026-11-25-native-quicknote-parity.md](./2026-11-25-native-quicknote-parity.md)（**已批准 2026-11-25**：快速笔记 T0 parity 清单，即 G1 里"功能 parity"的判据对象；28 条 A 级 + 6 条允许差异，5 项裁决已定案）
5. [2026-11-25-native-shell-spike.md](./2026-11-25-native-shell-spike.md)（**已完成**：S1 原生壳，热唤起 p95 48.9 ms）
6. [2026-11-25-richedit-island-spike.md](./2026-11-25-richedit-island-spike.md)（**已完成**：S2 文本岛，键入 p95 4.95 ms，2 项人工未测）
7. [2026-11-25-notes-db-compat-spike.md](./2026-11-25-notes-db-compat-spike.md)（**已完成**：S3 数据兼容，39 项检查，4 项未测）
8. [2026-11-25-native-architecture-gate-spike.md](./2026-11-25-native-architecture-gate-spike.md)（**已完成**：S4 门禁自检，23 项 + 8 次变异）
9. [2026-11-24-window-lifecycle-and-latency-contract.md](./2026-11-24-window-lifecycle-and-latency-contract.md)（已完成：P0.5 生命周期 + 计时契约，提供现状基线）
10. [2026-11-24-native-runtime-redesign.md](./2026-11-24-native-runtime-redesign.md)（总设计）
11. [../decisions/ADR-0009-windows-native-runtime.md](../decisions/ADR-0009-windows-native-runtime.md)

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

- [2026-11-25-g0-review.md](./2026-11-25-g0-review.md)：**G0 阶段门禁评审（已批准）**。四项 spike 的得分板、14 个未测项按"能否推翻栈选择"分级、4 个决定的裁决（内存 130 MB 接受但 65 不改 / IME 转 Phase 1 第一周硬性 / WAL 保持 delete / 有条件进入 Phase 1），含现网 Tauri 内存实测与三口径说明。
- [2026-11-25-c2-ime-acceptance.md](./2026-11-25-c2-ime-acceptance.md)：**C2 三种输入法人机验收的执行清单（待执行）**。机器起点实测、S2 已证与未证的边界、30 分钟节拍表、6 个观察点、S/A/B/C 定级、以及"什么情况才算 C2 通过"的决策规则；附 `scripts/collect-c2-env.ps1` 采集环境指纹（按 CLSID 识别输入法，2026-11-25 修正过 WeType 漏检）。
- [2026-11-25-native-quicknote-parity.md](./2026-11-25-native-quicknote-parity.md)：**快速笔记 T0 parity 清单（已批准）**。把 G1 里一直没定义的"功能 parity"落成 28 条 A 级可判条目 + 6 条允许差异；5 项裁决定案（切换器留、透明度留、预览延后 Phase 3、多标签页不做、尺寸记忆延后）；并记录现版代码与 `UI_REQUIREMENTS.md` 的两处分叉（自动保存 900 vs 500 ms、多标签页 P0 vs 每窗口一笔记 + 切换器）。
- [2026-11-25-native-shell-spike.md](./2026-11-25-native-shell-spike.md)：S1 原生壳 spike（已完成：热唤起 p95 48.9 ms，3 项未测）。
- [2026-11-25-richedit-island-spike.md](./2026-11-25-richedit-island-spike.md)：S2 文本岛 spike（已完成：键入 p95 4.95 ms，2 项人工未测）。
- [2026-11-25-notes-db-compat-spike.md](./2026-11-25-notes-db-compat-spike.md)：S3 数据兼容 spike（已完成：39 项检查，4 项未测）。
- [2026-11-25-native-architecture-gate-spike.md](./2026-11-25-native-architecture-gate-spike.md)：S4 架构门禁 spike（已完成：23 项自检 + 8 次变异，补 3 个真实缺口）。
- [2026-11-24-window-lifecycle-and-latency-contract.md](./2026-11-24-window-lifecycle-and-latency-contract.md)：P0.5 已交付（静置 600 → 262 MB、Tauri 热唤起基线 87/101/130 ms）。
- [2026-11-24-native-runtime-redesign.md](./2026-11-24-native-runtime-redesign.md)：WebView → Windows 原生单进程的重构设计（ADR-0009 `accepted`，G0 已通过，分阶段实施）。含需求分档、`lite` 低端降级、选型矩阵、架构门禁、Phase 0–4 与 G0–G4；Phase 0 已完成，当前在 Phase 1。
- [2026-10-19-architecture-and-file-size-review.md](./2026-10-19-architecture-and-file-size-review.md)：架构审查证据、大文件职责拆分、数据完整性风险和分阶段验收；审查已完成，修复待实施。
- [2026-10-18-v0.2.0-checkpoint-summary.md](./2026-10-18-v0.2.0-checkpoint-summary.md)
- [2026-10-16-v0.2.0-product-delivery-plan.md](./2026-10-16-v0.2.0-product-delivery-plan.md)
- [2026-04-25-ai-host-routing-implementation-plan.md](./2026-04-25-ai-host-routing-implementation-plan.md)
- [2026-04-24-v1-task-board.md](./2026-04-24-v1-task-board.md)
- [2026-04-24-v1-delivery-plan.md](./2026-04-24-v1-delivery-plan.md)
- [2026-04-11-stage-b-contribution-consumption-plan.md](./2026-04-11-stage-b-contribution-consumption-plan.md)
- [2026-04-11-vscode-style-extension-platform-roadmap.md](./2026-04-11-vscode-style-extension-platform-roadmap.md)
- [2026-04-08-extension-enhancement-plan.md](./2026-04-08-extension-enhancement-plan.md)
- [2026-04-07-extension-lifecycle-design.md](./2026-04-07-extension-lifecycle-design.md)
- [2026-04-07-api-relay-and-extension-phase1.md](./2026-04-07-api-relay-and-extension-phase1.md)

## Release Planning

当前个人效率版本按这个顺序读：

1. [../PRD-v0.2.0.md](../PRD-v0.2.0.md)
2. [2026-10-16-v0.2.0-product-delivery-plan.md](./2026-10-16-v0.2.0-product-delivery-plan.md)
3. [../roadmap.md](../roadmap.md)
4. [../PROJECT_PROFILE.md](../PROJECT_PROFILE.md)
5. [../PRD-v1.0.0.md](../PRD-v1.0.0.md)（既有发布治理基线）
6. 最近一次相关的专题 plan

## Reference / Legacy Notes

- `2026-03-*` 下的大部分方案属于历史实施背景，只有在维护对应老模块时才回看。
- `2026-03-23-modular-architecture-design.md` 已 superseded，只在需要追溯早期 `Deskflow` 模块化设计时查看。
- [../archive/plans/2026-04-22-ai-cli-facade-design.md](../archive/plans/2026-04-22-ai-cli-facade-design.md) 与 [../archive/plans/2026-04-22-ai-cli-facade-impl-plan.md](../archive/plans/2026-04-22-ai-cli-facade-impl-plan.md) 属于已 deferred 的 v0.3 AI CLI facade 草案，不是当前 v1.0/v1.1 交付依据。
- `scriptmgr-roadmap.md`、`scriptmgr-v1.0-roadmap.md` 属于旧路线文档，不应再作为当前优先级来源。
- `UI_REQUIREMENTS.md` 属于大体量参考材料，不适合作为快速开发入口。

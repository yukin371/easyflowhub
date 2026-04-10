# Deskflow 模块化架构设计

> status: superseded
> date: 2026-03-23
> superseded_by:
> - `easyflowhub-app/src/modules/MODULE.md`
> - `docs/plans/2026-04-11-vscode-style-extension-platform-roadmap.md`

## 说明

这是项目仍使用 `Deskflow` 命名时期的早期模块化设计文档。

它仍保留历史价值，但不再作为当前实现或后续扩展平台的事实来源，原因是：

- 文档中的命名、目录和边界基于旧阶段，容易与当前 `EasyFlowHub` 代码混淆
- 对“插件系统预留”的描述已经被后续更明确的扩展平台文档替代
- 当前 manager builtin module 和未来扩展平台已经拆成两个不同 owner，不应继续用一份旧文档混讲

## 当前应阅读的文档

如果你要继续推进 manager 模块化，请读：

1. `docs/roadmap.md`
2. `docs/ARCHITECTURE_GUARDRAILS.md`
3. `easyflowhub-app/src/modules/MODULE.md`

如果你要继续推进 VSCode 风格扩展平台，请读：

1. `docs/roadmap.md`
2. `docs/plans/2026-04-11-vscode-style-extension-platform-roadmap.md`
3. `docs/plans/2026-04-08-extension-enhancement-plan.md`
4. `docs/plans/2026-04-07-extension-lifecycle-design.md`
5. `scriptmgr-go/internal/extensions/MODULE.md`

## 保留原因

- 作为早期架构演进记录
- 供需要追溯历史设计取舍时参考

除此之外，不应再把本文件当作 active design 使用。

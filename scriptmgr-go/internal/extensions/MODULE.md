# internal/extensions MODULE

> status: active
> owner: EasyFlowHub script runtime maintainers
> last_verified: 2026-04-11

## 职责

`internal/extensions` 负责扫描 manifest 型扩展、返回贡献清单，并通过 `ContributionAggregator` 生成统一的 `EffectiveContributions` 视图，供 relay / MCP / manager 等后续消费链复用。

## 相关文档

- 当前版本目标与阶段顺序：`docs/roadmap.md`
- 长期平台路线：`docs/plans/2026-04-11-vscode-style-extension-platform-roadmap.md`
- 当前声明式扩展实施主线：`docs/plans/2026-04-08-extension-enhancement-plan.md`
- Stage B 真实消费链方案：`docs/plans/2026-04-11-stage-b-contribution-consumption-plan.md`
- 生命周期边界：`docs/plans/2026-04-07-extension-lifecycle-design.md`

## 数据流

extension roots -> scan `plugin.json` -> validate manifest -> mark loaded/invalid -> `ContributionAggregator.Merge()` -> `EffectiveContributions`

## 约定 & 陷阱

- v1 只允许声明式 manifest，不执行任意第三方代码。
- duplicate extension id 必须显式报错，避免不同根目录 silently 覆盖。
- 贡献发现和安装执行是两件事；当前模块只负责前者。
- effective contribution 只聚合 `status == loaded` 且 manifest 可用的扩展项；单条非法 contribution 应被局部过滤，不能拖垮全局视图。

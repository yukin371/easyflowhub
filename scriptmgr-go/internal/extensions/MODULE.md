# internal/extensions MODULE

> status: active
> owner: EasyFlowHub script runtime maintainers
> last_verified: 2026-04-07

## 职责

`internal/extensions` 负责扫描 manifest 型扩展、返回贡献清单，并为 relay / MCP / manager 等后续能力提供统一的扩展发现入口。

## 数据流

extension roots -> scan `plugin.json` -> validate manifest -> mark loaded/invalid -> expose contributions to callers

## 约定 & 陷阱

- v1 只允许声明式 manifest，不执行任意第三方代码。
- duplicate extension id 必须显式报错，避免不同根目录 silently 覆盖。
- 贡献发现和安装执行是两件事；当前模块只负责前者。

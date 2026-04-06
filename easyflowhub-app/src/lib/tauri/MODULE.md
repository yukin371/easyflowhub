# lib/tauri MODULE

> status: active
> owner: EasyFlowHub frontend maintainers
> last_verified: 2026-04-07
> verified_against: e524f8522ecf0a15e40af3e6f7627a34e319e81d

## 1. Responsibility

`src/lib/tauri` 是前端访问 Tauri 原生命令的唯一 typed bridge，负责把 UI / hooks 与 Rust 命令层解耦。

## 2. Owns

This module is the canonical owner of:

- 前端侧 `invoke()` 命令封装
- Tauri 命令的 typed request / response 适配
- 前端对窗口、笔记、脚本、设置、外观、小组件等原生命令的统一入口

## 3. Must Not Own

This module must not contain:

- 管理中心页面状态
- 业务工作流编排
- 复杂解析逻辑或 UI 渲染逻辑

## 4. Entry Points

| Entry point | Role | Notes |
|---|---|---|
| `index.ts` | barrel export | 前端统一导出 wrapper |
| `notes.ts` | notes bridge | 笔记 CRUD、回收站、图片落盘、窗口辅助 |
| `scriptmgr.ts` | script runtime bridge | 列表、详情、执行、MCP API |
| `settings.ts` / `appearance.ts` / `widget.ts` | native feature bridge | 设置、窗口外观、桌面小组件 |

## 5. Key Dependencies

| Dependency | Why it exists | Risk if changed |
|---|---|---|
| `@tauri-apps/api/core` | 提供 `invoke()` 能力 | 命令名、参数名或返回形状变动会直接破坏前端 |
| `src/types/*` | 提供 typed response | TS 类型与 Rust / Go 返回值不一致时易出现静默偏差 |
| `src-tauri/src` | Rust 命令实现 | 新命令若未注册或重命名，wrapper 会直接失效 |

## 6. Dependents

Main callers / consumers:

- `src/hooks`
- `src/components`
- `src/modules`

## 7. Invariants

These must remain true after changes:

- 前端 feature 代码优先调用 wrapper，而不是直接硬编码 `invoke('command')`
- wrapper 的函数名应表达业务意图，而不是泄露底层零散命令细节
- Rust 命令签名变化时，wrapper 与类型定义必须同步更新

## 8. Common Pitfalls

- 只改 Rust 命令名而忘记更新 wrapper
- 在组件中临时写新 `invoke()`，后续导致同类命令散落
- 返回值 shape 变化但未同步更新 `src/types`

## 9. Reuse Rules

Before adding new code here, check:

- 是否已有同类 wrapper 可扩展
- 新能力是否真的属于原生命令 bridge，而不是解析器或 feature 状态
- 是否需要同时补测试或类型定义

## 10. Verification

- Primary verification: `cd easyflowhub-app && bun run test`
- Secondary verification: `cd easyflowhub-app && bunx tsc --noEmit`

## 11. Doc Sync Triggers

Update this file when any of these change:

- wrapper 目录职责
- 原生命令 owner 规则
- 依赖方向
- 关键不变量
- 常见坑

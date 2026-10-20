# lib/tauri MODULE

> status: active
> owner: EasyFlowHub frontend maintainers
> last_verified: 2026-10-19
> verified_against: working tree (inbox delete + resource folder bridge)

## 1. Responsibility

`src/lib/tauri` 是前端访问 Tauri 原生命令的唯一 typed bridge，负责把 UI / hooks 与 Rust 命令层解耦。

## 2. Owns

This module is the canonical owner of:

- 前端侧 `invoke()` 命令封装
- Tauri 命令的 typed request / response 适配
- 前端对窗口、笔记、脚本、设置、外观、小组件等原生命令的统一入口
- Inbox 捕获项与附件的唯一前端命令入口

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
| `inbox.ts` | capture bridge | 捕获、活动列表、归档 / 恢复、回收站列表 / 恢复 / 清理、幂等转笔记、打开附件或链接、打开资源文件夹 |
| `scriptmgr.ts` | script runtime bridge | 列表、详情、执行、MCP API |
| `settings.ts` / `appearance.ts` / `widget.ts` | native feature bridge | 设置、窗口外观、桌面小组件 |
| `launcher.ts` | launcher bridge | 全局入口配置、前台排除、拖动/隐藏/置顶、唤起 / 聚焦 / 隐藏事件、Snow Shot 截图会话调用与安装页 |
| `fileSearch.ts` | file search bridge | 受控单根目录索引、搜索 / 预览 / 最近文件，以及带 generation 校验的文件操作 |
| `clipboard.ts` | clipboard bridge | 通过 native shell 读写系统文本剪贴板；路径复制使用严格 native 写入，Launcher 上下文和结果复制使用 native-first 浏览器回退 |
| `voice.ts` | voice capture bridge | 录音会话生命周期、状态轮询、打开录音与麦克风隐私设置 |
| `transcription.ts` | transcription bridge | 按 Inbox item 转写音频附件与读取已有转写文本；音频字节与 Provider 请求都在 native 侧 |
| `logging.ts` | diagnostics bridge | 读取 / 清空内存日志、切换开发者模式、打开日志目录 |

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
- Inbox UI 和 Launcher 动作都通过 `inbox.ts`；不要在组件中直接调用 Inbox 原生命令
- 永久删除与“打开资源文件夹”同样只经 `inbox.ts`；组件不得自行拼接 app-data / assets 路径，文件清理归 native `delete_inbox_item`
- 转写只通过 `transcription.ts` 按 Inbox item 调用，不在组件中自行读取音频文件或拼接 app-data 路径；发送前的 Provider / 隐私确认是 UI 责任，不得绕过
- Launcher 的关闭按钮 / Esc、重复全局入口和系统关闭请求都由原生 `dismiss_launcher` 隐藏并发送 `launcher-hidden`（空 payload）。页面通过 `onLauncherHidden` 取消当前会话请求、清理临时结果，并在卸载时取消监听；不得仅依赖前端关闭按钮来取消，也不得因失焦隐藏。

## 8. Common Pitfalls

- 只改 Rust 命令名而忘记更新 wrapper
- 在组件中临时写新 `invoke()`，后续导致同类命令散落
- 返回值 shape 变化但未同步更新 `src/types`
- 路径复制不能在组件中直接依赖 `navigator.clipboard`；统一调用 `clipboard.ts`，以覆盖 Windows 打包态 secure-context 和剪贴板占用错误。Launcher 读取上下文和复制动作也必须通过该 bridge，浏览器回退只用于开发预览。

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

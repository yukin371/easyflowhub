# Manager Settings Module

> status: active
> owner: EasyFlowHub maintainers
> last_verified: 2026-10-20

## Scope

`SettingsPanel.tsx` 只保留导航、分区组合与页面级反馈（加载失败横幅、重试）。
各设置域的加载 / 草稿 / 提交逻辑与视图拆分到本目录：

- 域 hooks：`useAppSettings`（普通设置聚合）、`useLauncherSettings`（Launcher 触发）、
  `useShortcutSettings`（快捷键配置）、`useShortcutsSettings`（快捷键区两域的提交编排）、
  `useAutostartSettings`（自启动）、`useDebugLogs`（开发者日志）
- 视图：`*Section.tsx`，加共用数字输入 `NumberSettingInput.tsx`
- 跨域共享卡片沿用 `../shared/SettingsSectionCard`

## Invariants

- 每个持久化域只有一个提交入口：
  - 普通设置聚合（quick_note / trash / editor / todo / translation / transcription / developer）
    -> `useAppSettings.save()`，经 typed bridge `lib/tauri/settings` 落到 native 单事务写入；
  - Launcher 触发 -> 快捷键区“保存配置 / 恢复默认”，经 `lib/tauri/launcher` 的
    `apply_launcher_trigger_settings`（native 内先注册再保存）；
  - 快捷键配置 -> 同一按钮内的 localStorage 落盘（`types/shortcut`）。
- F08：普通保存不夹带未提交的 Launcher 草稿。`useAppSettings.save()` 只回写
  `launcher_trigger` 的已提交镜像；Launcher 提交成功后通过 `onLauncherCommitted`
  刷新该镜像，避免后续普通保存覆盖刚提交的 Launcher 值。
- UI 组件不散落 raw `invoke()`；原生命令只经 `src/lib/tauri` typed bridge。

## Verification

- `SettingsPanel.test.tsx`（含 F08 回归）、`settings/useAppSettings.test.ts`、
  `settings/useShortcutsSettings.test.ts`；
- `bunx tsc --noEmit`；`scripts/check-theme-tokens.ps1` 覆盖本目录全部 Section 文件。

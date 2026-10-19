# Action Registry

## Responsibility

`features/actions` owns the serializable action contract and the in-memory
catalog consumed by Launcher. It provides registration, filtering,
search ranking, recent/favorite usage state, and normalized execution results.

## Boundaries

- Runtime executors are injected by feature owners; this module does not call
  Tauri commands, ScriptMgr HTTP APIs, or React components directly.
- Only favorite and recent **IDs** are persisted in `localStorage` under
  `easyflowhub.action-registry.v1`; action metadata and executable functions
  remain owned by their source feature.
- `src/modules` remains the manager panel registry. This registry is for
  Launcher actions and must not be used to enable or disable manager panels.
- Built-in metadata such as `SNOW_SHOT_ACTION` belongs here, while its native
  process detection and MCP screenshot-session calls remain owned by the Tauri bridge and Launcher
  feature adapter.
- `CAPTURE_CLIPBOARD_ACTION` is metadata only. `LauncherPage` owns its executor and persists the clipboard snapshot through `src/lib/tauri/inbox.ts`.
- `TRANSLATE_TEXT_ACTION` reuses Relay through `features/translation`; settings and credentials stay outside registry metadata. Only actions that can stop their work advertise `metadata.cancellable`.
- Launcher now calls `execute()` for built-ins and scripts. `ActionTextOutput` is the shared text result contract for preview, copying and Inbox capture; plain script output and translation share these exits.
- `ActionContextSnapshot` is serializable metadata for clipboard, selection, current file, screenshot and foreground window. Capture payload variants live in `src/types/capture`; the native Inbox DTO remains the persistence boundary.

## Usage

Import `actionRegistry` for the shared catalog, or instantiate `ActionRegistry`
with an explicit storage adapter in tests. Register an `Action` plus an
optional `ActionExecutor`, then call `search`, `getRecent`, `getFavorites`, or
`execute` from a Launcher adapter.

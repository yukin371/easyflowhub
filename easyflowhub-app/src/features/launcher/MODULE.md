# Action Launcher

## Responsibility

`features/launcher` owns the fast, keyboard-first action surface. `LauncherPage`
loads ScriptMgr entries into the shared Action Registry, supplies execution
adapters for script and Snow Shot actions, and keeps launcher window concerns
separate from the Manager workbench.

## Boundaries

- `LauncherPanel` is a presentational interaction surface; it does not call
  Tauri commands directly.
- Action executors can return the shared `ActionHandlerResult`; portable text
  outputs replace the action grid with selectable, scrollable text. Copy and
  collect callbacks stay in the page adapter, with one save per displayed result.
- `LauncherPage` is the feature adapter and may use `src/lib/tauri/scriptmgr`.
- Snow Shot capture delegates to `src/lib/tauri/launcher`; native code discovers
  the installation and uses its adjacent MCP bridge to enter the visible capture
  session. MCP-disabled errors must not be treated as an installation failure.
- `src/modules` remains the Manager registry and is not used for launcher
  actions.
- Windows global shortcut and launcher window lifecycle are owned by Tauri;
  `lib/tauri/launcher` owns the typed bridge for drag, hide, pin and activation events.

## Window Interaction Invariants

- A native reveal emits `launcher-opened`; the page forwards an activation token
  so the persistent webview resets its search and restores keyboard focus on every reveal.
- All native hide paths emit `launcher-hidden`, including repeated triggers and
  system close. The page aborts translation and clears context/results on hide.
- When the native window receives focus, `launcher-focused` restores DOM focus to
  the search field without clearing the active query or filter. A displayed text
  result retains its focus and selection instead; only Escape is intercepted so
  native text selection, scrolling and copying keep working.
- Native window activation and DOM input focus are both required. Focusing an input
  once on mount is insufficient when the window is repeatedly hidden and shown.
- Keep header dragging on a single primary `mousedown` path, excluding controls.
  Stop propagation to avoid a second drag from Tauri's injected document listener.
- Do not hide on blur or when clicking the transparent window margin: WebView2 may
  briefly change focus during native dragging, and empty canvas clicks are not close commands.
  The close button, Escape and repeated global trigger dismiss the panel.
- Keep the native Launcher window close to the panel dimensions. The panel fills
  the viewport so the app does not leave a large transparent hit area around it.
- Pinning is opt-in, lasts for the current native process and does not affect Manager.
- Search stays mounted while visually collapsed to receive ordinary typing and IME
  composition. Native operations remain in the page/bridge rather than the panel.
- Cancellation, dismissal, activation reset and unmount invalidate pending panel
  execution generations. Late promises cannot overwrite a newer result or clear
  its busy state. The page owns aborting the underlying work, and result callbacks
  use the same isolation when the user returns to actions or reopens the window.
- Show cancellation only for the running action's explicit `cancellable` capability
  and an available page abort callback. Native/script side effects that cannot be
  stopped must not offer a misleading cancel control.

## Current Scope

- ScriptMgr-backed actions, Snow Shot capture, clipboard collection and Relay translation. Clipboard collection reads an image lazily at execution time (for example, a screenshot copied from Snow Shot) and falls back to the text snapshot captured when the Launcher opened; native Inbox remains the owner of size validation and persistence.
- Every action uses `actionRegistry.execute()`; translation and script text output
  share result copying and Inbox collection. Translation capture preserves source and translated text.
- Search, recent actions, favorites, keyboard navigation, parameter prompts,
  clipboard context, execution cancellation and portable text results with copy
  and collect feedback.
- The action contract leaves room for URL, text, file, command, note, and todo
  actions in later iterations.

# Appearance Hooks

> Last updated: 2026-10-18

## Responsibility

`useAppearance` keeps persisted theme and window appearance settings synchronized with the current Tauri webview. This summary is inferred from the hook and its `App` consumer.

## Data flow

Tauri appearance bridge -> `useAppearance` -> document `data-theme` / `data-blur` and color scheme -> shared window surface tokens.

## Conventions & pitfalls

- `App` mounts the hook in every window, so each webview applies the same persisted theme and blur preference.
- Appearance updates must be broadcast between mounted hook instances and persisted before the document attributes change.
- Window surface CSS uses `data-blur="off"` for opaque surfaces; changing the setting only in a preview does not change the actual window.

## Dependency rule

- Hooks may depend on `src/types`, `src/lib` and other hooks only. They must not import from `src/components` or `src/pages`; shared note types now live in `src/types/notes.ts` and pure helpers in `src/lib/notesUtils.ts`.
- A hook that needs panel-owned logic should have that logic promoted to `src/lib` first, so both the hook and the panel consume the same leaf module.

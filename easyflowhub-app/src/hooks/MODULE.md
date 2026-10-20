# Hooks

> Last updated: 2026-10-20

## Responsibility

- `useAppearance` keeps persisted theme and window appearance settings synchronized with the current Tauri webview.
- `useTodos` (F05) is the single orchestration owner for todo loading, done-retention filtering, toggle, text edit and inbox-note writes. Both `TodoPanel` (Manager) and `TodoCardPage` (floating window) consume it instead of duplicating the read-modify-write flow; inbox identity creation is delegated to the native atomic `ensure_todo_inbox_note` command.
- `useAutoSave` (F04) owns note save versioning: a token plus a serialised promise chain ensures a slow save's response never clears a newer backup or refills the editor draft of a newer session.
- `useBackupRestore` inspects the localStorage backup written by `useAutoSave` and offers keep / restore / merge when a note is reopened.

## Todo write safety (F05)

- `lib/todoParser.locateTodoLine` re-finds a snapshot todo by **text identity** in the freshly-read note body; a stale `lineIndex` is only a hint. Duplicate todo text is always reported as `conflict` — the hook refuses to guess which line to modify.
- Writes go through native `save_note` with `expected_updated_at` (compare-and-swap). A mismatch surfaces a `note_conflict:` error; the hook reloads and returns a readable message instead of silently overwriting another window's write.
- Inbox initialization goes through typed `ensureTodoInboxNote()`; native SQLite `BEGIN IMMEDIATE` serializes the check-and-create across WebViews and reuses the newest active note whose trimmed title is `待办箱`.
- `resolveTodoLine` / `mutateTodo` inside the hook are the only place that maps lookup failures (`conflict` / `missing` / `stale`) to user-facing messages; panels render them and keep their own view state.

## Conventions & pitfalls

- `App` mounts `useAppearance` in every window, so each webview applies the same persisted theme and blur preference.
- Appearance updates must be broadcast between mounted hook instances and persisted before the document attributes change.
- Window surface CSS uses `data-blur="off"` for opaque surfaces; changing the setting only in a preview does not change the actual window.
- `useAutoSave` drops stale responses by design: do not "fix" a missing `onSaved` call by removing the token check — that reintroduces F04.
- `useTodos` refreshes the list after every failed write; callers must not assume the pre-write snapshot is still current.

## Data flow

Tauri appearance bridge -> `useAppearance` -> document `data-theme` / `data-blur` and color scheme -> shared window surface tokens.

## Dependency rule

- Hooks may depend on `src/types`, `src/lib` and other hooks only. They must not import from `src/components` or `src/pages`; shared note types now live in `src/types/notes.ts` and pure helpers in `src/lib/notesUtils.ts`.
- A hook that needs panel-owned logic should have that logic promoted to `src/lib` first, so both the hook and the panel consume the same leaf module.

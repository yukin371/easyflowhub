# Files Manager

> status: active
> owner: EasyFlowHub frontend maintainers

## Responsibility

`FilesPanel` provides a Windows-first UI for searching and managing files inside user-selected roots. Native indexing and filesystem operations belong to the `src/lib/tauri/fileSearch` bridge and its Rust implementation.

## Data flow

The panel loads root/index status through the bridge, sends debounced queries to the native index, and requests a preview only for the selected result. File operations always pass a canonical result path back through the bridge; the UI does not construct shell commands or access indexed files directly.

## Conventions and pitfalls

- Search is scoped to the single configured root; adding or removing a root must never imply deleting disk content. Copy and move destinations must also stay inside this root.
- Removing an indexed root, pausing its index, clearing index records, and moving a file to the Windows Recycle Bin are separate operations with separate feedback.
- The first index scan is explicit. Successful file mutations rescan the root; pausing preserves existing results, but a mutation while paused clears the in-memory index until the user resumes and refreshes.
- Recent entries are separate SQLite records, not index entries. They are recorded only after a Files-panel open succeeds, are limited to the current root when displayed, and do not contribute to the indexed count.
- The current entry point is this Manager panel. Launcher file search is outside this slice; mutation controls currently apply to regular files, not directories.
- Native permission, conflict, out-of-root, and preview errors must remain visible in the panel. Do not silently discard operation failures.
- Copy-path actions use the native clipboard bridge so packaged Windows builds do not depend on WebView secure-context permissions; clipboard errors remain visible in the panel.
- Disable per-file controls while the same path is being changed, and disable root/index controls during a global operation.
- Previews are limited to bridge-supported text and small image results; image previews arrive as bounded data URLs so arbitrary user directories do not need to be added to Tauri's asset protocol scope. All other file types remain openable without loading file contents into the webview.
- Keep the result list compact and responsive. Avoid nesting decorative cards around each file row.

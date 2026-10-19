# Notes Manager Views

> status: active

## Responsibility

`components/manager/notes` owns the Notes workspace views: the capture Inbox and the existing searchable Markdown notes archive. It does not own persistence or Launcher action execution.

## Data Flow

Inbox UI -> typed `lib/tauri/inbox` bridge -> Rust Inbox commands -> additive tables in `notes.db`; image and file bytes remain under app data `assets/`. Converting an Inbox item to a note is a single native transaction and preserves existing `asset:<uuid>` image references.

## Conventions and Pitfalls

- Keep Inbox inside the Notes workspace tabs; do not add another top-level Manager sidebar entry for it.
- Captures retain their original source/context metadata and are not deleted when archived or converted.
- Files are opened through their attachment id; never build an app-data path from a user-provided filename.
- Snow Shot does not report its result path to EasyFlowHub. A copied screenshot enters Inbox through the normal image-paste path.
- Reject empty or oversized clipboard images before reading their contents; native code enforces the same limit before decoding. Feedback must remain visible in both active and archived views and use theme tokens.
- Inbox items with an audio attachment render the shared `TranscribeControl` (`components/manager/shared`); this view never reads audio bytes or calls the provider itself. Transcription follows ADR-0008: the user confirms provider and privacy before anything is sent, and transcribing refreshes the list so the stored transcript attachment appears.
- Permanent delete is the only destructive Inbox action and always asks for confirmation first; the native command removes attachment files unless the item was already converted to a note (the note body still references them). Archiving stays reversible and never touches files.
- The toolbar "打开资源文件夹" action opens the shared `assets/` folder through `lib/tauri/inbox`; the view must not compose filesystem paths itself.
- Notes editor controls and status bar surfaces use the shared `manager-*` tokens so the dark theme does not inherit light-only `bg-white` surfaces.

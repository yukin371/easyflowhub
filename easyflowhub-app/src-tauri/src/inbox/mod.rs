//! Inbox domain facade.
//!
//! The Inbox stays the single owner of capture persistence inside the shared
//! `notes.db`. Internally it is split by responsibility:
//!
//! - `schema`: table creation and additive migrations
//! - `attachments`: attachment identity, staged file storage, transcripts
//! - `repository`: every `inbox_items` / `inbox_attachments` row access
//! - `service`: capture / conversion / deletion use cases and transactions
//! - `commands`: thin Tauri command wrappers delegating to service/repository
//!
//! External native owners (voice capture, transcription) reach Inbox data
//! through the re-exported controlled helpers below rather than reaching into
//! the submodules directly. Tauri command registration lives in `commands`
//! and is wired from `lib.rs` via `inbox::commands::*`.

mod attachments;
pub(crate) mod commands;
mod repository;
mod schema;
mod service;

// Voice capture delegates a whole capture through the same command path the
// frontend uses, so the command and its input type stay reachable here.
pub use commands::capture_inbox_item;
pub use service::CaptureInboxItemInput;

// Transcription reads / extends Inbox data (audio attachment lookup, transcript
// attachment store/read, connection and assets access) through these helpers.
pub(crate) use attachments::{
    find_audio_attachment_at, read_transcript_at, store_transcript_at, InboxTranscript,
};
pub(crate) use repository::{assets_dir, inbox_connection};
// Only sibling test code initializes the Inbox schema directly.
#[cfg(test)]
pub(crate) use schema::init_db;

/**
 * Note Types
 * Matches the SQLite schema in notes.rs
 */

export interface Note {
  id: string;
  title: string;
  content: string;
  tags: string;
  created_at: string;
  updated_at: string;
  is_pinned: boolean;
  deleted_at: string | null;
}

export interface Tab {
  id: string;
  noteId: string | null;
  title: string;
  isDirty: boolean;
}

// API response types
export interface ListNotesResponse {
  ok: boolean;
  count: number;
  notes: Note[];
}

export interface GetNoteResponse {
  ok: boolean;
  note: Note | null;
}

export interface SaveNoteResponse {
  ok: boolean;
  note: Note;
}

export interface DeleteNoteResponse {
  ok: boolean;
  deleted_id: string;
}

export interface SearchNotesResponse {
  ok: boolean;
  count: number;
  query: string;
  notes: Note[];
}

// Trash response types
export interface TrashNoteResponse {
  ok: boolean;
  trashed_id: string;
}

export interface RestoreNoteResponse {
  ok: boolean;
  restored_id: string;
}

export interface EmptyTrashResponse {
  ok: boolean;
  deleted_count: number;
}

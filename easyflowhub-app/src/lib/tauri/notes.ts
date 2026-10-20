/**
 * Notes IPC Wrapper
 * Provides typed frontend API for SQLite notes commands
 */

import { invoke } from '@tauri-apps/api/core';
import type {
  Note,
  ListNotesResponse,
  GetNoteResponse,
  SaveNoteResponse,
  DeleteNoteResponse,
  SearchNotesResponse,
  TrashNoteResponse,
  RestoreNoteResponse,
  EmptyTrashResponse,
} from '../../types/note';

// ============================================================================
// List Notes
// ============================================================================

export async function listNotes(): Promise<Note[]> {
  const response: ListNotesResponse = await invoke('list_notes');
  return response.notes;
}

export async function listNotesWithMeta(): Promise<ListNotesResponse> {
  return await invoke('list_notes');
}

// ============================================================================
// Get Note
// ============================================================================

export async function getNote(id: string): Promise<Note | null> {
  const response: GetNoteResponse = await invoke('get_note', { id });
  return response.note;
}

// ============================================================================
// Save Note
// ============================================================================

/**
 * Optional compare-and-swap guard (F05): when set, the native update only
 * applies if the stored `updated_at` still matches; otherwise the save is
 * rejected with a `note_conflict:` error instead of overwriting a newer write
 * from another window. Omit it for last-write-wins saves.
 */
export interface SaveNoteInput extends Partial<Note> {
  id: string;
  expected_updated_at?: string;
}

export async function saveNote(note: SaveNoteInput): Promise<Note> {
  const response: SaveNoteResponse = await invoke('save_note', { note });
  return response.note;
}

// ============================================================================
// Create Note
// ============================================================================

export async function createNote(): Promise<Note> {
  const response: SaveNoteResponse = await invoke('create_note');
  return response.note;
}

/**
 * Return the active todo inbox, creating it atomically when it does not exist.
 * The native command serializes the check-and-insert across windows.
 */
export async function ensureTodoInboxNote(): Promise<Note> {
  const response: SaveNoteResponse = await invoke('ensure_todo_inbox_note');
  return response.note;
}

// ============================================================================
// Delete Note (permanent)
// ============================================================================

export async function deleteNote(id: string): Promise<string> {
  const response: DeleteNoteResponse = await invoke('delete_note', { id });
  return response.deleted_id;
}

// ============================================================================
// Search Notes
// ============================================================================

export async function searchNotes(query: string): Promise<Note[]> {
  const response: SearchNotesResponse = await invoke('search_notes', { query });
  return response.notes;
}

// ============================================================================
// Toggle Pin
// ============================================================================

export async function togglePinNote(id: string): Promise<Note> {
  const response: SaveNoteResponse = await invoke('toggle_pin_note', { id });
  return response.note;
}

// ============================================================================
// Trash Commands
// ============================================================================

export async function trashNote(id: string): Promise<string> {
  const response: TrashNoteResponse = await invoke('trash_note', { id });
  return response.trashed_id;
}

export async function trashNotesBatch(ids: string[]): Promise<string> {
  const response: TrashNoteResponse = await invoke('trash_notes_batch', { ids });
  return response.trashed_id;
}

export async function listTrash(): Promise<Note[]> {
  const response: ListNotesResponse = await invoke('list_trash');
  return response.notes;
}

export async function restoreNote(id: string): Promise<string> {
  const response: RestoreNoteResponse = await invoke('restore_note', { id });
  return response.restored_id;
}

export async function restoreNotesBatch(ids: string[]): Promise<string> {
  const response: RestoreNoteResponse = await invoke('restore_notes_batch', { ids });
  return response.restored_id;
}

export async function permanentDeleteNote(id: string): Promise<string> {
  const response: DeleteNoteResponse = await invoke('permanent_delete_note', { id });
  return response.deleted_id;
}

export async function emptyTrash(): Promise<number> {
  const response: EmptyTrashResponse = await invoke('empty_trash');
  return response.deleted_count;
}

export async function closeAllNoteWindows(): Promise<void> {
  await invoke('close_all_note_windows');
}

export async function toggleNoteWindowsVisibility(): Promise<boolean> {
  return await invoke('toggle_note_windows_visibility');
}

// ============================================================================
// Todo Card Window
// ============================================================================

export async function createTodoCardWindow(): Promise<string> {
  return await invoke('create_todo_card_window');
}

// ============================================================================
// Save Image
// ============================================================================

export interface SaveImageResponse {
  ok: boolean;
  filename: string;
  path: string;
}

export async function saveImage(dataUrl: string): Promise<SaveImageResponse> {
  return await invoke('save_image', { dataUrl });
}

export async function saveImageFromPath(filePath: string): Promise<SaveImageResponse> {
  return await invoke('save_image_from_path', { filePath });
}

import { invoke } from '@tauri-apps/api/core';
import type {
  FileDeleteResult,
  FileEntry,
  FileOperationResponse,
  FilePreview,
  FileSearchRequest,
  FileSearchResponse,
  FileSearchStatus,
} from '../../types/fileSearch';

/** Read the native file index status and configured root. */
export function getFileSearchStatus(): Promise<FileSearchStatus> {
  return invoke<FileSearchStatus>('file_search_get_status');
}

/** Configure the single v0.2.0 index root. Pass null to clear it. */
export function setFileSearchRoot(root: string | null): Promise<FileSearchStatus> {
  return invoke<FileSearchStatus>('file_search_set_root', { root });
}

/** Pause or resume background indexing. */
export function setFileIndexPaused(paused: boolean): Promise<FileSearchStatus> {
  return invoke<FileSearchStatus>('file_search_set_paused', { paused });
}

/** Alias kept close to the native setting name for manager panels. */
export const setFileSearchPaused = setFileIndexPaused;

export function pauseFileIndexing(): Promise<FileSearchStatus> {
  return setFileIndexPaused(true);
}

export function resumeFileIndexing(): Promise<FileSearchStatus> {
  return setFileIndexPaused(false);
}

/** Rebuild the configured root index and return the resulting generation. */
export function refreshFileIndex(): Promise<FileSearchStatus> {
  return invoke<FileSearchStatus>('file_search_refresh_index');
}

/** Remove all indexed entries without changing the configured root. */
export function clearFileIndex(): Promise<FileSearchStatus> {
  return invoke<FileSearchStatus>('file_search_clear_index');
}

/** Search only the native index. The native side enforces the configured root. */
export function searchFiles(request: FileSearchRequest = { query: '' }): Promise<FileSearchResponse> {
  return invoke<FileSearchResponse>('file_search_search', { request });
}

export function previewFile(path: string, expectedGeneration: number): Promise<FilePreview> {
  return invoke<FilePreview>('file_search_preview', { path, expectedGeneration });
}

export function openFile(path: string, expectedGeneration: number): Promise<FileEntry> {
  return invoke<FileEntry>('file_search_open_path', { path, expectedGeneration });
}

export function revealFileInExplorer(path: string, expectedGeneration: number): Promise<string> {
  return invoke<string>('file_search_reveal_in_explorer', { path, expectedGeneration });
}

export const revealFile = revealFileInExplorer;

export function copyFileSearchPath(path: string, expectedGeneration: number): Promise<string> {
  return invoke<string>('file_search_copy_path', { path, expectedGeneration });
}

export const copyFilePath = copyFileSearchPath;

export function listRecentFiles(limit?: number): Promise<FileSearchResponse> {
  return invoke<FileSearchResponse>('file_search_recent', { limit: limit ?? null });
}

export function renameIndexedFile(path: string, newName: string, expectedGeneration: number): Promise<FileOperationResponse> {
  return invoke<FileOperationResponse>('file_search_rename_path', { path, newName, expectedGeneration });
}

export const renameFile = renameIndexedFile;

export function moveIndexedFile(path: string, destinationDir: string, expectedGeneration: number): Promise<FileOperationResponse> {
  return invoke<FileOperationResponse>('file_search_move_path', { path, destinationDir, expectedGeneration });
}

export const moveFile = moveIndexedFile;

export function copyIndexedFile(path: string, destinationDir: string, expectedGeneration: number): Promise<FileOperationResponse> {
  return invoke<FileOperationResponse>('file_search_copy_file', { path, destinationDir, expectedGeneration });
}

export const copyFile = copyIndexedFile;

/** Delete means move to the Windows recycle bin; native never permanently deletes here. */
export function deleteToRecycleBin(path: string, expectedGeneration: number): Promise<FileDeleteResult> {
  return invoke<FileDeleteResult>('file_search_delete_to_recycle_bin', { path, expectedGeneration });
}

export const deleteFile = deleteToRecycleBin;


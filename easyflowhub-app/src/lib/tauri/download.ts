import { invoke } from '@tauri-apps/api/core';
import type {
  DownloadConflictPolicy,
  DownloadEnqueueRequest,
  DownloadSettings,
  DownloadStatus,
} from '../../types/download';

/** Read the persisted download directory and conflict policy. */
export function getDownloadSettings(): Promise<DownloadSettings> {
  return invoke<DownloadSettings>('download_get_settings');
}

/** Persist download settings without touching existing queue entries. */
export function updateDownloadSettings(settings: DownloadSettings): Promise<DownloadSettings> {
  return invoke<DownloadSettings>('download_update_settings', { settings });
}

/** Add a URL to the native queue. The native side validates URL and target paths. */
export function enqueueDownload(request: DownloadEnqueueRequest): Promise<DownloadStatus> {
  return invoke<DownloadStatus>('download_enqueue', { ...request });
}

/** Poll queue status; native owns all worker state and transitions. */
export function listDownloads(): Promise<DownloadStatus[]> {
  return invoke<DownloadStatus[]>('download_list');
}

export function pauseDownload(id: string): Promise<DownloadStatus> {
  return invoke<DownloadStatus>('download_pause', { id });
}

export function resumeDownload(id: string): Promise<DownloadStatus> {
  return invoke<DownloadStatus>('download_resume', { id });
}

export function cancelDownload(id: string): Promise<DownloadStatus> {
  return invoke<DownloadStatus>('download_cancel', { id });
}

export function retryDownload(id: string): Promise<DownloadStatus> {
  return invoke<DownloadStatus>('download_retry', { id });
}

export function openDownload(id: string): Promise<void> {
  return invoke<void>('download_open', { id });
}

export function revealDownload(id: string): Promise<string> {
  return invoke<string>('download_reveal', { id });
}

export function copyDownloadPath(id: string): Promise<string> {
  return invoke<string>('download_copy_path', { id });
}

export type { DownloadConflictPolicy };

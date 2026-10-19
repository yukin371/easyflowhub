export type DownloadState =
  | 'queued'
  | 'downloading'
  | 'paused'
  | 'completed'
  | 'failed'
  | 'cancelled';

export type DownloadConflictPolicy = 'fail' | 'rename' | 'overwrite';

export interface DownloadStatus {
  id: string;
  url: string;
  fileName: string | null;
  targetDir: string | null;
  targetPath: string | null;
  status: DownloadState;
  downloadedBytes: number;
  totalBytes: number | null;
  error: string | null;
  retryCount: number;
  createdAt: string;
  updatedAt: string;
}

export interface DownloadSettings {
  targetDir: string | null;
  conflictPolicy: DownloadConflictPolicy;
}

export interface DownloadEnqueueRequest {
  url: string;
  fileName?: string;
  targetDir?: string;
  conflictPolicy?: DownloadConflictPolicy;
}

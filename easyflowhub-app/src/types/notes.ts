export interface BackupDraft {
  title: string;
  content: string;
  tags: string;
  timestamp?: number;
}

export type SaveState = 'saved' | 'saving' | 'unsaved' | 'error';

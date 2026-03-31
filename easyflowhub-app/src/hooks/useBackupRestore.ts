import { useCallback, useState } from 'react';
import type { Note } from '../types/note';
import type { BackupDraft } from '../components/manager/notes/types';
import { getBackupKey, normalizeTags } from '../components/manager/notes/utils';

interface UseBackupRestoreOptions {
  onApplyDraft: (draft: BackupDraft) => void;
  onMergeDraft: (draft: BackupDraft) => void;
}

export function useBackupRestore(options: UseBackupRestoreOptions) {
  const { onApplyDraft, onMergeDraft } = options;
  const [backupDraft, setBackupDraft] = useState<BackupDraft | null>(null);

  const inspectBackup = useCallback((note: Note) => {
    setBackupDraft(null);

    const backup = localStorage.getItem(getBackupKey(note.id));
    if (!backup) {
      return;
    }

    try {
      const parsed = JSON.parse(backup) as BackupDraft;
      const backupTime = parsed.timestamp ?? 0;
      const noteTime = new Date(note.updated_at).getTime();
      const hasChanges =
        (parsed.title ?? note.title) !== note.title ||
        (parsed.content ?? note.content) !== note.content ||
        normalizeTags(parsed.tags ?? note.tags) !== normalizeTags(note.tags);

      if (!hasChanges || backupTime < noteTime) {
        localStorage.removeItem(getBackupKey(note.id));
        return;
      }

      setBackupDraft({
        title: parsed.title ?? note.title,
        content: parsed.content ?? note.content,
        tags: parsed.tags ?? note.tags,
        timestamp: parsed.timestamp,
      });
    } catch (err) {
      console.error('Failed to restore backup:', err);
    }
  }, []);

  const clearBackupDraft = useCallback(() => {
    setBackupDraft(null);
  }, []);

  const restoreBackupDraft = useCallback(() => {
    if (!backupDraft) {
      return;
    }

    onApplyDraft({
      title: backupDraft.title,
      content: backupDraft.content,
      tags: normalizeTags(backupDraft.tags),
      timestamp: backupDraft.timestamp,
    });
    setBackupDraft(null);
  }, [backupDraft, onApplyDraft]);

  const mergeBackupDraft = useCallback(() => {
    if (!backupDraft) {
      return;
    }

    onMergeDraft(backupDraft);
    setBackupDraft(null);
  }, [backupDraft, onMergeDraft]);

  return {
    backupDraft,
    inspectBackup,
    clearBackupDraft,
    restoreBackupDraft,
    mergeBackupDraft,
    setBackupDraft,
  };
}

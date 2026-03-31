import { useCallback, useEffect, useRef, useState } from 'react';
import { saveNote } from '../lib/tauri/notes';
import type { Note } from '../types/note';
import type { SaveState } from '../components/manager/notes/types';
import { getBackupKey, normalizeTags } from '../components/manager/notes/utils';
import { deriveDisplayTitle, parseNoteContent } from '../lib/noteParser';

interface PersistParams {
  noteId: string;
  title: string;
  content: string;
  tags: string;
}

interface UseAutoSaveOptions {
  delayMs: number;
  onSaved: (note: Note) => void;
}

export function useAutoSave(options: UseAutoSaveOptions) {
  const { delayMs, onSaved } = options;
  const [saveState, setSaveState] = useState<SaveState>('saved');
  const [saveError, setSaveError] = useState('');
  const saveTimerRef = useRef<number | null>(null);

  const persistNow = useCallback(
    async ({ noteId, title, content, tags }: PersistParams) => {
      setSaveState('saving');
      setSaveError('');

      const parsed = parseNoteContent(content);
      const normalizedTags = normalizeTags([tags, parsed.tags].filter(Boolean).join(' '));
      const normalizedContent = parsed.cleanContent;

      const updated = await saveNote({
        id: noteId,
        title: deriveDisplayTitle(title, normalizedContent),
        content: normalizedContent,
        tags: normalizedTags,
      });

      localStorage.removeItem(getBackupKey(noteId));
      onSaved(updated);
      setSaveState('saved');
      return updated;
    },
    [onSaved]
  );

  const scheduleSave = useCallback(
    ({ noteId, title, content, tags }: PersistParams) => {
      if (saveTimerRef.current) {
        window.clearTimeout(saveTimerRef.current);
      }

      const normalizedTags = normalizeTags(tags);
      setSaveState('unsaved');
      setSaveError('');

      localStorage.setItem(
        getBackupKey(noteId),
        JSON.stringify({
          title,
          content,
          tags: normalizedTags,
          timestamp: Date.now(),
        })
      );

      saveTimerRef.current = window.setTimeout(async () => {
        try {
          await persistNow({ noteId, title, content, tags: normalizedTags });
        } catch (err) {
          console.error('Failed to save note:', err);
          setSaveState('error');
          setSaveError('本地服务连接失败');
        } finally {
          saveTimerRef.current = null;
        }
      }, delayMs);
    },
    [delayMs, persistNow]
  );

  const flushSave = useCallback(
    async (params: PersistParams | null) => {
      if (!params) {
        return null;
      }

      if (saveTimerRef.current === null) {
        return null;
      }

      window.clearTimeout(saveTimerRef.current);
      saveTimerRef.current = null;

      try {
        return await persistNow(params);
      } catch (err) {
        console.error('Failed to flush save:', err);
        setSaveState('error');
        setSaveError('本地服务连接失败');
        return null;
      }
    },
    [persistNow]
  );

  const resetSaveFeedback = useCallback(() => {
    setSaveState('saved');
    setSaveError('');
  }, []);

  useEffect(() => {
    return () => {
      if (saveTimerRef.current) {
        window.clearTimeout(saveTimerRef.current);
      }
    };
  }, []);

  return {
    saveState,
    saveError,
    scheduleSave,
    flushSave,
    resetSaveFeedback,
  };
}

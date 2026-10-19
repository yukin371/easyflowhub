import { useCallback, useEffect, useRef, useState } from 'react';
import { saveNote } from '../lib/tauri/notes';
import type { Note } from '../types/note';
import type { SaveState } from '../types/notes';
import { getBackupKey, normalizeTags } from '../lib/notesUtils';
import { normalizeStoredTitle, parseNoteContent } from '../lib/noteParser';

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

/**
 * Each call to `scheduleSave` mints a new token. Stale responses that resolve
 * after a newer session (timer or note switch) is started drop their result:
 * they neither overwrite the editor draft via `onSaved` nor clear the local
 * backup that now belongs to a different revision.
 *
 * In-flight saves are serialised through a single promise chain so the page
 * can `flushSave` deterministically, and so a slow save can't be eclipsed by a
 * later scheduleSave's response.
 */
export function useAutoSave(options: UseAutoSaveOptions) {
  const { delayMs, onSaved } = options;
  const [saveState, setSaveState] = useState<SaveState>('saved');
  const [saveError, setSaveError] = useState('');
  const saveTimerRef = useRef<number | null>(null);
  const tokenRef = useRef(0);
  const inflightRef = useRef<{ token: number; promise: Promise<Note | null> } | null>(null);
  // `onSaved` is captured into a ref so the chain doesn't restart on every
  // caller re-render (the page may pass a fresh closure on each render).
  const onSavedRef = useRef(onSaved);
  useEffect(() => {
    onSavedRef.current = onSaved;
  }, [onSaved]);

  const persistNow = useCallback(
    async ({ noteId, title, content, tags }: PersistParams, token: number) => {
      // Bail before mutating any UI state if this run has been superseded.
      if (tokenRef.current !== token) {
        return null;
      }
      setSaveState('saving');
      setSaveError('');

      try {
        const parsed = parseNoteContent(content);
        const normalizedTags = normalizeTags([tags, parsed.tags].filter(Boolean).join(' '));
        const normalizedContent = parsed.cleanContent;

        const updated = await saveNote({
          id: noteId,
          title: normalizeStoredTitle(title),
          content: normalizedContent,
          tags: normalizedTags,
        });

        // Stale responses must not clear the backup that now belongs to a
        // later session for the same note id, and must not call back into the
        // page with a draft that no longer matches the editor.
        if (tokenRef.current !== token) {
          return null;
        }

        try {
          localStorage.removeItem(getBackupKey(noteId));
        } catch (err) {
          console.warn('Failed to clear the local autosave backup:', err);
        }
        onSavedRef.current(updated);
        setSaveState('saved');
        return updated;
      } catch (err) {
        if (tokenRef.current === token) {
          console.error('Failed to save note:', err);
          setSaveState('error');
          setSaveError('本地服务连接失败');
        }
        throw err;
      }
    },
    []
  );

  const runPersist = useCallback(
    (params: PersistParams, token: number): Promise<Note | null> => {
      const previous = inflightRef.current?.promise ?? Promise.resolve(null);
      const promise = previous
        .catch(() => null)
        .then(() => {
          if (tokenRef.current !== token) {
            return null;
          }
          return persistNow(params, token);
        })
        .finally(() => {
          if (inflightRef.current?.token === token) {
            inflightRef.current = null;
          }
        });
      inflightRef.current = { token, promise };
      return promise;
    },
    [persistNow]
  );

  const scheduleSave = useCallback(
    ({ noteId, title, content, tags }: PersistParams) => {
      const token = ++tokenRef.current;

      if (saveTimerRef.current !== null) {
        window.clearTimeout(saveTimerRef.current);
        saveTimerRef.current = null;
      }

      const normalizedTags = normalizeTags(tags);
      setSaveState('unsaved');
      setSaveError('');

      // The backup is a best-effort safety net: localStorage can reject writes
      // (quota exceeded, private mode) and a thrown exception here would break
      // the editor's onChange handler.
      try {
        localStorage.setItem(
          getBackupKey(noteId),
          JSON.stringify({
            title,
            content,
            tags: normalizedTags,
            timestamp: Date.now(),
          })
        );
      } catch (err) {
        console.warn('Failed to persist the local autosave backup:', err);
      }

      const params: PersistParams = { noteId, title, content, tags: normalizedTags };
      saveTimerRef.current = window.setTimeout(() => {
        saveTimerRef.current = null;
        void runPersist(params, token);
      }, delayMs);
    },
    [delayMs, runPersist]
  );

  const flushSave = useCallback(
    async (params: PersistParams | null) => {
      if (!params) {
        return null;
      }

      const token = tokenRef.current;

      // Timer is scheduled but not yet fired: the user wants to close the
      // window now, so promote the queued save to the in-flight chain
      // instead of dropping it on the floor.
      if (saveTimerRef.current !== null) {
        window.clearTimeout(saveTimerRef.current);
        saveTimerRef.current = null;
        try {
          return await runPersist(params, token);
        } catch (err) {
          console.error('Failed to flush save:', err);
          return null;
        }
      }

      // No pending timer; if a save is currently in flight, surface its
      // result so callers can observe a real save attempt before closing.
      if (inflightRef.current) {
        try {
          return await inflightRef.current.promise;
        } catch (err) {
          console.error('Failed to await inflight save:', err);
          return null;
        }
      }

      return null;
    },
    [runPersist]
  );

  const resetSaveFeedback = useCallback(() => {
    setSaveState('saved');
    setSaveError('');
  }, []);

  const invalidateSaveSession = useCallback(() => {
    // A note switch must create a hard boundary for callbacks from the
    // previous editor session. The network request cannot be cancelled, but
    // its response will fail the token check in `persistNow`.
    tokenRef.current += 1;
    if (saveTimerRef.current !== null) {
      window.clearTimeout(saveTimerRef.current);
      saveTimerRef.current = null;
    }
  }, []);

  useEffect(() => {
    return () => {
      tokenRef.current += 1;
      if (saveTimerRef.current !== null) {
        window.clearTimeout(saveTimerRef.current);
        saveTimerRef.current = null;
      }
    };
  }, []);

  return {
    saveState,
    saveError,
    scheduleSave,
    flushSave,
    resetSaveFeedback,
    invalidateSaveSession,
  };
}

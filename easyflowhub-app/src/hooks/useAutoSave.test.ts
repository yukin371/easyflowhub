import { describe, expect, it, vi, beforeEach, afterEach } from 'vitest';
import { act, renderHook } from '@testing-library/react';
import { useAutoSave } from './useAutoSave';
import * as notesApi from '../lib/tauri/notes';
import type { Note } from '../types/note';

vi.mock('../lib/tauri/notes', () => ({
  saveNote: vi.fn(),
}));

const savedNote = { id: 'n1', title: 'T', content: 'C', tags: [] } as unknown as Note;

const mountedViews: Array<{ unmount: () => void }> = [];

type AutoSaveApi = ReturnType<typeof useAutoSave>;
interface AutoSaveHarness {
  result: { current: AutoSaveApi };
  unmount: () => void;
  onSaved: ReturnType<typeof vi.fn>;
}

async function renderAutoSave(delayMs = 0): Promise<AutoSaveHarness> {
  const onSaved = vi.fn();
  const view = renderHook(() => useAutoSave({ delayMs, onSaved }));
  mountedViews.push(view);
  return { result: view.result as { current: AutoSaveApi }, unmount: view.unmount, onSaved };
}

describe('useAutoSave', () => {
  beforeEach(() => {
    vi.mocked(notesApi.saveNote).mockReset();
    vi.mocked(notesApi.saveNote).mockResolvedValue(savedNote);
    localStorage.clear();
  });

  afterEach(() => {
    while (mountedViews.length > 0) {
      const view = mountedViews.pop();
      try {
        view?.unmount();
      } catch {
        // ignore unmount errors during cleanup
      }
    }
    vi.restoreAllMocks();
  });

  it('reaches saved state and clears the backup after a successful save', async () => {
    vi.useFakeTimers();
    try {
      const { result, onSaved } = await renderAutoSave();
      const setItem = vi.spyOn(Storage.prototype, 'setItem');
      const removeItem = vi.spyOn(Storage.prototype, 'removeItem');

      await act(async () => {
        result.current.scheduleSave({ noteId: 'n1', title: 'T', content: 'C', tags: 'a' });
      });
      expect(result.current.saveState).toBe('unsaved');
      expect(setItem).toHaveBeenCalledTimes(1);

      await act(async () => {
        await vi.runAllTimersAsync();
      });
      expect(result.current.saveState).toBe('saved');
      expect(onSaved).toHaveBeenCalledWith(savedNote);
      expect(removeItem).toHaveBeenCalled();
    } finally {
      vi.useRealTimers();
    }
  });

  it('keeps the editor alive when the backup write throws (quota / private mode)', async () => {
    vi.spyOn(Storage.prototype, 'setItem').mockImplementation(() => {
      throw new Error('QuotaExceededError');
    });

    const { result } = await renderAutoSave(10_000);
    await act(async () => {
      expect(() =>
        result.current.scheduleSave({ noteId: 'n1', title: 'T', content: 'C', tags: '' })
      ).not.toThrow();
    });
    // The schedule must still register the pending save so a flush can persist.
    await act(async () => {
      await result.current.flushSave({ noteId: 'n1', title: 'T', content: 'C', tags: '' });
    });
    expect(notesApi.saveNote).toHaveBeenCalled();
    expect(result.current.saveState).toBe('saved');
  });

  it('surfaces error state when the note save itself fails', async () => {
    vi.mocked(notesApi.saveNote).mockReset();
    vi.mocked(notesApi.saveNote).mockRejectedValue(new Error('offline'));
    const { result } = await renderAutoSave(10_000);
    await act(async () => {
      result.current.scheduleSave({ noteId: 'n1', title: 'T', content: 'C', tags: '' });
    });
    await act(async () => {
      await result.current.flushSave({ noteId: 'n1', title: 'T', content: 'C', tags: '' });
    });
    expect(result.current.saveState).toBe('error');
    expect(result.current.saveError).toBe('本地服务连接失败');
  });

  it('stale responses do not clobber a newer session for the same note', async () => {
    // F04: the slow A response must not call onSaved with A's body, must not
    // clear B's localStorage backup, and must not mark the page as 'saved'
    // when B is still pending.
    vi.useFakeTimers();
    try {
      let resolveSlow: ((value: Note) => void) | null = null;
      vi.mocked(notesApi.saveNote).mockReset();
      vi.mocked(notesApi.saveNote)
        .mockImplementationOnce(
          () =>
            new Promise<Note>((resolve) => {
              resolveSlow = resolve;
            })
        )
        .mockResolvedValueOnce({ ...savedNote, id: 'n1', content: 'B' });

      const { result, onSaved } = await renderAutoSave();
      const removeItem = vi.spyOn(Storage.prototype, 'removeItem');

      // Schedule A and let the debounce fire so the persist is in-flight
      // (still awaiting the slow saveNote).
      await act(async () => {
        result.current.scheduleSave({ noteId: 'n1', title: 'A', content: 'A-body', tags: '' });
      });
      await act(async () => {
        await vi.runAllTimersAsync();
      });
      expect(notesApi.saveNote).toHaveBeenCalledTimes(1);

      // User edits again while A is still awaiting: a new backup for B.
      await act(async () => {
        result.current.scheduleSave({ noteId: 'n1', title: 'B', content: 'B-body', tags: '' });
      });
      expect(localStorage.getItem('easyflowhub_note_autosave_n1')).toContain('B-body');

      // A's slow response finally resolves. The persist chain runs as a
      // microtask; B's debounce timer is faked and hasn't fired.
      await act(async () => {
        resolveSlow?.(savedNote);
        await Promise.resolve();
        await Promise.resolve();
      });

      // A's stale response must not have fired onSaved or cleared B's backup.
      expect(onSaved).not.toHaveBeenCalled();
      expect(removeItem).not.toHaveBeenCalled();
      expect(localStorage.getItem('easyflowhub_note_autosave_n1')).toContain('B-body');
      // The page should still show 'unsaved' because B is still pending.
      expect(result.current.saveState).toBe('unsaved');
    } finally {
      vi.useRealTimers();
    }
  });

  it('flushSave awaits an in-flight save instead of returning null', async () => {
    // F04: while a save is already running, a second flushSave must observe
    // the result of that save rather than the previous behaviour of bailing.
    vi.useFakeTimers();
    try {
      let resolveSlow: ((value: Note) => void) | null = null;
      vi.mocked(notesApi.saveNote).mockReset();
      vi.mocked(notesApi.saveNote).mockImplementationOnce(
        () =>
          new Promise<Note>((resolve) => {
            resolveSlow = resolve;
          })
      );

      const { result, onSaved } = await renderAutoSave();
      await act(async () => {
        result.current.scheduleSave({ noteId: 'n1', title: 'T', content: 'C', tags: '' });
      });
      // Let the debounce fire so the persist is in-flight (awaiting slow saveNote).
      await act(async () => {
        await vi.runAllTimersAsync();
      });
      expect(notesApi.saveNote).toHaveBeenCalledTimes(1);

      // The second flushSave must observe the in-flight save's result, not
      // bail out. It should be pending until resolveSlow fires.
      let flushed: Note | null | 'pending' = 'pending';
      await act(async () => {
        const flushPromise = (async () => {
          flushed = await result.current.flushSave({ noteId: 'n1', title: 'T', content: 'C', tags: '' });
        })();
        // Yield once so flushSave has a chance to either return or begin awaiting.
        await Promise.resolve();
        expect(flushed).toBe('pending');
        resolveSlow?.(savedNote);
        await flushPromise;
      });
      expect(flushed).toEqual(savedNote);
      expect(onSaved).toHaveBeenCalledWith(savedNote);
    } finally {
      vi.useRealTimers();
    }
  });

  it('invalidating a session drops a late response from the previous note', async () => {
    vi.useFakeTimers();
    try {
      let resolveSlow: ((value: Note) => void) | null = null;
      vi.mocked(notesApi.saveNote).mockReset();
      vi.mocked(notesApi.saveNote).mockImplementationOnce(
        () =>
          new Promise<Note>((resolve) => {
            resolveSlow = resolve;
          })
      );

      const { result, onSaved } = await renderAutoSave();
      await act(async () => {
        result.current.scheduleSave({ noteId: 'n1', title: 'A', content: 'A', tags: '' });
        await vi.runAllTimersAsync();
      });
      expect(notesApi.saveNote).toHaveBeenCalledTimes(1);

      await act(async () => {
        result.current.invalidateSaveSession();
        resolveSlow?.(savedNote);
        await Promise.resolve();
        await Promise.resolve();
      });

      expect(onSaved).not.toHaveBeenCalled();
      expect(localStorage.getItem('easyflowhub_note_autosave_n1')).toContain('A');
    } finally {
      vi.useRealTimers();
    }
  });

  it('scheduleSave cancels a previously scheduled but unfired timer', async () => {
    // F04: rapid edits should not enqueue multiple persists; only the latest
    // scheduleSave's content should reach saveNote.
    vi.useFakeTimers();
    try {
      vi.mocked(notesApi.saveNote).mockReset();
      vi.mocked(notesApi.saveNote).mockResolvedValue(savedNote);
      const { result } = await renderAutoSave();

      await act(async () => {
        result.current.scheduleSave({ noteId: 'n1', title: 'first', content: '1', tags: '' });
        result.current.scheduleSave({ noteId: 'n1', title: 'second', content: '2', tags: '' });
        result.current.scheduleSave({ noteId: 'n1', title: 'third', content: '3', tags: '' });
      });

      await act(async () => {
        await result.current.flushSave({ noteId: 'n1', title: 'third', content: '3', tags: '' });
      });
      expect(notesApi.saveNote).toHaveBeenCalledTimes(1);
      expect(notesApi.saveNote).toHaveBeenLastCalledWith(
        expect.objectContaining({ content: '3', title: 'third' })
      );
    } finally {
      vi.useRealTimers();
    }
  });
});

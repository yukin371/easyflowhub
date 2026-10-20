// useNotesArchive - 笔记归档工作区的列表 + 编辑会话 owner（Phase C 拆分）。
//
// 从 components/manager/NotesPanel 的 NotesArchivePanel 提取：笔记列表加载、
// 打开 / 关闭编辑会话（带外部导航竞态保护）、草稿编辑与乐观列表更新、自动
// 保存与备份恢复接线、归档级 CRUD（新建 / 回收站 / 置顶 / 重命名）。
//
// 不变量（架构审查第 5 节）：
// - 保存版本与 flush 只归 useAutoSave（F04）；切换笔记时先 flush 再
//   invalidateSaveSession，旧会话的迟到响应不得回填新草稿；
// - SQLite CRUD 只经 lib/tauri/notes typed bridge；
// - 面板只保留视图状态（搜索 / 标签筛选 / 视图模式 / 多选）与窗口交互。

import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import {
  createNote as createNoteApi,
  getNote,
  listNotes,
  saveNote,
  togglePinNote,
  trashNote,
  trashNotesBatch,
} from '../lib/tauri/notes';
import { splitTags } from '../lib/notesUtils';
import { useAutoSave } from './useAutoSave';
import { useBackupRestore } from './useBackupRestore';
import { useHistory } from './useHistory';
import { applyTextareaSelectionUpdate } from '../lib/editorSelection';
import type { Note } from '../types/note';
import type { SaveState } from '../types/notes';

const AUTOSAVE_DELAY = 900;

interface UseNotesArchiveOptions {
  /** 撤销历史步数，来自编辑器设置（undo_steps）。 */
  maxUndoSteps: number;
}

export function useNotesArchive(options: UseNotesArchiveOptions) {
  const { maxUndoSteps } = options;
  const [notes, setNotes] = useState<Note[]>([]);
  const [isLoading, setIsLoading] = useState(true);
  const [editingNoteId, setEditingNoteId] = useState<string | null>(null);
  const [draftTitle, setDraftTitle] = useState('');
  const [draftTags, setDraftTags] = useState('');

  const textareaRef = useRef<HTMLTextAreaElement | null>(null);
  const openNoteRequestRef = useRef(0);
  const editingNoteIdRef = useRef<string | null>(null);
  editingNoteIdRef.current = editingNoteId;

  const {
    value: draftContent,
    setValue: setDraftContent,
    undo: undoDraftContent,
    redo: redoDraftContent,
    canUndo: canUndoDraftContent,
    canRedo: canRedoDraftContent,
    clear: clearDraftHistory,
    flush: flushDraftHistory,
  } = useHistory('', { maxSteps: maxUndoSteps, debounceMs: 400 });

  const loadNotes = useCallback(async () => {
    setIsLoading(true);
    try {
      const loadedNotes = await listNotes();
      setNotes(loadedNotes);
    } catch (err) {
      console.error('Failed to load notes:', err);
    } finally {
      setIsLoading(false);
    }
  }, []);

  useEffect(() => {
    loadNotes();
  }, [loadNotes]);

  const {
    saveState,
    saveError,
    scheduleSave,
    flushSave,
    resetSaveFeedback,
    invalidateSaveSession,
  } = useAutoSave({
    delayMs: AUTOSAVE_DELAY,
    onSaved: (updated) => {
      setNotes((prev) => prev.map((note) => (note.id === updated.id ? updated : note)));
      if (editingNoteIdRef.current !== updated.id) {
        return;
      }
      setDraftTitle(updated.title);
      setDraftContent(updated.content, true);
      setDraftTags(updated.tags);
    },
  });

  const currentPersistParams = editingNoteId
    ? {
        noteId: editingNoteId,
        title: draftTitle,
        content: draftContent,
        tags: draftTags,
      }
    : null;

  const {
    backupDraft,
    inspectBackup,
    clearBackupDraft,
    restoreBackupDraft,
    mergeBackupDraft,
  } = useBackupRestore({
    onApplyDraft: (draft) => {
      setDraftTitle(draft.title);
      setDraftContent(draft.content, true);
      setDraftTags(draft.tags);
      if (editingNoteId) {
        scheduleSave({
          noteId: editingNoteId,
          title: draft.title,
          content: draft.content,
          tags: draft.tags,
        });
      }
    },
    onMergeDraft: (draft) => {
      const mergedContent = [draftContent, '', '--- 备份内容 ---', draft.content]
        .filter(Boolean)
        .join('\n');
      const mergedTags = Array.from(
        new Set([...splitTags(draftTags), ...splitTags(draft.tags)])
      ).join(' ');

      setDraftContent(mergedContent);
      setDraftTags(mergedTags);
      if (editingNoteId) {
        scheduleSave({
          noteId: editingNoteId,
          title: draftTitle,
          content: mergedContent,
          tags: mergedTags,
        });
      }
    },
  });

  const editingNote = useMemo(
    () => notes.find((note) => note.id === editingNoteId) ?? null,
    [editingNoteId, notes]
  );

  const openNote = useCallback(async (note: Note, requestId = ++openNoteRequestRef.current) => {
    if (requestId !== openNoteRequestRef.current) {
      return;
    }

    // Navigation can arrive from another window while the current note still
    // has a debounced or in-flight save. Persist that draft before changing
    // the editor identity, then invalidate late responses from the old
    // session so they cannot repopulate the new note's draft.
    if (editingNoteId && editingNoteId !== note.id) {
      await flushSave(currentPersistParams);
      if (requestId !== openNoteRequestRef.current) {
        return;
      }
      invalidateSaveSession();
    }

    if (requestId !== openNoteRequestRef.current) {
      return;
    }

    setEditingNoteId(note.id);
    setDraftTitle(note.title);
    setDraftContent(note.content, true);
    setDraftTags(note.tags);
    clearDraftHistory();
    resetSaveFeedback();
    inspectBackup(note);
  }, [clearDraftHistory, currentPersistParams, editingNoteId, flushSave, inspectBackup, invalidateSaveSession, resetSaveFeedback, setDraftContent]);

  const openNoteById = useCallback(async (noteId: string) => {
    const requestId = ++openNoteRequestRef.current;
    const existing = notes.find((note) => note.id === noteId);
    if (existing) {
      await openNote(existing, requestId);
      return;
    }

    try {
      const note = await getNote(noteId);
      if (!note) {
        return;
      }

      if (requestId !== openNoteRequestRef.current) {
        return;
      }

      setNotes((prev) => (prev.some((item) => item.id === note.id) ? prev : [note, ...prev]));
      await openNote(note, requestId);
    } catch (error) {
      console.error('Failed to open note by id:', error);
    }
  }, [notes, openNote]);

  const closeEditor = useCallback(async () => {
    const requestId = ++openNoteRequestRef.current;
    flushDraftHistory();
    await flushSave(currentPersistParams);
    if (requestId !== openNoteRequestRef.current) {
      return;
    }
    invalidateSaveSession();
    setEditingNoteId(null);
    clearDraftHistory();
    resetSaveFeedback();
    clearBackupDraft();
  }, [clearBackupDraft, clearDraftHistory, currentPersistParams, flushDraftHistory, flushSave, invalidateSaveSession, resetSaveFeedback]);

  const handleDraftChange = useCallback((field: 'title' | 'content' | 'tags', value: string) => {
    if (!editingNoteId) {
      return;
    }

    if (field === 'title') {
      setDraftTitle(value);
      setNotes((prev) =>
        prev.map((note) =>
          note.id === editingNoteId
            ? {
                ...note,
                title: value,
                updated_at: new Date().toISOString(),
              }
            : note
        )
      );
      scheduleSave({ noteId: editingNoteId, title: value, content: draftContent, tags: draftTags });
      return;
    }

    if (field === 'content') {
      setDraftContent(value);
      setNotes((prev) =>
        prev.map((note) =>
          note.id === editingNoteId
            ? {
                ...note,
                title: draftTitle,
                content: value,
                updated_at: new Date().toISOString(),
              }
            : note
        )
      );
      scheduleSave({ noteId: editingNoteId, title: draftTitle, content: value, tags: draftTags });
      return;
    }

    const normalized = Array.from(new Set(splitTags(value))).join(' ');
    setDraftTags(normalized);
    setNotes((prev) =>
      prev.map((note) =>
        note.id === editingNoteId ? { ...note, tags: normalized, updated_at: new Date().toISOString() } : note
        )
    );
    scheduleSave({
      noteId: editingNoteId,
      title: draftTitle,
      content: draftContent,
      tags: normalized,
    });
  }, [draftContent, draftTags, draftTitle, editingNoteId, scheduleSave, setDraftContent]);

  const applyShortcutContentChange = useCallback(
    (update: { value: string; selectionStart: number; selectionEnd?: number }) => {
      if (!editingNoteId) {
        return;
      }

      const nextValue = update.value;
      setDraftContent(nextValue);
      setNotes((prev) =>
        prev.map((note) =>
          note.id === editingNoteId
            ? { ...note, content: nextValue, updated_at: new Date().toISOString() }
            : note
        )
      );
      scheduleSave({
        noteId: editingNoteId,
        title: draftTitle,
        content: nextValue,
        tags: draftTags,
      });

      applyTextareaSelectionUpdate(textareaRef, update, setDraftContent);
    },
    [draftTags, draftTitle, editingNoteId, scheduleSave, setDraftContent]
  );

  const createAndOpenNote = useCallback(async () => {
    try {
      const note = await createNoteApi();
      setNotes((prev) => [note, ...prev]);
      await openNote(note);
    } catch (err) {
      console.error('Failed to create note:', err);
    }
  }, [openNote]);

  const trashNoteById = useCallback(async (noteId: string) => {
    try {
      await trashNote(noteId);
      setNotes((prev) => prev.filter((note) => note.id !== noteId));
      if (editingNoteId === noteId) {
        setEditingNoteId(null);
      }
    } catch (err) {
      console.error('Failed to trash note:', err);
    }
  }, [editingNoteId]);

  const trashNotesByIds = useCallback(async (noteIds: string[]) => {
    if (noteIds.length === 0) {
      return;
    }

    try {
      await trashNotesBatch(noteIds);
      const idSet = new Set(noteIds);
      setNotes((prev) => prev.filter((note) => !idSet.has(note.id)));
      if (editingNoteId && idSet.has(editingNoteId)) {
        setEditingNoteId(null);
      }
    } catch (err) {
      console.error('Failed to delete selected notes:', err);
    }
  }, [editingNoteId]);

  const togglePin = useCallback(async (noteId: string): Promise<Note | null> => {
    try {
      const updated = await togglePinNote(noteId);
      setNotes((prev) => prev.map((note) => (note.id === noteId ? updated : note)));
      if (editingNoteId === noteId) {
        setDraftTitle(updated.title);
        setDraftContent(updated.content, true);
        setDraftTags(updated.tags);
      }
      return updated;
    } catch (err) {
      console.error('Failed to toggle pin note:', err);
      return null;
    }
  }, [editingNoteId, setDraftContent]);

  const renameNote = useCallback(async (noteId: string, nextTitle: string) => {
    const normalizedTitle = nextTitle.trim();
    const currentNote = notes.find((item) => item.id === noteId);
    if (!currentNote || currentNote.title.trim() === normalizedTitle) {
      return;
    }

    try {
      const updated = await saveNote({ id: noteId, title: normalizedTitle });
      setNotes((prev) => prev.map((note) => (note.id === noteId ? updated : note)));
      if (editingNoteId === noteId) {
        setDraftTitle(updated.title);
      }
    } catch (error) {
      console.error('Failed to rename note:', error);
    }
  }, [editingNoteId, notes]);

  return {
    notes,
    isLoading,
    loadNotes,
    editingNoteId,
    isEditing: editingNoteId !== null,
    editingNote,
    draftTitle,
    draftContent,
    draftTags,
    saveState: saveState as SaveState,
    saveError,
    persistParams: currentPersistParams,
    flushSave,
    textareaRef,
    backupDraft,
    clearBackupDraft,
    restoreBackupDraft,
    mergeBackupDraft,
    openNote,
    openNoteById,
    closeEditor,
    handleDraftChange,
    applyShortcutContentChange,
    createAndOpenNote,
    trashNoteById,
    trashNotesByIds,
    togglePin,
    renameNote,
    undoDraftContent,
    redoDraftContent,
    canUndoDraftContent,
    canRedoDraftContent,
  };
}

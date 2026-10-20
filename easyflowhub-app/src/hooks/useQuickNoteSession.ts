// useQuickNoteSession - 快速笔记浮窗的编辑会话 owner（Phase C 拆分）。
//
// 从 QuickNotePage 提取：当前笔记身份、最近列表、草稿历史、自动保存接线、
// localStorage 备份恢复接线、附件图片列表与切换器的读写流程。页面只保留
// 窗口特有交互（透明度 / 置顶 / 关闭 / 快捷键 / 渲染）。
//
// 不变量（架构审查第 5 节）：
// - 保存版本与 flush 只归 useAutoSave（F04），本 hook 不复制其逻辑；
// - SQLite CRUD 只经 lib/tauri/notes typed bridge；
// - 备份恢复复用 useBackupRestore，图片插入复用 useEditorImageInsertion。

import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { createNote, getNote, listNotes } from '../lib/tauri/notes';
import { formatNoteForDisplay, parseNoteContent } from '../lib/noteParser';
import { buildPersistParams } from '../lib/notePersistence';
import {
  appendAttachedImageAssets,
  extractAttachedImageAssets,
  resolveAssetFilenameToUrl,
  type AttachedImageAsset,
} from '../lib/imageAssets';
import { applyTextareaSelectionUpdate } from '../lib/editorSelection';
import { dedupeNotesById, isNoteContinueCandidate } from '../lib/notesUtils';
import { useAutoSave } from './useAutoSave';
import { useBackupRestore } from './useBackupRestore';
import { useHistory } from './useHistory';
import { useEditorImageInsertion } from './useEditorImageInsertion';
import type { Note } from '../types/note';

const AUTOSAVE_DELAY = 900;
const RECENT_NOTES_LIMIT = 12;

interface UseQuickNoteSessionOptions {
  /** 撤销历史步数，来自编辑器设置（undo_steps）。 */
  maxUndoSteps: number;
}

export function useQuickNoteSession(options: UseQuickNoteSessionOptions) {
  const { maxUndoSteps } = options;

  const [note, setNote] = useState<Note | null>(null);
  const [recentNotes, setRecentNotes] = useState<Note[]>([]);
  const [attachedImages, setAttachedImages] = useState<AttachedImageAsset[]>([]);
  const [resolvedImageLinks, setResolvedImageLinks] = useState<Array<{ src: string; alt: string }>>(
    []
  );

  const textareaRef = useRef<HTMLTextAreaElement | null>(null);

  const {
    value: displayContent,
    setValue: setDisplayContent,
    undo,
    redo,
    canUndo,
    canRedo,
    clear: clearHistory,
    flush: flushHistory,
  } = useHistory('', { maxSteps: maxUndoSteps, debounceMs: 400 });

  const { flushSave, scheduleSave, resetSaveFeedback } = useAutoSave({
    delayMs: AUTOSAVE_DELAY,
    onSaved: (updated) => {
      setNote(updated);
      setRecentNotes((prev) => {
        const next = prev.some((item) => item.id === updated.id)
          ? prev.map((item) => (item.id === updated.id ? updated : item))
          : [updated, ...prev];
        return dedupeNotesById(next).slice(0, RECENT_NOTES_LIMIT);
      });
    },
  });

  const mergedEditorContent = useMemo(
    () => appendAttachedImageAssets(displayContent, attachedImages),
    [attachedImages, displayContent]
  );

  // 快捷便签页同样会写入 localStorage 备份，需要提供恢复入口：
  // 否则进程被杀后备份只能去管理页编辑器里找回。
  const {
    backupDraft,
    inspectBackup,
    clearBackupDraft,
    restoreBackupDraft,
    mergeBackupDraft,
  } = useBackupRestore({
    onApplyDraft: (draft) => {
      const parsed = extractAttachedImageAssets(draft.content);
      setDisplayContent(parsed.textContent, true);
      setAttachedImages(parsed.images);
      if (note?.id) {
        scheduleSave({
          noteId: note.id,
          title: draft.title,
          content: draft.content,
          tags: draft.tags,
        });
      }
    },
    onMergeDraft: (draft) => {
      const mergedContent = [mergedEditorContent, '', '--- 备份内容 ---', draft.content]
        .filter(Boolean)
        .join('\n');
      const parsed = extractAttachedImageAssets(mergedContent);
      setDisplayContent(parsed.textContent, true);
      // 合并时当前内容与备份可能引用同一批图片，按文件名去重
      const uniqueImages = Array.from(
        new Map(parsed.images.map((image) => [image.filename, image])).values()
      );
      setAttachedImages(uniqueImages);
      if (note?.id) {
        scheduleSave({
          noteId: note.id,
          title: draft.title,
          content: mergedContent,
          tags: draft.tags,
        });
      }
    },
  });

  const currentDraftTags = useMemo(
    () => parseNoteContent(mergedEditorContent).tags,
    [mergedEditorContent]
  );

  const currentPersistParams = useMemo(() => {
    return buildPersistParams(note?.id ?? null, mergedEditorContent, note?.title ?? '');
  }, [mergedEditorContent, note]);

  const refreshRecentNotes = useCallback(async (preferredId?: string | null) => {
    try {
      const loadedNotes = await listNotes();
      const sorted = [...loadedNotes].sort((a, b) => {
        if (a.id === preferredId) {
          return -1;
        }
        if (b.id === preferredId) {
          return 1;
        }
        const aContinue = isNoteContinueCandidate(a);
        const bContinue = isNoteContinueCandidate(b);
        if (aContinue !== bContinue) {
          return aContinue ? -1 : 1;
        }
        return new Date(b.updated_at).getTime() - new Date(a.updated_at).getTime();
      });
      setRecentNotes(dedupeNotesById(sorted).slice(0, RECENT_NOTES_LIMIT));
    } catch (error) {
      console.error('Failed to refresh note list:', error);
    }
  }, []);

  const removeAttachedImage = useCallback((indexToRemove: number) => {
    setAttachedImages((prev) => prev.filter((_, index) => index !== indexToRemove));
  }, []);

  // 初始化笔记：URL 带 note_id 则加载，否则新建。
  useEffect(() => {
    const initialize = async () => {
      try {
        await refreshRecentNotes();

        // 从 URL 获取 note_id
        const urlParams = new URLSearchParams(window.location.search);
        const noteId = urlParams.get('note_id');

        if (noteId) {
          const existingNote = await getNote(noteId);
          if (existingNote) {
            const parsed = extractAttachedImageAssets(formatNoteForDisplay(existingNote));
            setNote(existingNote);
            setDisplayContent(parsed.textContent, true); // skipHistory
            setAttachedImages(parsed.images);
            resetSaveFeedback();
            inspectBackup(existingNote);
          } else {
            const newNote = await createNote();
            setNote(newNote);
            setDisplayContent('', true);
            setAttachedImages([]);
          }
        } else {
          const newNote = await createNote();
          setNote(newNote);
          setDisplayContent('', true);
          setAttachedImages([]);
        }
      } catch (error) {
        console.error('Failed to initialize note:', error);
      }
    };

    initialize();
  }, [refreshRecentNotes]);

  useEffect(() => {
    if (!currentPersistParams) {
      return;
    }

    scheduleSave(currentPersistParams);
  }, [currentPersistParams, scheduleSave]);

  const applyShortcutContentChange = useCallback(
    (update: { value: string; selectionStart: number; selectionEnd?: number }) => {
      applyTextareaSelectionUpdate(textareaRef, update, setDisplayContent);
    },
    [setDisplayContent]
  );

  const imageInsertion = useEditorImageInsertion({
    value: displayContent,
    textareaRef,
    onChange: applyShortcutContentChange,
    nativeWindowDropEnabled: true,
    onImageSaved: ({ altText, filename }) => {
      setAttachedImages((prev) => [...prev, { alt: altText, filename }]);
    },
  });

  const handleSwitchNote = useCallback(
    async (nextNoteId: string) => {
      if (!nextNoteId || nextNoteId === note?.id) {
        return;
      }

      try {
        flushHistory(); // 刷新历史记录
        await flushSave(currentPersistParams);
        const nextNote = await getNote(nextNoteId);
        if (!nextNote) {
          return;
        }

        const parsed = extractAttachedImageAssets(formatNoteForDisplay(nextNote));
        setNote(nextNote);
        setDisplayContent(parsed.textContent, true); // skipHistory
        setAttachedImages(parsed.images);
        clearHistory(); // 清空历史
        resetSaveFeedback();
        inspectBackup(nextNote);
        await refreshRecentNotes(nextNote.id);
      } catch (error) {
        console.error('Failed to switch note:', error);
      }
    },
    [currentPersistParams, flushSave, note?.id, refreshRecentNotes, resetSaveFeedback, setDisplayContent, clearHistory, flushHistory, inspectBackup]
  );

  const handleCreateAndSwitch = useCallback(async () => {
    try {
      flushHistory(); // 刷新历史记录
      await flushSave(currentPersistParams);
      const nextNote = await createNote();
      setNote(nextNote);
      setDisplayContent('', true); // skipHistory
      setAttachedImages([]);
      clearHistory(); // 清空历史
      resetSaveFeedback();
      await refreshRecentNotes(nextNote.id);
    } catch (error) {
      console.error('Failed to create and switch note:', error);
    }
  }, [currentPersistParams, flushSave, refreshRecentNotes, resetSaveFeedback, setDisplayContent, clearHistory, flushHistory]);

  useEffect(() => {
    let cancelled = false;

    const resolveImages = async () => {
      const next = await Promise.all(
        attachedImages.map(async (image) => ({
          alt: image.alt,
          src: await resolveAssetFilenameToUrl(image.filename),
        }))
      );

      if (!cancelled) {
        setResolvedImageLinks(next);
      }
    };

    void resolveImages();
    return () => {
      cancelled = true;
    };
  }, [attachedImages]);

  /** 只把已排队的保存立即落盘（不冲历史），供快捷键保存使用。 */
  const flushSaveNow = useCallback(async () => {
    await flushSave(currentPersistParams);
  }, [currentPersistParams, flushSave]);

  /** 关闭 / 切换前的完整 flush：先把防抖历史提交，再落盘保存。 */
  const flushPendingSave = useCallback(async () => {
    flushHistory();
    await flushSave(currentPersistParams);
  }, [currentPersistParams, flushSave, flushHistory]);

  return {
    note,
    recentNotes,
    displayContent,
    setDisplayContent,
    undo,
    redo,
    canUndo,
    canRedo,
    textareaRef,
    attachedImages,
    resolvedImageLinks,
    removeAttachedImage,
    mergedEditorContent,
    currentDraftTags,
    persistParams: currentPersistParams,
    backupDraft,
    clearBackupDraft,
    restoreBackupDraft,
    mergeBackupDraft,
    imageInsertion,
    applyShortcutContentChange,
    switchNote: handleSwitchNote,
    createAndSwitch: handleCreateAndSwitch,
    refreshRecentNotes,
    resetSaveFeedback,
    flushSaveNow,
    flushPendingSave,
  };
}

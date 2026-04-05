/**
 * NotesPanel - 禅模式笔记管理面板
 */

import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { createNote, trashNote, trashNotesBatch, listNotes, saveNote, togglePinNote } from '../../lib/tauri/notes';
import type { Note } from '../../types/note';
import { MANAGER_ACTIVATED_EVENT } from './ManagerPage';
import { TagFilterBar } from './notes/filters/TagFilterBar';
import { ListView } from './notes/views/ListView';
import { GridView } from './notes/views/GridView';
import { TimelineView } from './notes/views/TimelineView';
import { useShortcutEngine } from '../../hooks/useShortcutEngine';
import { useAutoSave } from '../../hooks/useAutoSave';
import { useBackupRestore } from '../../hooks/useBackupRestore';
import { NoteEditor } from './notes/NoteEditor';
import type { SaveState } from './notes/types';
import {
  relativeTimeLabel,
  splitTags,
  timelineBucket,
} from './notes/utils';
import { deriveDisplayTitle } from '../../lib/noteParser';
import { applyTextareaSelectionUpdate, focusTextareaAtEnd } from '../../lib/editorSelection';
import { useEditorPreferences } from '../../hooks/useEditorPreferences';
import { useEditorImageInsertion } from '../../hooks/useEditorImageInsertion';

type ViewMode = 'list' | 'grid' | 'timeline';
type FilterLogic = 'and' | 'or';

const AUTOSAVE_DELAY = 900;

function matchesQueryToken(note: Note, token: string): boolean {
  const normalizedToken = token.trim().toLowerCase();
  if (!normalizedToken) {
    return true;
  }

  const noteTags = splitTags(note.tags).map((tag) => tag.toLowerCase());
  const title = deriveDisplayTitle(note.title, note.content).toLowerCase();
  const content = note.content.toLowerCase();

  if (normalizedToken.startsWith('tag:') || normalizedToken.startsWith('#')) {
    const value = normalizedToken.replace(/^tag:/, '').replace(/^#/, '');
    return value ? noteTags.some((tag) => tag.includes(value)) : true;
  }

  if (normalizedToken.startsWith('title:') || normalizedToken.startsWith('t:')) {
    const value = normalizedToken.replace(/^title:/, '').replace(/^t:/, '');
    return value ? title.includes(value) : true;
  }

  if (
    normalizedToken.startsWith('content:') ||
    normalizedToken.startsWith('c:') ||
    normalizedToken.startsWith('body:')
  ) {
    const value = normalizedToken
      .replace(/^content:/, '')
      .replace(/^c:/, '')
      .replace(/^body:/, '');
    return value ? content.includes(value) : true;
  }

  return (
    title.includes(normalizedToken) ||
    content.includes(normalizedToken) ||
    noteTags.some((tag) => tag.includes(normalizedToken))
  );
}

function matchesSearchQuery(note: Note, query: string): boolean {
  const tokens = query
    .split(/\s+/)
    .map((token) => token.trim())
    .filter(Boolean);

  if (tokens.length === 0) {
    return true;
  }

  return tokens.every((token) => matchesQueryToken(note, token));
}

function saveStateLabel(saveState: SaveState): string {
  switch (saveState) {
    case 'saving':
      return '● 保存中';
    case 'unsaved':
      return '○ 未保存';
    case 'error':
      return '✗ 保存失败';
    default:
      return '✓ 已保存';
  }
}

export function NotesPanel() {
  const [notes, setNotes] = useState<Note[]>([]);
  const [isLoading, setIsLoading] = useState(true);
  const [viewMode, setViewMode] = useState<ViewMode>('grid');
  const [searchQuery, setSearchQuery] = useState('');
  const [selectedTags, setSelectedTags] = useState<string[]>([]);
  const [filterLogic, setFilterLogic] = useState<FilterLogic>('or');
  const [editingNoteId, setEditingNoteId] = useState<string | null>(null);
  const [draftTitle, setDraftTitle] = useState('');
  const [draftContent, setDraftContent] = useState('');
  const [draftTags, setDraftTags] = useState('');
  const [selectedNoteIds, setSelectedNoteIds] = useState<Set<string>>(new Set());

  const textareaRef = useRef<HTMLTextAreaElement | null>(null);
  const activationRefreshTimerRef = useRef<number | null>(null);
  const { shortcutConfig, editorSettings, refreshEditorPreferences } = useEditorPreferences({
    refreshEvent: MANAGER_ACTIVATED_EVENT,
  });

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

  const scheduleActivationRefresh = useCallback(() => {
    if (activationRefreshTimerRef.current !== null) {
      window.clearTimeout(activationRefreshTimerRef.current);
    }

    activationRefreshTimerRef.current = window.setTimeout(() => {
      activationRefreshTimerRef.current = null;

      void refreshEditorPreferences();

      if (!editingNoteId) {
        void loadNotes();
      }
    }, 120);
  }, [editingNoteId, loadNotes, refreshEditorPreferences]);

  useEffect(() => {
    loadNotes();
  }, [loadNotes]);

  useEffect(() => {
    let unlisten: (() => void) | undefined;

    const setupFocusListener = async () => {
      try {
        const win = await getCurrentWindow();
        unlisten = await win.onFocusChanged(({ payload: focused }) => {
          if (focused && !editingNoteId) {
            scheduleActivationRefresh();
          }
        });
      } catch (err) {
        console.error('Failed to setup focus listener:', err);
      }
    };

    setupFocusListener();

    return () => {
      if (unlisten) {
        unlisten();
      }
    };
  }, [editingNoteId, scheduleActivationRefresh]);

  useEffect(() => {
    return () => {
      if (activationRefreshTimerRef.current !== null) {
        window.clearTimeout(activationRefreshTimerRef.current);
      }
    };
  }, []);

  useEffect(() => {
    if (!editingNoteId) {
      return;
    }

    return focusTextareaAtEnd(textareaRef, 20);
  }, [editingNoteId]);

  const allTags = useMemo(() => {
    const tagMap = new Map<string, number>();
    for (const note of notes) {
      for (const tag of splitTags(note.tags)) {
        tagMap.set(tag, (tagMap.get(tag) ?? 0) + 1);
      }
    }
    return Array.from(tagMap.entries()).sort((a, b) => b[1] - a[1]);
  }, [notes]);

  const filteredNotes = useMemo(() => {
    return [...notes]
      .sort((a, b) => {
        if (a.is_pinned !== b.is_pinned) {
          return a.is_pinned ? -1 : 1;
        }
        return new Date(b.updated_at).getTime() - new Date(a.updated_at).getTime();
      })
      .filter((note) => {
        const noteTags = splitTags(note.tags);
        const matchesQuery = matchesSearchQuery(note, searchQuery);

        if (!matchesQuery) {
          return false;
        }

        if (selectedTags.length === 0) {
          return true;
        }

        return filterLogic === 'and'
          ? selectedTags.every((tag) => noteTags.includes(tag))
          : selectedTags.some((tag) => noteTags.includes(tag));
      });
  }, [filterLogic, notes, searchQuery, selectedTags]);

  const editingNote = useMemo(
    () => notes.find((note) => note.id === editingNoteId) ?? null,
    [editingNoteId, notes]
  );
  const { saveState, saveError, scheduleSave, flushSave, resetSaveFeedback } = useAutoSave({
    delayMs: AUTOSAVE_DELAY,
    onSaved: (updated) => {
      setNotes((prev) => prev.map((note) => (note.id === updated.id ? updated : note)));
      setDraftTitle(updated.title);
      setDraftContent(updated.content);
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
      setDraftContent(draft.content);
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

  const openNote = useCallback((note: Note) => {
    setEditingNoteId(note.id);
    setDraftTitle(note.title);
    setDraftContent(note.content);
    setDraftTags(note.tags);
    resetSaveFeedback();
    inspectBackup(note);
  }, [inspectBackup, resetSaveFeedback]);

  const closeEditor = useCallback(async () => {
    await flushSave(currentPersistParams);
    setEditingNoteId(null);
    resetSaveFeedback();
    clearBackupDraft();
  }, [clearBackupDraft, currentPersistParams, flushSave, resetSaveFeedback]);

  const handleDraftChange = (field: 'title' | 'content' | 'tags', value: string) => {
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
  };

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
    [draftTags, draftTitle, editingNoteId, scheduleSave]
  );

  const handleCreateNote = async () => {
    try {
      const note = await createNote();
      setNotes((prev) => [note, ...prev]);
      openNote(note);
    } catch (err) {
      console.error('Failed to create note:', err);
    }
  };

  const handleDeleteNote = async (noteId: string) => {
    if (!window.confirm('确定将此笔记移到回收站吗？')) {
      return;
    }

    try {
      await trashNote(noteId);
      setNotes((prev) => prev.filter((note) => note.id !== noteId));
      if (editingNoteId === noteId) {
        setEditingNoteId(null);
      }
    } catch (err) {
      console.error('Failed to trash note:', err);
    }
  };

  const handleTogglePin = async (noteId: string, event?: React.MouseEvent<HTMLButtonElement>) => {
    event?.stopPropagation();
    try {
      const updated = await togglePinNote(noteId);
      setNotes((prev) => prev.map((note) => (note.id === noteId ? updated : note)));
      if (editingNoteId === noteId) {
        setDraftTitle(updated.title);
        setDraftContent(updated.content);
        setDraftTags(updated.tags);
      }
    } catch (err) {
      console.error('Failed to toggle pin note:', err);
    }
  };

  const handleDeleteSelectedNotes = async () => {
    if (selectedNoteIds.size === 0) return;

    try {
      await trashNotesBatch(Array.from(selectedNoteIds));
      setNotes((prev) => prev.filter((note) => !selectedNoteIds.has(note.id)));
      if (editingNoteId && selectedNoteIds.has(editingNoteId)) {
        setEditingNoteId(null);
      }
      setSelectedNoteIds(new Set());
    } catch (err) {
      console.error('Failed to delete selected notes:', err);
    }
  };

  const handleRenameNote = useCallback(async (noteId: string, nextTitle: string) => {
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

  const toggleSelectedTag = (tag: string) => {
    setSelectedTags((prev) =>
      prev.includes(tag) ? prev.filter((item) => item !== tag) : [...prev, tag]
    );
  };

  const appendSearchToken = (token: string) => {
    setSearchQuery((prev) => {
      const trimmed = prev.trim();
      if (!trimmed) {
        return token;
      }
      return trimmed.endsWith(' ') ? `${trimmed}${token}` : `${trimmed} ${token}`;
    });
  };

  const timelineGroups = useMemo(() => {
    const groups = new Map<string, Note[]>();
    for (const note of filteredNotes) {
      const key = timelineBucket(note.updated_at);
      const group = groups.get(key) ?? [];
      group.push(note);
      groups.set(key, group);
    }
    return Array.from(groups.entries());
  }, [filteredNotes]);

  useShortcutEngine({
    enabled: Boolean(editingNote),
    textareaRef,
    config: shortcutConfig,
    onSave: async () => {
      await flushSave(currentPersistParams);
    },
    onClose: closeEditor,
    onChange: applyShortcutContentChange,
  });
  const imageInsertion = useEditorImageInsertion({
    value: draftContent,
    textareaRef,
    onChange: applyShortcutContentChange,
  });

  // Handle Delete key for batch delete selected notes
  useEffect(() => {
    const handleKeyDown = (event: KeyboardEvent) => {
    if (event.key === 'Delete' && selectedNoteIds.size > 0 && !editingNoteId) {
      event.preventDefault();
      void handleDeleteSelectedNotes();
    }
  };

    window.addEventListener('keydown', handleKeyDown);
    return () => window.removeEventListener('keydown', handleKeyDown);
  }, [selectedNoteIds, editingNoteId]);

  if (isLoading) {
    return (
      <div className="flex h-full items-center justify-center">
        <div className="h-7 w-7 rounded-full border-2 border-[color:var(--manager-border)] border-t-[color:var(--manager-accent)] animate-spin" />
      </div>
    );
  }

  if (editingNote) {
    return (
      <NoteEditor
        note={editingNote}
        draftTitle={draftTitle}
        draftContent={draftContent}
        draftTags={draftTags}
        allTags={allTags}
        saveStateLabel={saveStateLabel(saveState)}
        saveError={saveError}
        backupDraft={backupDraft}
        textareaRef={textareaRef}
        shortcutConfig={shortcutConfig}
        editorSettings={editorSettings}
        splitTags={splitTags}
        imageDragActive={imageInsertion.isDragActive}
        onPasteImage={imageInsertion.handlePaste}
        onDragEnterImage={imageInsertion.handleDragEnter}
        onDragLeaveImage={imageInsertion.handleDragLeave}
        onDragOverImage={imageInsertion.handleDragOver}
        onDropImage={imageInsertion.handleDrop}
        onClose={closeEditor}
        onTogglePin={(event) => handleTogglePin(editingNote.id, event)}
        onTitleChange={(value) => handleDraftChange('title', value)}
        onContentChange={(value) => handleDraftChange('content', value)}
        onTagsChange={(value) => handleDraftChange('tags', value)}
        onTagRemove={(tag) =>
          handleDraftChange(
            'tags',
            splitTags(draftTags).filter((item) => item !== tag).join(' ')
          )
        }
        onKeepCurrentDraft={clearBackupDraft}
        onRestoreBackupDraft={restoreBackupDraft}
        onMergeBackupDraft={mergeBackupDraft}
      />
    );
  }

  return (
    <section className="flex h-full flex-col gap-2 px-1 py-1 sm:px-2 sm:py-2">
      <header className="px-3 py-2 sm:px-4 sm:py-2.5">
        <div className="flex flex-col gap-2">
            <div className="grid gap-2 xl:grid-cols-[auto_minmax(0,1fr)_auto] xl:items-center">
              <div className="space-y-1">
                <p className="manager-kicker">Notes Archive</p>
                <h2 className="font-['Iowan_Old_Style','Palatino_Linotype','Noto_Serif_SC',serif] text-[32px] leading-[1.04] text-[color:var(--manager-ink-strong)]">笔记管理</h2>
              </div>
              <div className="grid gap-2 xl:grid-cols-[minmax(0,1fr)_minmax(320px,420px)]">
              <div className="rounded-[15px] bg-[rgba(255,251,245,0.52)] px-3 py-2">
                <div className="flex items-center gap-3">
                  <span className="flex h-9 w-9 items-center justify-center rounded-full border border-[color:var(--manager-border)] bg-[rgba(255,251,245,0.56)] text-xs tracking-[0.24em] text-[color:var(--manager-ink-soft)]">
                    检
                  </span>
                  <input
                    type="text"
                    placeholder="搜索全部，或用 title: / content: / tag:"
                    value={searchQuery}
                    onChange={(event) => setSearchQuery(event.target.value)}
                    className="w-full border-none bg-transparent text-sm text-[color:var(--manager-ink)] outline-none placeholder:text-[color:var(--manager-ink-subtle)]"
                  />
                  <span className="shrink-0 text-xs uppercase tracking-[0.22em] text-[color:var(--manager-ink-subtle)]">
                    {filteredNotes.length} / {notes.length}
                  </span>
                </div>
                <div className="mt-2 flex flex-wrap gap-2 pl-12">
                  {[
                    ['标题', 'title:'],
                    ['正文', 'content:'],
                    ['标签', 'tag:'],
                  ].map(([label, token]) => (
                    <button
                      key={token}
                      onClick={() => appendSearchToken(token)}
                      className="rounded-full border border-[color:var(--manager-border)] bg-[rgba(255,251,245,0.56)] px-3 py-1 text-xs tracking-[0.14em] text-[color:var(--manager-ink-soft)] transition hover:border-[color:var(--manager-accent)] hover:text-[color:var(--manager-ink-strong)]"
                    >
                      {label}
                    </button>
                  ))}
                  {searchQuery ? (
                    <button
                      onClick={() => setSearchQuery('')}
                      className="rounded-full border border-transparent px-3 py-1 text-xs tracking-[0.14em] text-[color:var(--manager-ink-subtle)] transition hover:border-[color:var(--manager-border)] hover:text-[color:var(--manager-ink-strong)]"
                    >
                      清空
                    </button>
                  ) : null}
                </div>
              </div>
              <TagFilterBar
                selectedTags={selectedTags}
                allTags={allTags}
                filterLogic={filterLogic}
                onToggleTag={toggleSelectedTag}
                onChangeLogic={setFilterLogic}
                onClear={() => setSelectedTags([])}
              />
              </div>
              <div className="flex flex-wrap items-center justify-end gap-2">
                <div className="flex items-center gap-1 rounded-full border border-[color:var(--manager-border)] bg-white/70 p-1">
                  {([
                    ['list', '列'],
                    ['grid', '格'],
                    ['timeline', '时'],
                  ] as const).map(([mode, label]) => (
                    <button
                      key={mode}
                      onClick={() => setViewMode(mode)}
                      className={`flex h-10 min-w-10 items-center justify-center rounded-full px-4 text-sm transition ${
                        viewMode === mode
                          ? 'bg-[color:var(--manager-accent)] text-white'
                          : 'text-[color:var(--manager-ink-soft)] hover:bg-[color:var(--manager-accent-soft)] hover:text-[color:var(--manager-ink-strong)]'
                      }`}
                      title={mode}
                    >
                      {label}
                    </button>
                  ))}
                </div>
                <button
                  onClick={() => void handleCreateNote()}
                  className="rounded-full bg-[color:var(--manager-accent)] px-5 py-3 text-sm tracking-[0.16em] text-white transition hover:bg-[color:var(--manager-accent-strong)]"
                >
                  新建笔记
                </button>
              </div>
            </div>

            <div className="flex items-center justify-end px-1">
              <div className="text-sm text-[color:var(--manager-ink-soft)]">
                当前快捷键：保存 {shortcutConfig.save}，退出 {shortcutConfig.close}
              </div>
            </div>

        </div>
      </header>

      <div className="flex-1 overflow-auto rounded-[16px] bg-[rgba(255,249,242,0.24)] px-2 py-2">
        {filteredNotes.length === 0 ? (
          <div className="flex h-full min-h-[320px] items-center justify-center rounded-[20px] border border-dashed border-[color:var(--manager-border)] bg-white/35 text-center">
            <div>
              <p className="font-['Iowan_Old_Style','Palatino_Linotype','Noto_Serif_SC',serif] text-3xl text-[color:var(--manager-ink-strong)]">暂无匹配内容</p>
              <p className="mt-3 text-sm text-[color:var(--manager-ink-soft)]">调整搜索或标签组合，或者直接新建一条笔记。</p>
            </div>
          </div>
        ) : null}

        {filteredNotes.length > 0 && viewMode === 'list' ? (
          <ListView
            notes={filteredNotes}
            splitTags={splitTags}
            relativeTimeLabel={relativeTimeLabel}
            onOpenNote={openNote}
            onRenameNote={handleRenameNote}
            onTogglePin={handleTogglePin}
            onDeleteNote={handleDeleteNote}
          />
        ) : null}

        {filteredNotes.length > 0 && viewMode === 'grid' ? (
          <GridView
            notes={filteredNotes}
            splitTags={splitTags}
            relativeTimeLabel={relativeTimeLabel}
            onOpenNote={openNote}
            onRenameNote={handleRenameNote}
            onTogglePin={handleTogglePin}
            onDeleteNote={handleDeleteNote}
          />
        ) : null}

        {filteredNotes.length > 0 && viewMode === 'timeline' ? (
          <TimelineView groups={timelineGroups} onOpenNote={openNote} onRenameNote={handleRenameNote} />
        ) : null}
      </div>
    </section>
  );
}

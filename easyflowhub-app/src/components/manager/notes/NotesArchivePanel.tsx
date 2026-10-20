/**
 * NotesArchivePanel - 笔记归档视图（列表 / 网格 / 时间线 + 编辑器）
 * 列表与编辑会话的编排归 useNotesArchive；本组件只保留视图状态、
 * 搜索 / 标签筛选与窗口交互（激活刷新、外部打开事件、Delete 批量删除）。
 */

import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { getCurrentWindow } from '@tauri-apps/api/window';
import type { Note } from '../../../types/note';
import { MANAGER_ACTIVATED_EVENT, MANAGER_OPEN_NOTE_EVENT } from '../ManagerPage';
import { TagFilterBar } from './filters/TagFilterBar';
import { ListView } from './views/ListView';
import { GridView } from './views/GridView';
import { TimelineView } from './views/TimelineView';
import { useShortcutEngine } from '../../../hooks/useShortcutEngine';
import { useEditorPreferences } from '../../../hooks/useEditorPreferences';
import { useEditorImageInsertion } from '../../../hooks/useEditorImageInsertion';
import { useNotesArchive } from '../../../hooks/useNotesArchive';
import { NoteEditor } from './NoteEditor';
import type { SaveState } from '../../../types/notes';
import {
  relativeTimeLabel,
  splitTags,
  timelineBucket,
} from '../../../lib/notesUtils';
import { matchesSearchQuery } from '../../../lib/noteSearch';
import { focusTextareaAtEnd } from '../../../lib/editorSelection';

type ViewMode = 'list' | 'grid' | 'timeline';
type FilterLogic = 'and' | 'or';

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

export function NotesArchivePanel() {
  const [viewMode, setViewMode] = useState<ViewMode>('grid');
  const [searchQuery, setSearchQuery] = useState('');
  const [selectedTags, setSelectedTags] = useState<string[]>([]);
  const [filterLogic, setFilterLogic] = useState<FilterLogic>('or');
  const [selectedNoteIds, setSelectedNoteIds] = useState<Set<string>>(new Set());

  const activationRefreshTimerRef = useRef<number | null>(null);
  const { shortcutConfig, editorSettings, refreshEditorPreferences } = useEditorPreferences({
    refreshEvent: MANAGER_ACTIVATED_EVENT,
  });

  const archive = useNotesArchive({ maxUndoSteps: editorSettings.undo_steps });
  const {
    notes,
    isLoading,
    loadNotes,
    editingNoteId,
    editingNote,
    draftTitle,
    draftContent,
    draftTags,
    saveState,
    saveError,
    persistParams,
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
  } = archive;

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

  // 外部（其他窗口 / Launcher 动作）请求打开指定笔记。
  useEffect(() => {
    const handleOpenNote = (event: Event) => {
      const customEvent = event as CustomEvent<{ noteId?: string }>;
      const noteId = customEvent.detail?.noteId;
      if (!noteId) {
        return;
      }

      void openNoteById(noteId);
    };

    window.addEventListener(MANAGER_OPEN_NOTE_EVENT, handleOpenNote as EventListener);
    return () => window.removeEventListener(MANAGER_OPEN_NOTE_EVENT, handleOpenNote as EventListener);
  }, [openNoteById]);

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

  const handleTogglePin = async (noteId: string, event?: React.MouseEvent<HTMLButtonElement>) => {
    event?.stopPropagation();
    await togglePin(noteId);
  };

  const handleDeleteNote = async (noteId: string) => {
    if (!window.confirm('确定将此笔记移到回收站吗？')) {
      return;
    }

    await trashNoteById(noteId);
  };

  const handleDeleteSelectedNotes = async () => {
    if (selectedNoteIds.size === 0) return;

    await trashNotesByIds(Array.from(selectedNoteIds));
    setSelectedNoteIds(new Set());
  };

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
      await flushSave(persistParams);
    },
    onClose: closeEditor,
    onChange: applyShortcutContentChange,
    onUndo: undoDraftContent,
    onRedo: redoDraftContent,
    canUndo: canUndoDraftContent,
    canRedo: canRedoDraftContent,
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
    <section className="flex h-full flex-col gap-1 px-0 py-0">
      <header className="border-b border-[color:var(--manager-border)] px-3 py-3 sm:px-4 sm:py-3">
        <div className="flex flex-col gap-2">
            <div className="grid gap-2 xl:grid-cols-[auto_minmax(0,1fr)_auto] xl:items-center">
              <div className="space-y-1">
                <p className="manager-kicker">Notes Archive</p>
                <h2 className="font-['Iowan_Old_Style','Palatino_Linotype','Noto_Serif_SC',serif] text-[32px] leading-[1.04] text-[color:var(--manager-ink-strong)]">笔记管理</h2>
              </div>
              <div className="grid gap-2 xl:grid-cols-[minmax(0,1fr)_minmax(320px,420px)]">
              <div className="rounded-[12px] border border-[color:var(--manager-border)] bg-[color:var(--manager-panel)] px-3 py-2">
                <div className="flex items-center gap-3">
                  <span className="flex h-8 w-8 items-center justify-center rounded-[10px] border border-[color:var(--manager-border)] bg-[color:var(--manager-panel-strong)] text-xs tracking-[0.24em] text-[color:var(--manager-ink-soft)]">
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
                      className="rounded-[8px] border border-[color:var(--manager-border)] bg-[color:var(--manager-panel-strong)] px-2.5 py-1 text-xs tracking-[0.14em] text-[color:var(--manager-ink-soft)] transition hover:border-[color:var(--manager-accent)] hover:text-[color:var(--manager-ink-strong)]"
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
                <div className="flex items-center gap-1 rounded-[10px] border border-[color:var(--manager-border)] bg-[color:var(--manager-panel)] p-1">
                  {([
                    ['list', '列'],
                    ['grid', '格'],
                    ['timeline', '时'],
                  ] as const).map(([mode, label]) => (
                    <button
                      key={mode}
                      onClick={() => setViewMode(mode)}
                      className={`flex h-9 min-w-9 items-center justify-center rounded-[7px] px-3 text-sm transition ${
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
                  onClick={() => void createAndOpenNote()}
                  className="rounded-[10px] bg-[color:var(--manager-accent)] px-4 py-2.5 text-sm tracking-[0.16em] text-white transition hover:bg-[color:var(--manager-accent-strong)]"
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

      <div className="flex-1 overflow-auto px-2 py-2">
        {filteredNotes.length === 0 ? (
          <div className="flex h-full min-h-[320px] items-center justify-center rounded-[12px] border border-dashed border-[color:var(--manager-border)] bg-[color:var(--manager-panel)] text-center">
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
            onRenameNote={renameNote}
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
            onRenameNote={renameNote}
            onTogglePin={handleTogglePin}
            onDeleteNote={handleDeleteNote}
          />
        ) : null}

        {filteredNotes.length > 0 && viewMode === 'timeline' ? (
          <TimelineView groups={timelineGroups} onOpenNote={openNote} onRenameNote={renameNote} />
        ) : null}
      </div>
    </section>
  );
}

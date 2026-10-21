/**
 * TrashPanel - 回收站面板
 * 统一管理已删除的笔记、收集项与脚本。
 *
 * The panel owns the three lists, the active tab and the destructive-action
 * confirmations; every tab body is rendered by its own view under `./trash/`,
 * which also owns that tab's list interactions.
 */

import { useCallback, useEffect, useState } from 'react';
import { emptyTrash, listTrash, restoreNotesBatch } from '../../lib/tauri/notes';
import { listInboxTrash } from '../../lib/tauri/inbox';
import { scriptTrashApi } from '../../lib/api/scriptmgr';
import type { Note } from '../../types/note';
import type { ScriptTrashEntry } from '../../types/scriptmgr';
import type { InboxItem } from '../../types/inbox';
import { TrashNotesView } from './trash/TrashNotesView';
import { TrashInboxView } from './trash/TrashInboxView';
import { TrashScriptsView } from './trash/TrashScriptsView';
import { TrashEmptyAction } from './trash/trashShared';

type TrashView = 'notes' | 'inbox' | 'scripts';

export function TrashPanel() {
  const [view, setView] = useState<TrashView>('notes');
  const [trashedNotes, setTrashedNotes] = useState<Note[]>([]);
  const [selectedIds, setSelectedIds] = useState<Set<string>>(new Set());
  const [isLoading, setIsLoading] = useState(true);

  const [trashedScripts, setTrashedScripts] = useState<ScriptTrashEntry[]>([]);
  const [scriptLoading, setScriptLoading] = useState(true);
  const [scriptError, setScriptError] = useState<string>();

  const [trashedInboxItems, setTrashedInboxItems] = useState<InboxItem[]>([]);
  const [inboxLoading, setInboxLoading] = useState(true);
  const [inboxError, setInboxError] = useState<string>();

  const loadTrash = useCallback(async () => {
    setIsLoading(true);
    try {
      setTrashedNotes(await listTrash());
    } catch (err) {
      console.error('Failed to load trash:', err);
    } finally {
      setIsLoading(false);
    }
  }, []);

  const loadScriptTrash = useCallback(async () => {
    setScriptLoading(true);
    try {
      setTrashedScripts(await scriptTrashApi.list());
      setScriptError(undefined);
    } catch (err) {
      setScriptError(err instanceof Error ? err.message : String(err));
    } finally {
      setScriptLoading(false);
    }
  }, []);

  const loadInboxTrash = useCallback(async () => {
    setInboxLoading(true);
    try {
      setTrashedInboxItems(await listInboxTrash());
      setInboxError(undefined);
    } catch (err) {
      setInboxError(err instanceof Error ? err.message : String(err));
    } finally {
      setInboxLoading(false);
    }
  }, []);

  useEffect(() => {
    void loadTrash();
    void loadScriptTrash();
    void loadInboxTrash();
  }, [loadTrash, loadScriptTrash, loadInboxTrash]);

  const clearSelection = useCallback(() => {
    setSelectedIds(new Set());
  }, []);

  const handleRestoreSelected = useCallback(async () => {
    if (selectedIds.size === 0) return;
    try {
      await restoreNotesBatch(Array.from(selectedIds));
      setTrashedNotes((previous) => previous.filter((note) => !selectedIds.has(note.id)));
      clearSelection();
    } catch (err) {
      console.error('Failed to restore selected notes:', err);
    }
  }, [clearSelection, selectedIds]);

  const handleEmptyNotesTrash = useCallback(async () => {
    try {
      await emptyTrash();
      setTrashedNotes([]);
      clearSelection();
    } catch (err) {
      console.error('Failed to empty trash:', err);
    }
  }, [clearSelection]);

  if (isLoading && scriptLoading && inboxLoading) {
    return (
      <div className="flex h-full items-center justify-center">
        <div className="h-7 w-7 rounded-full border-2 border-[color:var(--manager-border)] border-t-[color:var(--manager-accent)] animate-spin" />
      </div>
    );
  }

  const summary = view === 'notes'
    ? trashedNotes.length === 0
      ? '回收站为空'
      : `共 ${trashedNotes.length} 条笔记，已删除的笔记将保留 30 天后自动清除。`
    : view === 'inbox'
      ? trashedInboxItems.length === 0
        ? '没有已删除的收集项'
        : `共 ${trashedInboxItems.length} 条收集项，恢复后会回到原收集箱状态。`
      : trashedScripts.length === 0
        ? '没有已删除的脚本'
        : `共 ${trashedScripts.length} 个脚本，恢复后会回到原来的存储位置。`;

  return (
    <section className="flex h-full flex-col">
      <header className="border-b border-[color:var(--manager-border)] px-5 py-4 sm:px-7 sm:py-5">
        <div className="flex flex-col gap-5">
          <div className="flex flex-col gap-4 xl:flex-row xl:items-end xl:justify-between">
            <div>
              <p className="manager-kicker">Trash</p>
              <h2 className="mt-3 font-['Iowan_Old_Style','Palatino_Linotype','Noto_Serif_SC',serif] text-4xl leading-none text-[color:var(--manager-ink-strong)]">回收站</h2>
              <p className="mt-3 max-w-2xl text-sm leading-7 text-[color:var(--manager-ink-soft)]">{summary}</p>
              <div className="mt-4 flex items-center gap-2" role="tablist" aria-label="回收站类型">
                {(['notes', 'inbox', 'scripts'] as const).map((value) => (
                  <button
                    key={value}
                    type="button"
                    role="tab"
                    aria-selected={view === value}
                    onClick={() => setView(value)}
                    className={`rounded-[8px] border px-3 py-1.5 text-sm transition ${
                      view === value
                        ? 'border-[color:var(--manager-accent)] bg-[color:var(--manager-accent-soft)] text-[color:var(--manager-ink-strong)]'
                        : 'border-transparent text-[color:var(--manager-ink-muted)] hover:bg-[color:var(--manager-panel-strong)]'
                    }`}
                  >
                    {value === 'notes'
                      ? `笔记 (${trashedNotes.length})`
                      : value === 'inbox'
                        ? `收集箱 (${trashedInboxItems.length})`
                        : `脚本 (${trashedScripts.length})`}
                  </button>
                ))}
              </div>
            </div>
            <div className="flex flex-wrap items-center gap-3">
              {view === 'notes' && selectedIds.size > 1 && (
                <>
                  <button
                    onClick={handleRestoreSelected}
                    className="rounded-full bg-[color:var(--manager-accent)] px-5 py-3 text-sm text-white transition hover:bg-[color:var(--manager-accent-strong)]"
                  >
                    恢复选中 ({selectedIds.size})
                  </button>
                  <button
                    onClick={clearSelection}
                    className="rounded-full border border-[color:var(--manager-border)] bg-[color:var(--manager-panel-strong)] px-5 py-3 text-sm text-[color:var(--manager-ink-soft)] transition hover:border-[color:var(--manager-accent)] hover:text-[color:var(--manager-ink-strong)]"
                  >
                    取消选择
                  </button>
                </>
              )}
              {view === 'notes' && trashedNotes.length > 0 && (
                <TrashEmptyAction onEmpty={handleEmptyNotesTrash}>清空回收站</TrashEmptyAction>
              )}
            </div>
          </div>
        </div>
      </header>

      <div className="flex-1 overflow-auto px-5 py-4 sm:px-7 sm:py-5">
        {view === 'notes' ? (
          <TrashNotesView
            notes={trashedNotes}
            onNotesChange={setTrashedNotes}
            selectedIds={selectedIds}
            onSelectionChange={setSelectedIds}
          />
        ) : view === 'inbox' ? (
          <TrashInboxView
            items={trashedInboxItems}
            isLoading={inboxLoading}
            error={inboxError}
            onItemsChange={setTrashedInboxItems}
            onErrorChange={setInboxError}
          />
        ) : (
          <TrashScriptsView
            entries={trashedScripts}
            isLoading={scriptLoading}
            error={scriptError}
            onEntriesChange={setTrashedScripts}
            onErrorChange={setScriptError}
          />
        )}
      </div>
    </section>
  );
}

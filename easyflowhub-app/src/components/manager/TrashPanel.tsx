/**
 * TrashPanel - 回收站面板
 * 统一管理已删除的笔记与脚本。
 */

import { useCallback, useEffect, useMemo, useState } from 'react';
import {
  listTrash,
  restoreNote,
  restoreNotesBatch,
  permanentDeleteNote,
  emptyTrash,
} from '../../lib/tauri/notes';
import { scriptTrashApi } from '../../lib/api/scriptmgr';
import type { Note } from '../../types/note';
import type { ScriptTrashEntry } from '../../types/scriptmgr';
import { deriveDisplayTitle } from '../../lib/noteParser';

type TrashView = 'notes' | 'scripts';

function formatDeletedTime(deletedAt: string): string {
  const date = new Date(deletedAt);
  const now = new Date();
  const diffMs = now.getTime() - date.getTime();
  const diffDays = Math.floor(diffMs / (1000 * 60 * 60 * 24));

  if (diffDays === 0) return '1 天前';
  if (diffDays < 7) return `${diffDays} 天前`;
  if (diffDays < 30) return `${Math.floor(diffDays / 7)} 周前`;
  return date.toLocaleDateString('zh-CN');
}

function formatBytes(bytes: number): string {
  if (!bytes) return '-';
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}

export function TrashPanel() {
  const [view, setView] = useState<TrashView>('notes');
  const [trashedNotes, setTrashedNotes] = useState<Note[]>([]);
  const [isLoading, setIsLoading] = useState(true);
  const [selectedIds, setSelectedIds] = useState<Set<string>>(new Set());

  const [trashedScripts, setTrashedScripts] = useState<ScriptTrashEntry[]>([]);
  const [scriptLoading, setScriptLoading] = useState(true);
  const [scriptError, setScriptError] = useState<string>();

  const loadTrash = useCallback(async () => {
    setIsLoading(true);
    try {
      const notes = await listTrash();
      setTrashedNotes(notes);
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

  useEffect(() => {
    void loadTrash();
    void loadScriptTrash();
  }, [loadTrash, loadScriptTrash]);

  const toggleSelection = useCallback((id: string) => {
    setSelectedIds((prev) => {
      const next = new Set(prev);
      if (next.has(id)) {
        next.delete(id);
      } else {
        next.add(id);
      }
      return next;
    });
  }, []);

  const clearSelection = useCallback(() => {
    setSelectedIds(new Set());
  }, []);

  const handleRestore = useCallback(async (id: string) => {
    try {
      await restoreNote(id);
      setTrashedNotes((prev) => prev.filter((n) => n.id !== id));
      setSelectedIds((prev) => {
        const next = new Set(prev);
        next.delete(id);
        return next;
      });
    } catch (err) {
      console.error('Failed to restore note:', err);
    }
  }, []);

  const handleRestoreSelected = useCallback(async () => {
    if (selectedIds.size === 1) return;
    try {
      await restoreNotesBatch(Array.from(selectedIds));
      setTrashedNotes((prev) => prev.filter((n) => !selectedIds.has(n.id)));
      setSelectedIds(new Set());
    } catch (err) {
      console.error('Failed to restore selected notes:', err);
    }
  }, [selectedIds]);

  const handlePermanentDelete = useCallback(async (id: string) => {
    try {
      await permanentDeleteNote(id);
      setTrashedNotes((prev) => prev.filter((n) => n.id !== id));
      setSelectedIds((prev) => {
        const next = new Set(prev);
        next.delete(id);
        return next;
      });
    } catch (err) {
      console.error('Failed to permanently delete note:', err);
    }
  }, []);

  const handleEmptyTrash = useCallback(async () => {
    try {
      await emptyTrash();
      setTrashedNotes([]);
      setSelectedIds(new Set());
    } catch (err) {
      console.error('Failed to empty trash:', err);
    }
  }, []);

  const handleRestoreScript = useCallback(async (entry: ScriptTrashEntry) => {
    try {
      await scriptTrashApi.restore(entry.id);
      setTrashedScripts((prev) => prev.filter((item) => item.id !== entry.id));
      setScriptError(undefined);
    } catch (err) {
      setScriptError(err instanceof Error ? err.message : String(err));
    }
  }, []);

  const handlePurgeScript = useCallback(async (entry: ScriptTrashEntry) => {
    if (!window.confirm(`彻底删除“${entry.name}”？此操作不可恢复。`)) return;
    try {
      await scriptTrashApi.purge(entry.id);
      setTrashedScripts((prev) => prev.filter((item) => item.id !== entry.id));
      setScriptError(undefined);
    } catch (err) {
      setScriptError(err instanceof Error ? err.message : String(err));
    }
  }, []);

  const handleEmptyScriptTrash = useCallback(async () => {
    if (!window.confirm('彻底删除回收站中的所有脚本？此操作不可恢复。')) return;
    try {
      await scriptTrashApi.empty();
      setTrashedScripts([]);
      setScriptError(undefined);
    } catch (err) {
      setScriptError(err instanceof Error ? err.message : String(err));
    }
  }, []);

  const groupedNotes = useMemo(() => {
    const groups = new Map<string, Note[]>();
    for (const note of trashedNotes) {
      const key = note.deleted_at ? formatDeletedTime(note.deleted_at) : '未知';
      const group = groups.get(key) ?? [];
      group.push(note);
      groups.set(key, group);
    }
    return Array.from(groups.entries()).sort((a, b) => {
      const dateA = new Date(a[0] || 0);
      const dateB = new Date(b[0] || 0);
      return dateB.getTime() - dateA.getTime();
    });
  }, [trashedNotes]);

  if (isLoading && scriptLoading) {
    return (
      <div className="flex h-full items-center justify-center">
        <div className="h-7 w-7 rounded-full border-2 border-[color:var(--manager-border)] border-t-[color:var(--manager-accent)] animate-spin" />
      </div>
    );
  }

  const summary =
    view === 'notes'
      ? trashedNotes.length === 0
        ? '回收站为空'
        : `共 ${trashedNotes.length} 条笔记，已删除的笔记将保留 30 天后自动清除。`
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
                {(['notes', 'scripts'] as const).map((value) => (
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
                    {value === 'notes' ? `笔记 (${trashedNotes.length})` : `脚本 (${trashedScripts.length})`}
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
                <button
                  onClick={handleEmptyTrash}
                  className="rounded-full border border-[color:var(--manager-danger)]/40 bg-[color:var(--manager-danger)]/12 px-5 py-3 text-sm text-[color:var(--manager-danger)] transition hover:bg-[color:var(--manager-danger)]/20"
                >
                  清空回收站
                </button>
              )}
              {view === 'scripts' && trashedScripts.length > 0 && (
                <button
                  onClick={() => void handleEmptyScriptTrash()}
                  className="rounded-full border border-[color:var(--manager-danger)]/40 bg-[color:var(--manager-danger)]/12 px-5 py-3 text-sm text-[color:var(--manager-danger)] transition hover:bg-[color:var(--manager-danger)]/20"
                >
                  清空脚本回收站
                </button>
              )}
            </div>
          </div>
        </div>
      </header>

      <div className="flex-1 overflow-auto px-5 py-4 sm:px-7 sm:py-5">
        {view === 'scripts' ? (
          <div className="space-y-3">
            {scriptError ? (
              <p role="alert" className="rounded-[12px] border border-[color:var(--manager-danger)]/40 px-4 py-2 text-xs text-[color:var(--manager-danger)]">
                {scriptError}
              </p>
            ) : null}
            {scriptLoading ? (
              <p className="py-10 text-center text-sm text-[color:var(--manager-ink-muted)]">加载中…</p>
            ) : trashedScripts.length === 0 ? (
              <div className="flex h-full min-h-[320px] items-center justify-center rounded-[20px] border border-dashed border-[color:var(--manager-border)] bg-[color:var(--manager-panel)] text-center">
                <div>
                  <p className="font-['Iowan_Old_Style','Palatino_Linotype','Noto_Serif_SC',serif] text-3xl text-[color:var(--manager-ink-strong)]">没有已删除的脚本</p>
                  <p className="mt-3 text-sm text-[color:var(--manager-ink-soft)]">在脚本工作台中删除的脚本会显示在这里。</p>
                </div>
              </div>
            ) : (
              trashedScripts.map((entry) => (
                <div
                  key={entry.id}
                  className="rounded-[18px] border border-[color:var(--manager-border)] bg-[color:var(--manager-panel)] p-4 transition hover:border-[color:var(--manager-accent)]"
                >
                  <div className="flex items-start justify-between gap-3">
                    <div className="min-w-0 flex-1">
                      <div className="flex flex-wrap items-center gap-2">
                        <span className="rounded-[5px] border border-[color:var(--manager-border)] px-1.5 py-0.5 text-[10px] text-[color:var(--manager-ink-muted)]">
                          {entry.script_type}
                        </span>
                        <span className="text-[11px] text-[color:var(--manager-ink-subtle)]">
                          {formatDeletedTime(entry.deleted_at)} · {formatBytes(entry.size_bytes)}
                        </span>
                      </div>
                      <p className="mt-1 break-words text-sm font-medium text-[color:var(--manager-ink-strong)]">{entry.name}</p>
                      <p className="mt-1 break-all font-mono text-[11px] text-[color:var(--manager-ink-subtle)]">{entry.original_path}</p>
                    </div>
                    <div className="flex shrink-0 items-center gap-2">
                      <button
                        onClick={() => void handleRestoreScript(entry)}
                        className="rounded-full bg-[color:var(--manager-success)]/12 px-3 py-1 text-xs text-[color:var(--manager-success)] transition hover:bg-[color:var(--manager-success)]/20"
                      >
                        恢复
                      </button>
                      <button
                        onClick={() => void handlePurgeScript(entry)}
                        className="rounded-full bg-[color:var(--manager-danger)]/12 px-3 py-1 text-xs text-[color:var(--manager-danger)] transition hover:bg-[color:var(--manager-danger)]/20"
                      >
                        永久删除
                      </button>
                    </div>
                  </div>
                </div>
              ))
            )}
          </div>
        ) : (
          <>
            {trashedNotes.length === 0 ? (
              <div className="flex h-full min-h-[320px] items-center justify-center rounded-[20px] border border-dashed border-[color:var(--manager-border)] bg-[color:var(--manager-panel)] text-center">
                <div>
                  <p className="font-['Iowan_Old_Style','Palatino_Linotype','Noto_Serif_SC',serif] text-3xl text-[color:var(--manager-ink-strong)]">回收站为空</p>
                  <p className="mt-3 text-sm text-[color:var(--manager-ink-soft)]">删除的笔记会显示在这里。</p>
                </div>
              </div>
            ) : null}

            {groupedNotes.map(([timeKey, notes]) => (
              <div key={timeKey} className="mb-6">
                <p className="mb-3 px-1 text-xs font-medium tracking-[0.12em] text-[color:var(--manager-ink-subtle)]">
                  {timeKey}
                </p>
                <div className="space-y-2">
                  {notes.map((note) => {
                    const title = deriveDisplayTitle(note.title, note.content) || '无标题';
                    const isSelected = selectedIds.has(note.id);

                    return (
                      <div
                        key={note.id}
                        className={`rounded-[18px] border p-4 transition ${
                          isSelected
                            ? 'border-[color:var(--manager-accent)] bg-[color:var(--manager-accent-soft)]'
                            : 'border-[color:var(--manager-border)] bg-[color:var(--manager-panel)] hover:border-[color:var(--manager-accent)]'
                        }`}
                      >
                        <div className="flex items-start justify-between gap-3">
                          <div className="flex-1 min-w-0">
                            <div className="flex items-center gap-2">
                              <input
                                type="checkbox"
                                checked={isSelected}
                                onChange={() => toggleSelection(note.id)}
                                className="h-4 w-4 rounded border-[color:var(--manager-border)] accent-checked:border-[color:var(--manager-accent)] accent-checked:bg-[color:var(--manager-accent-soft)]"
                              />
                              <div className="flex-1">
                                <p className="text-sm font-medium text-[color:var(--manager-ink-strong)]">
                                  {title}
                                </p>
                                <p className="mt-1 text-xs text-[color:var(--manager-ink-subtle)] line-clamp-2">
                                  {note.content.slice(0, 100)}...
                                </p>
                              </div>
                            </div>
                          </div>
                          <div className="flex items-center gap-2">
                            <button
                              onClick={() => handleRestore(note.id)}
                              className="rounded-full bg-[color:var(--manager-success)]/12 px-3 py-1 text-xs text-[color:var(--manager-success)] transition hover:bg-[color:var(--manager-success)]/20"
                            >
                              恢复
                            </button>
                            <button
                              onClick={() => handlePermanentDelete(note.id)}
                              className="rounded-full bg-[color:var(--manager-danger)]/12 px-3 py-1 text-xs text-[color:var(--manager-danger)] transition hover:bg-[color:var(--manager-danger)]/20"
                            >
                              永久删除
                            </button>
                          </div>
                        </div>
                      </div>
                    );
                  })}
                </div>
              </div>
            ))}
          </>
        )}
      </div>
    </section>
  );
}

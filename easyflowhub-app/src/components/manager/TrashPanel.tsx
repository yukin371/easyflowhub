/**
 * TrashPanel - 回收站面板
 */

import { useCallback, useEffect, useMemo, useState } from 'react';
import {
  listTrash,
  restoreNote,
  restoreNotesBatch,
  permanentDeleteNote,
  emptyTrash,
} from '../../lib/tauri/notes';
import type { Note } from '../../types/note';
import { deriveDisplayTitle } from '../../lib/noteParser';

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

export function TrashPanel() {
  const [trashedNotes, setTrashedNotes] = useState<Note[]>([]);
  const [isLoading, setIsLoading] = useState(true);
  const [selectedIds, setSelectedIds] = useState<Set<string>>(new Set());

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

  useEffect(() => {
    loadTrash();
  }, [loadTrash]);

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

  // Reserved for future select all UI
  const selectAll = useCallback(() => {
    setSelectedIds(new Set(trashedNotes.map((n) => n.id)));
  }, [trashedNotes]);
  void selectAll; // Suppress unused warning

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

  if (isLoading) {
    return (
      <div className="flex h-full items-center justify-center">
        <div className="h-7 w-7 rounded-full border-2 border-[color:var(--manager-border)] border-t-[color:var(--manager-accent)] animate-spin" />
      </div>
    );
  }

  return (
    <section className="flex h-full flex-col">
      <header className="border-b border-[color:var(--manager-border)] px-5 py-4 sm:px-7 sm:py-5">
        <div className="flex flex-col gap-5">
          <div className="flex flex-col gap-4 xl:flex-row xl:items-end xl:justify-between">
            <div>
              <p className="manager-kicker">Trash</p>
              <h2 className="mt-3 font-['Iowan_Old_Style','Palatino_Linotype','Noto_Serif_SC',serif] text-4xl leading-none text-[color:var(--manager-ink-strong)]">回收站</h2>
              <p className="mt-3 max-w-2xl text-sm leading-7 text-[color:var(--manager-ink-soft)]">
                {trashedNotes.length === 0
                  ? '回收站为空'
                  : `共 ${trashedNotes.length} 条笔记，已删除的笔记将保留 30 天后自动清除。`}
              </p>
            </div>
            <div className="flex flex-wrap items-center gap-3">
              {selectedIds.size > 1 && (
                <>
                  <button
                    onClick={handleRestoreSelected}
                    className="rounded-full bg-[color:var(--manager-accent)] px-5 py-3 text-sm text-white transition hover:bg-[color:var(--manager-accent-strong)]"
                  >
                    恢复选中 ({selectedIds.size})
                  </button>
                  <button
                    onClick={clearSelection}
                    className="rounded-full border border-[color:var(--manager-border)] bg-white/75 px-5 py-3 text-sm text-[color:var(--manager-ink-soft)] transition hover:border-[color:var(--manager-accent)] hover:text-[color:var(--manager-ink-strong)]"
                  >
                    取消选择
                  </button>
                </>
              )}
              {trashedNotes.length > 0 && (
                <button
                  onClick={handleEmptyTrash}
                  className="rounded-full border border-red-300 bg-red-50 px-5 py-3 text-sm text-red-600 transition hover:bg-red-100"
                >
                  清空回收站
                </button>
              )}
            </div>
          </div>
        </div>
      </header>

      <div className="flex-1 overflow-auto px-5 py-4 sm:px-7 sm:py-5">
        {trashedNotes.length === 0 ? (
          <div className="flex h-full min-h-[320px] items-center justify-center rounded-[20px] border border-dashed border-[color:var(--manager-border)] bg-white/35 text-center">
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
                        : 'border-[color:var(--manager-border)] bg-white/68 hover:border-[color:var(--manager-accent)]'
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
                          className="rounded-full bg-green-50 px-3 py-1 text-xs text-green-600 transition hover:bg-green-100"
                        >
                          恢复
                        </button>
                        <button
                          onClick={() => handlePermanentDelete(note.id)}
                          className="rounded-full bg-red-50 px-3 py-1 text-xs text-red-600 transition hover:bg-red-100"
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
      </div>
    </section>
  );
}

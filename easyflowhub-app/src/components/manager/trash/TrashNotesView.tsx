/**
 * Notes section of the trash panel: trashed notes grouped by deletion time,
 * with single restore, permanent delete and multi-select checkboxes. Batch
 * restore and empty-trash live in the panel header because they act on the
 * page-level selection.
 */

import { useCallback, useMemo } from 'react';
import { permanentDeleteNote, restoreNote } from '../../../lib/tauri/notes';
import type { Note } from '../../../types/note';
import { deriveDisplayTitle } from '../../../lib/noteParser';
import { formatDeletedTime, TrashEmptyState } from './trashShared';

interface TrashNotesViewProps {
  notes: Note[];
  onNotesChange: (updater: (previous: Note[]) => Note[]) => void;
  /** Selection is page-level: the header renders the batch actions. */
  selectedIds: Set<string>;
  onSelectionChange: (next: Set<string>) => void;
}

export function TrashNotesView({ notes, onNotesChange, selectedIds, onSelectionChange }: TrashNotesViewProps) {
  const toggleSelection = useCallback(
    (id: string) => {
      const next = new Set(selectedIds);
      if (next.has(id)) {
        next.delete(id);
      } else {
        next.add(id);
      }
      onSelectionChange(next);
    },
    [onSelectionChange, selectedIds],
  );

  const handleRestore = useCallback(
    async (id: string) => {
      try {
        await restoreNote(id);
        onNotesChange((previous) => previous.filter((candidate) => candidate.id !== id));
        if (selectedIds.has(id)) {
          const next = new Set(selectedIds);
          next.delete(id);
          onSelectionChange(next);
        }
      } catch (err) {
        console.error('Failed to restore note:', err);
      }
    },
    [onNotesChange, onSelectionChange, selectedIds],
  );

  const handlePermanentDelete = useCallback(
    async (id: string) => {
      try {
        await permanentDeleteNote(id);
        onNotesChange((previous) => previous.filter((candidate) => candidate.id !== id));
        if (selectedIds.has(id)) {
          const next = new Set(selectedIds);
          next.delete(id);
          onSelectionChange(next);
        }
      } catch (err) {
        console.error('Failed to permanently delete note:', err);
      }
    },
    [onNotesChange, onSelectionChange, selectedIds],
  );

  const groupedNotes = useMemo(() => {
    const groups = new Map<string, Note[]>();
    for (const note of notes) {
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
  }, [notes]);

  if (notes.length === 0) {
    return <TrashEmptyState title="回收站为空" hint="删除的笔记会显示在这里。" />;
  }

  return (
    <>
      {groupedNotes.map(([timeKey, groupNotes]) => (
        <div key={timeKey} className="mb-6">
          <p className="mb-3 px-1 text-xs font-medium tracking-[0.12em] text-[color:var(--manager-ink-subtle)]">
            {timeKey}
          </p>
          <div className="space-y-2">
            {groupNotes.map((note) => {
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
  );
}

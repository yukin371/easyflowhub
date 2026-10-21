/**
 * Scripts section of the trash panel: scripts deleted from the script
 * workbench. Restore puts them back at their original storage location; purge
 * removes the file from the trash directory.
 */

import { useCallback } from 'react';
import { scriptTrashApi } from '../../../lib/api/scriptmgr';
import type { ScriptTrashEntry } from '../../../types/scriptmgr';
import {
  formatBytes,
  formatDeletedTime,
  TrashCard,
  TrashEmptyAction,
  TrashEmptyState,
  TrashEntryActions,
  TrashErrorBanner,
  TrashKindBadge,
  TrashLoadingHint,
  TrashMetaLine,
} from './trashShared';

interface TrashScriptsViewProps {
  entries: ScriptTrashEntry[];
  isLoading: boolean;
  error?: string;
  onEntriesChange: (updater: (previous: ScriptTrashEntry[]) => ScriptTrashEntry[]) => void;
  onErrorChange: (message: string | undefined) => void;
}

export function TrashScriptsView({ entries, isLoading, error, onEntriesChange, onErrorChange }: TrashScriptsViewProps) {
  const handleRestore = useCallback(
    async (entry: ScriptTrashEntry) => {
      try {
        await scriptTrashApi.restore(entry.id);
        onEntriesChange((previous) => previous.filter((candidate) => candidate.id !== entry.id));
        onErrorChange(undefined);
      } catch (err) {
        onErrorChange(err instanceof Error ? err.message : String(err));
      }
    },
    [onEntriesChange, onErrorChange],
  );

  const handlePurge = useCallback(
    async (entry: ScriptTrashEntry) => {
      if (!window.confirm(`彻底删除“${entry.name}”？此操作不可恢复。`)) return;
      try {
        await scriptTrashApi.purge(entry.id);
        onEntriesChange((previous) => previous.filter((candidate) => candidate.id !== entry.id));
        onErrorChange(undefined);
      } catch (err) {
        onErrorChange(err instanceof Error ? err.message : String(err));
      }
    },
    [onEntriesChange, onErrorChange],
  );

  const handleEmpty = useCallback(async () => {
    if (!window.confirm('彻底删除回收站中的所有脚本？此操作不可恢复。')) return;
    try {
      await scriptTrashApi.empty();
      onEntriesChange(() => []);
      onErrorChange(undefined);
    } catch (err) {
      onErrorChange(err instanceof Error ? err.message : String(err));
    }
  }, [onEntriesChange, onErrorChange]);

  return (
    <div className="space-y-3">
      <TrashErrorBanner message={error} />
      {isLoading ? (
        <TrashLoadingHint />
      ) : entries.length === 0 ? (
        <TrashEmptyState
          title="没有已删除的脚本"
          hint="在脚本工作台中删除的脚本会显示在这里。"
        />
      ) : (
        entries.map((entry) => (
          <TrashCard key={entry.id}>
            <div className="min-w-0 flex-1">
              <div className="flex flex-wrap items-center gap-2">
                <TrashKindBadge>{entry.script_type}</TrashKindBadge>
                <TrashMetaLine>
                  {formatDeletedTime(entry.deleted_at)} · {formatBytes(entry.size_bytes)}
                </TrashMetaLine>
              </div>
              <p className="mt-1 break-words text-sm font-medium text-[color:var(--manager-ink-strong)]">{entry.name}</p>
              <p className="mt-1 break-all font-mono text-[11px] text-[color:var(--manager-ink-subtle)]">
                {entry.original_path}
              </p>
            </div>
            <TrashEntryActions
              restoreLabel="恢复"
              onRestore={() => void handleRestore(entry)}
              onPurge={() => void handlePurge(entry)}
            />
          </TrashCard>
        ))
      )}
      {!isLoading && entries.length > 0 ? (
        <div className="flex justify-end">
          <TrashEmptyAction onEmpty={() => void handleEmpty()}>清空脚本回收站</TrashEmptyAction>
        </div>
      ) : null}
    </div>
  );
}

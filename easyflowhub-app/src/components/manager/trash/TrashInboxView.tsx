/**
 * Inbox section of the trash panel: soft-deleted capture items. Restore returns
 * them to their previous Inbox state; permanent delete / empty also removes the
 * stored attachments (and the recording source file) in native code.
 */

import { useCallback } from 'react';
import { emptyInboxTrash, permanentlyDeleteInboxItem, restoreInboxItemFromTrash } from '../../../lib/tauri/inbox';
import type { InboxItem } from '../../../types/inbox';
import {
  formatDeletedTime,
  inboxKindLabel,
  TrashCard,
  TrashEmptyAction,
  TrashEmptyState,
  TrashEntryActions,
  TrashErrorBanner,
  TrashKindBadge,
  TrashLoadingHint,
  TrashMetaLine,
} from './trashShared';

interface TrashInboxViewProps {
  items: InboxItem[];
  isLoading: boolean;
  error?: string;
  onItemsChange: (updater: (previous: InboxItem[]) => InboxItem[]) => void;
  onErrorChange: (message: string | undefined) => void;
}

export function TrashInboxView({ items, isLoading, error, onItemsChange, onErrorChange }: TrashInboxViewProps) {
  const handleRestore = useCallback(
    async (item: InboxItem) => {
      try {
        await restoreInboxItemFromTrash(item.id);
        onItemsChange((previous) => previous.filter((candidate) => candidate.id !== item.id));
        onErrorChange(undefined);
      } catch (err) {
        onErrorChange(err instanceof Error ? err.message : String(err));
      }
    },
    [onErrorChange, onItemsChange],
  );

  const handlePurge = useCallback(
    async (item: InboxItem) => {
      const label = item.title || item.content || inboxKindLabel(item);
      if (!window.confirm(`永久删除“${label}”？附件也会被清理，且无法恢复。`)) return;
      try {
        await permanentlyDeleteInboxItem(item.id);
        onItemsChange((previous) => previous.filter((candidate) => candidate.id !== item.id));
        onErrorChange(undefined);
      } catch (err) {
        onErrorChange(err instanceof Error ? err.message : String(err));
      }
    },
    [onErrorChange, onItemsChange],
  );

  const handleEmpty = useCallback(async () => {
    if (!window.confirm('永久删除收集箱回收站中的所有项目？附件也会被清理，且无法恢复。')) return;
    try {
      await emptyInboxTrash();
      onItemsChange(() => []);
      onErrorChange(undefined);
    } catch (err) {
      onErrorChange(err instanceof Error ? err.message : String(err));
    }
  }, [onErrorChange, onItemsChange]);

  return (
    <div className="space-y-3">
      <TrashErrorBanner message={error} />
      {isLoading ? (
        <TrashLoadingHint />
      ) : items.length === 0 ? (
        <TrashEmptyState
          title="没有已删除的收集项"
          hint="从收集箱删除的文字、图片、文件和录音会显示在这里。"
        />
      ) : (
        items.map((item) => (
          <TrashCard key={item.id}>
            <div className="min-w-0 flex-1">
              <div className="flex flex-wrap items-center gap-2">
                <TrashKindBadge>{inboxKindLabel(item)}</TrashKindBadge>
                <TrashMetaLine>{item.deletedAt ? formatDeletedTime(item.deletedAt) : '未知时间'}</TrashMetaLine>
              </div>
              <p className="mt-1 break-words text-sm font-medium text-[color:var(--manager-ink-strong)]">
                {item.title || inboxKindLabel(item)}
              </p>
              {item.content ? (
                <p className="mt-1 line-clamp-2 break-words text-xs text-[color:var(--manager-ink-soft)]">{item.content}</p>
              ) : null}
              {item.attachments.length > 0 ? (
                <p className="mt-1 break-words text-xs text-[color:var(--manager-ink-subtle)]">
                  附件：{item.attachments.map((attachment) => attachment.name).join('、')}
                </p>
              ) : null}
            </div>
            <TrashEntryActions
              restoreLabel="恢复"
              onRestore={() => void handleRestore(item)}
              onPurge={() => void handlePurge(item)}
            />
          </TrashCard>
        ))
      )}
      {!isLoading && items.length > 0 ? (
        <div className="flex justify-end">
          <TrashEmptyAction onEmpty={() => void handleEmpty()}>清空收集箱回收站</TrashEmptyAction>
        </div>
      ) : null}
    </div>
  );
}

/**
 * Shared presentational pieces of the trash panel.
 *
 * The three trash views (notes / inbox / scripts) render the same chrome:
 * an empty state card, an inline error banner and the pill action buttons.
 * Keeping them here stops each view from re-declaring the manager tokens.
 */

import type { InboxItem } from '../../../types/inbox';

export function formatDeletedTime(deletedAt: string): string {
  const date = new Date(deletedAt);
  const now = new Date();
  const diffMs = now.getTime() - date.getTime();
  const diffDays = Math.floor(diffMs / (1000 * 60 * 60 * 24));

  if (diffDays === 0) return '1 天前';
  if (diffDays < 7) return `${diffDays} 天前`;
  if (diffDays < 30) return `${Math.floor(diffDays / 7)} 周前`;
  return date.toLocaleDateString('zh-CN');
}

export function formatBytes(bytes: number): string {
  if (!bytes) return '-';
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}

export function inboxKindLabel(item: InboxItem): string {
  if (item.kind === 'link') return '链接';
  if (item.kind === 'image') return '图片';
  if (item.kind === 'file') return '文件';
  if (item.kind === 'audio') return '录音';
  return '文本';
}

interface TrashEmptyStateProps {
  title: string;
  hint: string;
}

export function TrashEmptyState({ title, hint }: TrashEmptyStateProps) {
  return (
    <div className="flex h-full min-h-[320px] items-center justify-center rounded-[20px] border border-dashed border-[color:var(--manager-border)] bg-[color:var(--manager-panel)] text-center">
      <div>
        <p className="font-['Iowan_Old_Style','Palatino_Linotype','Noto_Serif_SC',serif] text-3xl text-[color:var(--manager-ink-strong)]">{title}</p>
        <p className="mt-3 text-sm text-[color:var(--manager-ink-soft)]">{hint}</p>
      </div>
    </div>
  );
}

export function TrashErrorBanner({ message }: { message?: string }) {
  if (!message) return null;

  return (
    <p role="alert" className="rounded-[12px] border border-[color:var(--manager-danger)]/40 px-4 py-2 text-xs text-[color:var(--manager-danger)]">
      {message}
    </p>
  );
}

export function TrashLoadingHint() {
  return <p className="py-10 text-center text-sm text-[color:var(--manager-ink-muted)]">加载中…</p>;
}

interface TrashCardProps {
  children: React.ReactNode;
}

export function TrashCard({ children }: TrashCardProps) {
  return (
    <div className="rounded-[18px] border border-[color:var(--manager-border)] bg-[color:var(--manager-panel)] p-4 transition hover:border-[color:var(--manager-accent)]">
      <div className="flex items-start justify-between gap-3">{children}</div>
    </div>
  );
}

export function TrashKindBadge({ children }: { children: React.ReactNode }) {
  return (
    <span className="rounded-[5px] border border-[color:var(--manager-border)] px-1.5 py-0.5 text-[10px] text-[color:var(--manager-ink-muted)]">
      {children}
    </span>
  );
}

export function TrashMetaLine({ children }: { children: React.ReactNode }) {
  return <span className="text-[11px] text-[color:var(--manager-ink-subtle)]">{children}</span>;
}

interface TrashEntryActionsProps {
  restoreLabel: string;
  onRestore: () => void;
  onPurge: () => void;
}

export function TrashEntryActions({ restoreLabel, onRestore, onPurge }: TrashEntryActionsProps) {
  return (
    <div className="flex shrink-0 items-center gap-2">
      <button
        onClick={onRestore}
        className="rounded-full bg-[color:var(--manager-success)]/12 px-3 py-1 text-xs text-[color:var(--manager-success)] transition hover:bg-[color:var(--manager-success)]/20"
      >
        {restoreLabel}
      </button>
      <button
        onClick={onPurge}
        className="rounded-full bg-[color:var(--manager-danger)]/12 px-3 py-1 text-xs text-[color:var(--manager-danger)] transition hover:bg-[color:var(--manager-danger)]/20"
      >
        永久删除
      </button>
    </div>
  );
}

interface TrashEmptyActionProps {
  onEmpty: () => void;
  children: React.ReactNode;
}

/** Destructive "empty this trash" pill shown in the panel header. */
export function TrashEmptyAction({ onEmpty, children }: TrashEmptyActionProps) {
  return (
    <button
      onClick={onEmpty}
      className="rounded-full border border-[color:var(--manager-danger)]/40 bg-[color:var(--manager-danger)]/12 px-5 py-3 text-sm text-[color:var(--manager-danger)] transition hover:bg-[color:var(--manager-danger)]/20"
    >
      {children}
    </button>
  );
}

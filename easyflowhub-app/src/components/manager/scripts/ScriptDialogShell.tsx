/**
 * Shared shell and styles for script file management dialogs.
 */

import type { ReactNode } from 'react';

export const SCRIPT_EXTENSIONS = ['ps1', 'py', 'bat', 'cmd'];

export const primaryButton =
  'rounded-[10px] bg-[color:var(--manager-accent)] px-4 py-2 text-sm text-white transition disabled:opacity-50';
export const secondaryButton =
  'rounded-[10px] border border-[color:var(--manager-border)] bg-[color:var(--manager-panel-strong)] px-4 py-2 text-sm text-[color:var(--manager-ink-soft)] transition hover:border-[color:var(--manager-accent)] hover:text-[color:var(--manager-ink-strong)] disabled:opacity-50';
export const dangerButton =
  'rounded-[10px] border border-[color:var(--manager-danger)] px-4 py-2 text-sm text-[color:var(--manager-danger)] transition hover:bg-[color:var(--manager-danger)]/10 disabled:opacity-50';
export const inputClass =
  'w-full rounded-[10px] border border-[color:var(--manager-border)] bg-[color:var(--manager-panel-strong)] px-3 py-2 text-sm text-[color:var(--manager-ink-strong)] outline-none transition placeholder:text-[color:var(--manager-ink-subtle)] focus:border-[color:var(--manager-accent)]';

export function Modal({
  title,
  subtitle,
  onClose,
  children,
  footer,
  wide = false,
}: {
  title: string;
  subtitle?: string;
  onClose: () => void;
  children: ReactNode;
  footer?: ReactNode;
  wide?: boolean;
}) {
  return (
    <div
      className="fixed inset-0 z-50 flex items-center justify-center bg-black/45 p-4"
      role="dialog"
      aria-modal="true"
      aria-label={title}
    >
      <div
        className={`flex max-h-[90vh] w-full ${
          wide ? 'max-w-[880px]' : 'max-w-[560px]'
        } flex-col overflow-hidden rounded-[20px] border border-[color:var(--manager-border)] bg-[color:var(--manager-panel)] shadow-[0_30px_80px_rgba(0,0,0,0.35)]`}
      >
        <header className="flex shrink-0 items-start justify-between gap-3 border-b border-[color:var(--manager-border)] px-5 py-4">
          <div className="min-w-0">
            <h3 className="truncate text-base font-medium text-[color:var(--manager-ink-strong)]">{title}</h3>
            {subtitle ? (
              <p className="mt-1 truncate text-xs text-[color:var(--manager-ink-subtle)]">{subtitle}</p>
            ) : null}
          </div>
          <button
            type="button"
            onClick={onClose}
            aria-label="关闭"
            className="flex h-8 w-8 shrink-0 items-center justify-center rounded-full text-[color:var(--manager-ink-muted)] transition hover:bg-[color:var(--manager-accent-soft)] hover:text-[color:var(--manager-ink-strong)]"
          >
            ✕
          </button>
        </header>
        <div className="min-h-0 flex-1 overflow-auto px-5 py-4">{children}</div>
        {footer ? (
          <footer className="shrink-0 border-t border-[color:var(--manager-border)] px-5 py-3">{footer}</footer>
        ) : null}
      </div>
    </div>
  );
}

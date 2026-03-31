import type { DiffResult } from '../../lib/utils/diff';

interface DiffViewProps {
  diff: DiffResult;
}

const LINE_STYLES: Record<string, string> = {
  add: 'bg-[rgba(101,114,85,0.12)] text-[color:var(--manager-ink)]',
  delete: 'bg-[rgba(154,78,63,0.1)] text-[color:var(--manager-ink)]',
  modify: 'bg-[rgba(203,163,92,0.12)] text-[color:var(--manager-ink)]',
  same: 'bg-transparent text-[color:var(--manager-ink-soft)]',
};

export function DiffView({ diff }: DiffViewProps) {
  return (
    <div className="overflow-hidden rounded-[24px] border border-[color:var(--manager-border)] bg-white/55">
      <div className="grid grid-cols-[72px_minmax(0,1fr)_minmax(0,1fr)] border-b border-[color:var(--manager-border)] px-4 py-3 text-xs uppercase tracking-[0.24em] text-[color:var(--manager-ink-subtle)]">
        <span>行号</span>
        <span>当前版本</span>
        <span>备份版本</span>
      </div>
      <div className="max-h-[320px] overflow-auto">
        {diff.lines.map((line) => (
          <div
            key={`${line.lineNumber}-${line.type}`}
            className={`grid grid-cols-[72px_minmax(0,1fr)_minmax(0,1fr)] gap-3 px-4 py-2 text-sm leading-6 ${LINE_STYLES[line.type]}`}
          >
            <span className="text-xs uppercase tracking-[0.18em] text-[color:var(--manager-ink-subtle)]">
              {line.lineNumber}
            </span>
            <pre className="whitespace-pre-wrap break-words font-inherit">
              {line.currentLine ?? ''}
            </pre>
            <pre className="whitespace-pre-wrap break-words font-inherit">
              {line.backupLine ?? ''}
            </pre>
          </div>
        ))}
      </div>
    </div>
  );
}

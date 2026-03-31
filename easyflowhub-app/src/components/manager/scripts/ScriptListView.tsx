/**
 * ScriptListView - 脚本列表视图
 */

import type { ScriptSummary } from '../../../types/scriptmgr';

interface ScriptListViewProps {
  scripts: ScriptSummary[];
  onSelect: (scriptId: string) => void;
}

function ScriptTypeIcon({ type }: { type: string }) {
  const icons: Record<string, string> = {
    python: '🐍',
    powershell: '⚡',
    bash: '🐚',
    batch: '📜',
    javascript: '✨',
    typescript: '🔷',
  };
  return <span>{icons[type] || '📄'}</span>;
}

export function ScriptListView({ scripts, onSelect }: ScriptListViewProps) {
  return (
    <div className="space-y-2">
      {scripts.map((script) => (
        <button
          key={script.id}
          onClick={() => onSelect(script.id)}
          className="group flex w-full items-start gap-3 rounded-[16px] border border-transparent bg-white/55 px-4 py-3 text-left transition-all hover:border-[color:var(--manager-border)] hover:bg-white/75"
        >
          {/* Icon */}
          <span className="flex h-10 w-10 shrink-0 items-center justify-center rounded-[12px] border border-[color:var(--manager-border)] bg-white/70 text-base">
            <ScriptTypeIcon type={script.script_type} />
          </span>

          {/* Content */}
          <div className="min-w-0 flex-1">
            <div className="flex items-center gap-2">
              <span className="truncate font-medium text-[color:var(--manager-ink-strong)]">
                {script.name}
              </span>
              {script.category && (
                <span className="shrink-0 rounded-full bg-[color:var(--manager-accent-soft)] px-2 py-0.5 text-[11px] text-[color:var(--manager-accent)]">
                  {script.category}
                </span>
              )}
            </div>

            {/* Description */}
            {script.description && (
              <p className="mt-0.5 line-clamp-1 text-sm text-[color:var(--manager-ink-soft)]">
                {script.description}
              </p>
            )}

            {/* Tags */}
            {script.tags && script.tags.length > 0 && (
              <div className="mt-1.5 flex flex-wrap gap-1">
                {script.tags.slice(0, 4).map((tag) => (
                  <span
                    key={tag}
                    className="rounded-full border border-[color:var(--manager-border)] bg-white/50 px-2 py-0.5 text-[11px] text-[color:var(--manager-ink-muted)]"
                  >
                    #{tag}
                  </span>
                ))}
                {script.tags.length > 4 && (
                  <span className="text-[11px] text-[color:var(--manager-ink-subtle)]">
                    +{script.tags.length - 4}
                  </span>
                )}
              </div>
            )}
          </div>

          {/* Arrow */}
          <span className="flex h-10 w-10 shrink-0 items-center justify-center rounded-full text-[color:var(--manager-ink-subtle)] opacity-0 transition-opacity group-hover:opacity-100">
            →
          </span>
        </button>
      ))}
    </div>
  );
}

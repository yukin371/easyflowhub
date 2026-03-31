interface TagFilterBarProps {
  selectedTags: string[];
  allTags: Array<[string, number]>;
  filterLogic: 'and' | 'or';
  onToggleTag: (tag: string) => void;
  onChangeLogic: (logic: 'and' | 'or') => void;
  onClear: () => void;
}

export function TagFilterBar(props: TagFilterBarProps) {
  const { selectedTags, allTags, filterLogic, onToggleTag, onChangeLogic, onClear } = props;

  return (
    <div className="rounded-[15px] bg-[rgba(255,251,245,0.52)] px-3 py-2">
      <div className="flex flex-wrap items-center gap-2">
        <span className="manager-kicker">Logic</span>
        <div className="flex items-center gap-1 rounded-full bg-[color:var(--manager-accent-soft)] p-1">
          {(['or', 'and'] as const).map((logic) => (
            <button
              key={logic}
              onClick={() => onChangeLogic(logic)}
              className={`rounded-full px-3 py-1 text-xs uppercase tracking-[0.22em] transition ${
                filterLogic === logic
                  ? 'bg-[color:var(--manager-accent)] text-white'
                  : 'text-[color:var(--manager-ink-soft)]'
              }`}
            >
              {logic}
            </button>
          ))}
        </div>
        <span className="manager-kicker ml-2">Filter Tags</span>
        {selectedTags.length === 0 ? (
          <span className="text-xs tracking-[0.12em] text-[color:var(--manager-ink-subtle)]">未选择筛选标签</span>
        ) : (
          <span className="text-xs tracking-[0.12em] text-[color:var(--manager-ink-subtle)]">
            已选择 {selectedTags.length} 个
          </span>
        )}
        {selectedTags.length > 0 ? (
          <button
            onClick={onClear}
            className="ml-auto text-xs uppercase tracking-[0.18em] text-[color:var(--manager-ink-soft)] transition hover:text-[color:var(--manager-ink-strong)]"
          >
            清空
          </button>
        ) : null}
      </div>

      {allTags.length > 0 ? (
        <div className="mt-2 flex flex-wrap gap-2">
          {allTags.slice(0, 12).map(([tag, count]) => {
            const active = selectedTags.includes(tag);
            return (
              <button
                key={tag}
                onClick={() => onToggleTag(tag)}
                className={`rounded-full border px-3 py-1 text-xs tracking-[0.14em] transition ${
                  active
                    ? 'border-[color:var(--manager-accent)] bg-[color:var(--manager-accent)] text-white'
                    : 'border-[color:var(--manager-border)] bg-[rgba(255,251,245,0.56)] text-[color:var(--manager-ink-soft)] hover:border-[color:var(--manager-accent)] hover:text-[color:var(--manager-ink-strong)]'
                }`}
              >
                #{tag} <span className="opacity-60">{count}</span>
              </button>
            );
          })}
        </div>
      ) : null}
    </div>
  );
}

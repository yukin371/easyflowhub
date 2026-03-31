import { useState } from 'react';

interface StatusBarProps {
  draftTags: string;
  allTags: Array<[string, number]>;
  splitTags: (tagString: string) => string[];
  onTagRemove: (tag: string) => void;
  onTagsChange: (value: string) => void;
  saveStateLabel: string;
  contentLength: number;
  saveError: string;
}

export function StatusBar(props: StatusBarProps) {
  const {
    draftTags,
    allTags,
    splitTags,
    onTagRemove,
    onTagsChange,
    saveStateLabel,
    contentLength,
    saveError,
  } = props;
  const activeTags = splitTags(draftTags);
  const [tagInput, setTagInput] = useState('');

  const suggestions = allTags.filter(([tag]) => {
    if (activeTags.includes(tag)) {
      return false;
    }
    if (!tagInput.trim()) {
      return true;
    }
    return tag.toLowerCase().includes(tagInput.trim().replace(/^#/, '').toLowerCase());
  }).slice(0, 6);

  const commitTag = (raw: string) => {
    const normalized = raw.trim().replace(/^#+/, '');
    if (!normalized || activeTags.includes(normalized)) {
      setTagInput('');
      return;
    }

    onTagsChange([...activeTags, normalized].join(' '));
    setTagInput('');
  };

  return (
    <footer className="border-t border-[color:var(--manager-border)] bg-white/45 px-5 py-2.5 backdrop-blur-sm sm:px-8">
      <div className="mx-auto flex max-w-5xl flex-col gap-2">
        <div className="flex flex-col gap-2 sm:flex-row sm:items-center sm:justify-between">
          <div className="flex min-w-0 flex-1 flex-col gap-2 sm:max-w-[560px]">
            <div className="flex flex-wrap items-center gap-2 rounded-[14px] border border-[color:var(--manager-border)] bg-white/80 px-3 py-1.5">
              <span className="manager-kicker shrink-0">Tags</span>
              {activeTags.map((tag) => (
                <button
                  key={tag}
                  onClick={() => onTagRemove(tag)}
                  className="rounded-full bg-[color:var(--manager-accent-soft)] px-2.5 py-0.5 text-xs tracking-[0.12em] text-[color:var(--manager-ink-soft)] transition hover:bg-white"
                >
                  #{tag}
                </button>
              ))}
              <input
                value={tagInput}
                onChange={(event) => setTagInput(event.target.value)}
                onKeyDown={(event) => {
                  if (event.key === 'Enter' || event.key === ' ' || event.key === ',') {
                    event.preventDefault();
                    commitTag(tagInput);
                  }
                  if (event.key === 'Backspace' && !tagInput && activeTags.length > 0) {
                    onTagRemove(activeTags[activeTags.length - 1]);
                  }
                }}
                placeholder={activeTags.length > 0 ? '添加标签' : '输入标签，空格或回车确认'}
                className="min-w-[140px] flex-1 border-none bg-transparent py-0.5 text-sm text-[color:var(--manager-ink)] outline-none placeholder:text-[color:var(--manager-ink-subtle)]"
              />
            </div>
            {suggestions.length > 0 ? (
              <div className="flex flex-wrap gap-2">
                {suggestions.map(([tag, count]) => (
                  <button
                    key={tag}
                    onClick={() => commitTag(tag)}
                    className="rounded-full border border-[color:var(--manager-border)] bg-white/70 px-2.5 py-0.5 text-xs tracking-[0.12em] text-[color:var(--manager-ink-subtle)] transition hover:border-[color:var(--manager-accent)] hover:text-[color:var(--manager-ink-strong)]"
                  >
                    #{tag} <span className="opacity-60">{count}</span>
                  </button>
                ))}
              </div>
            ) : null}
          </div>
          <div className="flex items-center gap-4 text-sm text-[color:var(--manager-ink-soft)]">
            <span>{saveStateLabel}</span>
            <span>{contentLength} 字符</span>
            {saveError ? <span className="text-[color:var(--manager-danger)]">{saveError}</span> : null}
          </div>
        </div>
      </div>
    </footer>
  );
}

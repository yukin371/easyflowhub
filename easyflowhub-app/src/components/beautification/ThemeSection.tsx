// src/components/beautification/ThemeSection.tsx
import type { ThemeMode } from '../../types/appearance';
import { THEME_OPTIONS } from '../../types/appearance';

interface ThemeSectionProps {
  theme: ThemeMode;
  onThemeChange: (theme: ThemeMode) => void;
}

export function ThemeSection({ theme, onThemeChange }: ThemeSectionProps) {
  return (
    <section>
      <h2 className="mb-4 text-sm font-medium text-[color:var(--manager-ink-strong)]">主题</h2>
      <div className="grid grid-cols-3 gap-3">
        {THEME_OPTIONS.map((option) => (
          <button
            key={option.id}
            onClick={() => onThemeChange(option.id)}
            className={`
              flex flex-col items-center gap-2 rounded-[18px] border-2 p-4 transition-all
              ${theme === option.id
                ? 'border-[color:var(--manager-accent)] bg-[color:var(--manager-accent-soft)] shadow-[var(--manager-shadow)]'
                : 'border-[color:var(--manager-border)] bg-[color:var(--manager-panel)] hover:border-[color:var(--manager-accent)] hover:bg-[color:var(--manager-glass-highlight)]'
              }
            `}
          >
            <span className="text-2xl">{option.icon}</span>
            <span className="text-sm text-[color:var(--manager-ink-soft)]">{option.name}</span>
          </button>
        ))}
      </div>
    </section>
  );
}

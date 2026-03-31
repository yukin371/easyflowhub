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
      <h2 className="text-sm font-medium text-gray-300 mb-4">主题</h2>
      <div className="grid grid-cols-3 gap-3">
        {THEME_OPTIONS.map((option) => (
          <button
            key={option.id}
            onClick={() => onThemeChange(option.id)}
            className={`
              p-4 rounded-lg border-2 flex flex-col items-center gap-2 transition-all
              ${theme === option.id
                ? 'border-blue-500 bg-blue-900/20'
                : 'border-white/10 hover:border-white/20 bg-white/5'
              }
            `}
          >
            <span className="text-2xl">{option.icon}</span>
            <span className="text-sm text-gray-300">{option.name}</span>
          </button>
        ))}
      </div>
    </section>
  );
}

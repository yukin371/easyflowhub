// src/components/beautification/WindowSection.tsx
import type { WindowAppearance } from '../../types/appearance';

interface WindowSectionProps {
  window: WindowAppearance;
  onChange: (updates: Partial<WindowAppearance>) => void;
}

export function WindowSection({ window: windowConfig, onChange }: WindowSectionProps) {
  return (
    <section>
      <h2 className="mb-4 text-sm font-medium text-[color:var(--manager-ink-strong)]">窗口外观</h2>
      <div className="space-y-4">
        {/* 透明度 */}
        <div>
          <div className="flex items-center justify-between mb-2">
            <label className="text-sm text-[color:var(--manager-ink-soft)]">透明度</label>
            <span className="text-sm text-[color:var(--manager-ink-soft)]">{Math.round(windowConfig.opacity * 100)}%</span>
          </div>
          <input
            type="range"
            min="0.3"
            max="1"
            step="0.05"
            value={windowConfig.opacity}
            onChange={(e) => onChange({ opacity: parseFloat(e.target.value) })}
            className="h-2 w-full cursor-pointer appearance-none rounded-lg bg-[color:var(--manager-accent-soft)]"
            style={{ accentColor: 'var(--manager-accent)' }}
          />
        </div>

        {/* 圆角大小 */}
        <div>
          <div className="flex items-center justify-between mb-2">
            <label className="text-sm text-[color:var(--manager-ink-soft)]">圆角大小</label>
            <span className="text-sm text-[color:var(--manager-ink-soft)]">{windowConfig.cornerRadius}px</span>
          </div>
          <input
            type="range"
            min="0"
            max="20"
            step="1"
            value={windowConfig.cornerRadius}
            onChange={(e) => onChange({ cornerRadius: parseInt(e.target.value, 10) })}
            className="h-2 w-full cursor-pointer appearance-none rounded-lg bg-[color:var(--manager-accent-soft)]"
            style={{ accentColor: 'var(--manager-accent)' }}
          />
        </div>

        {/* 开关选项 */}
        <div className="space-y-2">
          <label className="flex cursor-pointer items-center gap-3 text-sm text-[color:var(--manager-ink-soft)]">
            <input
              type="checkbox"
              checked={windowConfig.isAlwaysOnTop}
              onChange={(e) => onChange({ isAlwaysOnTop: e.target.checked })}
              className="h-4 w-4 rounded border-[color:var(--manager-border)] bg-[color:var(--manager-panel)] focus:ring-[color:var(--manager-accent)]"
              style={{ accentColor: 'var(--manager-accent)' }}
            />
            窗口置顶
          </label>
          <label className="flex cursor-pointer items-center gap-3 text-sm text-[color:var(--manager-ink-soft)]">
            <input
              type="checkbox"
              checked={windowConfig.isBlurEnabled}
              onChange={(e) => onChange({ isBlurEnabled: e.target.checked })}
              className="h-4 w-4 rounded border-[color:var(--manager-border)] bg-[color:var(--manager-panel)] focus:ring-[color:var(--manager-accent)]"
              style={{ accentColor: 'var(--manager-accent)' }}
            />
            模糊背景
          </label>
        </div>
      </div>
    </section>
  );
}

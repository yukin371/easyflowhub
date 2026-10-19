// src/components/beautification/FontSection.tsx
import type { FontSettings } from '../../types/appearance';
import { AVAILABLE_FONTS } from '../../types/appearance';

interface FontSectionProps {
  font: FontSettings;
  onChange: (updates: Partial<FontSettings>) => void;
}

export function FontSection({ font, onChange }: FontSectionProps) {
  return (
    <section>
      <h2 className="mb-4 text-sm font-medium text-[color:var(--manager-ink-strong)]">字体设置</h2>
      <div className="space-y-4">
        {/* 字体选择 */}
        <div>
          <label className="mb-2 block text-sm text-[color:var(--manager-ink-soft)]">字体</label>
          <select
            value={font.fontFamily}
            onChange={(e) => onChange({ fontFamily: e.target.value })}
            className="w-full rounded-[12px] border border-[color:var(--manager-border)] bg-[color:var(--manager-panel)] px-3 py-2 text-sm text-[color:var(--manager-ink)] outline-none focus:border-[color:var(--manager-accent)]"
          >
            {AVAILABLE_FONTS.map((fontName) => (
              <option key={fontName} value={fontName} style={{ fontFamily: fontName }}>
                {fontName}
              </option>
            ))}
          </select>
        </div>

        {/* 字体大小 */}
        <div>
          <div className="flex items-center justify-between mb-2">
            <label className="text-sm text-[color:var(--manager-ink-soft)]">字体大小</label>
            <span className="text-sm text-[color:var(--manager-ink-soft)]">{font.fontSize}px</span>
          </div>
          <input
            type="range"
            min="12"
            max="24"
            step="1"
            value={font.fontSize}
            onChange={(e) => onChange({ fontSize: parseInt(e.target.value, 10) })}
            className="h-2 w-full cursor-pointer appearance-none rounded-lg bg-[color:var(--manager-accent-soft)]"
            style={{ accentColor: 'var(--manager-accent)' }}
          />
        </div>

        {/* 行高 */}
        <div>
          <div className="flex items-center justify-between mb-2">
            <label className="text-sm text-[color:var(--manager-ink-soft)]">行高</label>
            <span className="text-sm text-[color:var(--manager-ink-soft)]">{font.lineHeight.toFixed(1)}</span>
          </div>
          <input
            type="range"
            min="1"
            max="2"
            step="0.1"
            value={font.lineHeight}
            onChange={(e) => onChange({ lineHeight: parseFloat(e.target.value) })}
            className="h-2 w-full cursor-pointer appearance-none rounded-lg bg-[color:var(--manager-accent-soft)]"
            style={{ accentColor: 'var(--manager-accent)' }}
          />
        </div>
      </div>
    </section>
  );
}
